//! `apps-shell serve`: the apps.shell cell behind this machine's door, its liveness loop, and its gateway face.
//!
//! The face listens on loopback once per tab route it serves (`$APPS_SHELL_HTTP`, default
//! `127.0.0.1:8881=/approvals/,127.0.0.1:8882=/core/,127.0.0.1:8883=/search/`); the apps.ce-net.com sign-in gate sends `/<route>*` there with
//! the prefix cut and the person signed (`X-Ce-Person*`, the gate's per-host key, repo:ui-cells site-gateway's check).
//! - `GET /` the page (the same bytes for everyone; it names no system), `/_k/*` the kit;
//! - `GET /tabs.json` this viewer's tabs: allowed by their abilities and live (their shell answered HealthOp by name
//!   within LIVE_MS; asked every PROBE_MS); a tab's `view` only when this site carries it;
//! - `GET /v/<tab>.js` a tab's view module, only to a viewer whose tabs include that tab (404 to anyone else);
//! - `GET /api/search?q=<words>`: the Search tab, search.hybrid `Find` and search.mesh `Mesh` at once, in three groups:
//!   `private` (search.hybrid by the viewer's exact read grants), `network` (search.mesh: every node's own index of
//!   repos, cells and docs; internal, only with `search.mesh:find`) and `public` (what cells published for everyone);
//!   each group its own [`PER_GROUP`] (default 10, `k=` 1 to 50): an owner's private hits never push Public to 0;
//! - `GET /api/ref?r=repo:<repo>/<path>[@<commit>]`: the Search tab's file viewer, through the cell reference system
//!   as the viewer: ref.resolve `Resolve` (where it lives, its ce-ref:// id and content hash) and ref.pin `Fetch` (only
//!   the bytes with that hash); with no commit, the HEAD of the checkout those cells read on this node;
//! - `GET /api/inbox`, `/api/card?b=<id>`, `/api/cellmap`: the data cells called by name with a token attenuated
//!   from this cell's grant to exactly the viewer's abilities in that cell (their row caveats included), so the data
//!   cell's own gate decides, for this viewer.
use apps_shell::*;
use apps_shell_api::*;
use ce_wire::Server;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

mod site {
    include!(concat!(env!("OUT_DIR"), "/site.rs"));
}

type Live = Arc<RwLock<BTreeMap<String, u64>>>;

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn env_or(k: &str, d: String) -> String {
    std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or(d)
}

fn now_s() -> u64 {
    ui_chain::now_ms() / 1000
}

/// HealthOp of one tab shell, by name, with this cell's key: Some(ok) when a replica answered.
macro_rules! health {
    ($api:path, $op:path, $right:expr) => {{
        async {
            let tok = ui_chain::source_token::<$api>(&[$right], 30, None).ok()?;
            let mut c = tokio::time::timeout(Duration::from_millis(HEALTH_MS), ce_wire::Client::<$api>::by_name(tok)).await.ok()?.ok()?;
            c.deadline = Duration::from_millis(HEALTH_MS);
            let h = c.call::<$op>(&()).await.ok()?;
            Some(h.ok)
        }
        .await
    }};
}

async fn shell_ok(shell: &str) -> bool {
    let r = match shell {
        "approve.inbox" => health!(approve_inbox_api::ApproveInbox, approve_inbox_api::HealthOp, "approve.inbox:health"),
        "site.cellmap" => health!(site_cellmap_api::SiteCellmap, site_cellmap_api::HealthOp, "site.cellmap:health"),
        "foreman.shell" => health!(foreman_shell_api::ForemanShell, foreman_shell_api::HealthOp, "foreman.shell:health"),
        "cr.shell" => health!(cr_shell_api::CrShell, cr_shell_api::HealthOp, "cr.shell:health"),
        "search.hybrid" => health!(hybrid_api::SearchHybrid, hybrid_api::HealthOp, "search.hybrid:health"),
        // a shell whose contract this build does not carry yet: never live here (its system shows in site.cellmap)
        _ => None,
    };
    r == Some(true)
}

async fn liveness(live: Live) {
    loop {
        let t0 = Instant::now();
        let shells: Vec<&'static str> = all_tabs().iter().map(|(_, t)| t.shell.name).collect();
        let hs: Vec<_> = shells.into_iter().map(|s| tokio::spawn(async move { (s, shell_ok(s).await) })).collect();
        for h in hs {
            if let Ok((s, true)) = h.await {
                live.write().await.insert(s.to_string(), ui_chain::now_ms());
            }
        }
        tokio::time::sleep(Duration::from_millis(PROBE_MS).saturating_sub(t0.elapsed())).await;
    }
}

struct Face {
    route: String,
    live: Live,
    person_key: PathBuf,
    people: PathBuf,
}

impl Face {
    fn abilities(&self, person: &str) -> Vec<String> {
        let text = std::fs::read_to_string(&self.people).ok();
        apps_shell::abilities(text.as_deref(), person, now_s())
    }
}

async fn reply(s: &mut TcpStream, status: &str, ty: &str, body: &[u8], head: bool) {
    let csp = "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self' https://ce-net.com https://fonts.gstatic.com; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
    let h = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ty}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: {csp}\r\nVary: X-Ce-Person\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(h.as_bytes()).await;
    if !head {
        let _ = s.write_all(body).await;
    }
}

fn json<T: serde::Serialize>(v: &T) -> Vec<u8> {
    serde_json::to_vec(v).unwrap_or_default()
}

fn kind(n: &str) -> &'static str {
    match n.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "woff2" => "font/woff2",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn file(name: &str) -> Option<&'static [u8]> {
    site::SITE.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

/// A query value with `+` and `%xx` decoded (None when it is not UTF-8).
fn query_text(path: &str, k: &str) -> Option<String> {
    let raw = query(path, k)?;
    let b = raw.as_bytes();
    let (mut out, mut i) = (Vec::with_capacity(b.len()), 0);
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => match std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                Some(v) => {
                    out.push(v);
                    i += 2;
                }
                None => out.push(b'%'),
            },
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8(out).ok()
}

fn query(path: &str, k: &str) -> Option<String> {
    let q = path.split_once('?')?.1;
    q.split('&').find_map(|kv| kv.split_once('=').filter(|(a, _)| *a == k).map(|(_, v)| v.to_string()))
}

#[derive(serde::Serialize)]
struct TabOut {
    system: String,
    title: String,
    route: String,
    view: Option<String>,
    badge: Option<u32>,
}

#[derive(serde::Serialize)]
struct TabsOut {
    person: String,
    here: String,
    tabs: Vec<TabOut>,
    at_ms: u64,
}

/// wire.ws-door's contract (repo:wire-ws-door crates/wire-ws-door-api, major 2), copied verbatim as in repo:control-room.
#[allow(dead_code)]
mod door_api {
    ce_wire::api! {
        api WireWsDoor = "wire.ws-door", version 2;
        types {
            /// Health contract: these three fields, in this order, in every cell.
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            /// `page`: the app's URL the grant is appended to (`<page>#g=...`). `abilities`: what the browser may do,
            /// e.g. ["doc.workspace:read"]. `ttl_s`: 60 to 86400.
            pub struct MintReq { pub page: String, pub abilities: Vec<String>, pub ttl_s: u32 }
            /// `link`: page + fragment; the fragment is the secret and a browser never sends it to a server.
            pub struct Minted { pub link: String, pub not_after: u64, pub browser_key: String }
            /// `NotAllowed`: an asked ability outside what the caller's own grant lets it mint (its
            /// `wire.ws-door:mint-for:<pattern>` abilities), or one no caller but this node may ever mint (keys,
            /// cell.install, this door, any own-key-only cell, a wildcard, `delegate:`).
            pub enum MintError { Invalid(String), Failed(String), NotAllowed { ability: String, why: String } }
        }
        ops {
            1 => HealthOp(()) -> Health, needs "wire.ws-door:health";
            2 => Mint(MintReq) -> Result<Minted, MintError>, needs "wire.ws-door:mint";
        }
    }
}

/// search.hybrid's contract (repo:search-hybrid crates/search-hybrid-api, major 1), copied verbatim like door_api:
/// its doc comments are part of the schema hash (2e6c7e98274d467f), so not a character may change.
#[allow(dead_code)]
mod hybrid_api {
    ce_wire::api! {
        api SearchHybrid = "search.hybrid", version 1;
        types {
            /// Health contract: these three fields, in this order, in every cell.
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            /// `q` in plain words. `k` 0 = 10, at most 50. `sources` narrows to "files", "words" and/or "mail"; empty = all
            /// the caller may read. `rerank` false skips the cross-encoder (faster, rougher).
            pub struct FindReq { pub q: String, pub k: u32, pub sources: Vec<String>, pub rerank: bool }
            /// One document (best chunk per path). `id` is "<source>:<machine>:<path>"; `path` is relative to the
            /// machine's dev dir for files. `bm25_rank` / `vec_rank`: 1-based place in each list, 0 = not in it. `acl`: the
            /// label whose right let this caller see it.
            pub struct Hit { pub id: String, pub source: String, pub machine: String, pub path: String, pub title: String,
                             pub snippet: String, pub score: f32, pub bm25_rank: u32, pub vec_rank: u32, pub acl: String }
            pub struct FindResp { pub hits: Vec<Hit>, pub took_ms: f64, pub chunks: u32, pub reranked: bool, pub model: String }
            pub struct StatRow { pub source: String, pub machine: String, pub docs: u32, pub chunks: u32 }
            /// `built_ms`: when the index on disk was built (epoch ms).
            pub struct StatsResp { pub chunks: u32, pub docs: u32, pub built_ms: u64, pub model: String, pub reranker: String, pub rows: Vec<StatRow> }
            pub enum FindError { Invalid(String), NotReady(String), Failed(String) }
        }
        ops {
            1 => HealthOp(()) -> Health, needs "search.hybrid:health";
            // Only the docs the caller's abilities read: `search.hybrid:read:<acl>` for acl `repo:<name>`, `words` (Leif's
            // talk log), `mail`; ce-cap wildcards (`search.hybrid:read:repo:*`, `search.hybrid:*`, `*`) cover repos only, words
            // and mail need a grant naming them. No ability = no hits;
            // a doc without an acl is shown to nobody; key-like values are masked at crawl. `chunks` counts visible ones.
            2 => Find(FindReq) -> Result<FindResp, FindError>, needs "search.hybrid:find", deadline_ms 20000;
            // Counts of what the caller can read, nothing else.
            4 => Stats(()) -> StatsResp, needs "search.hybrid:stats";
        }
    }
}

/// search.mesh's contract (repo:ce-search crates/search-mesh-api, major 1), copied verbatim like door_api (schema
/// b78f3a1097281268).
#[allow(dead_code)]
mod mesh_api {
    ce_wire::api! {
        api SearchMesh = "search.mesh", version 1;
        types {
            /// Health contract: these three fields, in this order, in every cell.
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            /// `q` in plain words; at most `k` hits (0 = 20); `sources`: source keys to ask (`Sources` lists them), empty = all.
            pub struct LocalReq { pub q: String, pub k: u32, pub sources: Vec<String> }
            /// One thing this node holds. `source`: which local source found it; `kind`: repo, cell, doc or page; `key`:
            /// the same on every node for the same thing (a repo's origin, a cell's api, a doc's repo and path), the
            /// merge joins on it; `machine`: where it lives (the answering node unless its index says another machine);
            /// `path`: where on that machine (`~` is that machine's home); `how`: the call or command that uses it.
            pub struct Hit { pub source: String, pub kind: String, pub key: String, pub title: String, pub snippet: String, pub score: f64, pub machine: String, pub path: String, pub how: String }
            /// One local source's part: how many hits, how long, and why it did not answer (empty when it did).
            pub struct SourceNote { pub key: String, pub hits: u32, pub took_ms: f64, pub err: String }
            /// `node`: this node's name; `indexed`: things in its own index; `age_s`: how old that index is.
            pub struct LocalResp { pub node: String, pub hits: Vec<Hit>, pub sources: Vec<SourceNote>, pub indexed: u32, pub age_s: u64, pub took_ms: f64 }
            /// `deadline_ms`: how long any node may take (0 = 800); a node that has not answered by then is reported, not waited for.
            pub struct MeshReq { pub q: String, pub k: u32, pub deadline_ms: u32, pub sources: Vec<String> }
            /// One place a result lives and how to use it there.
            pub struct Location { pub node: String, pub path: String, pub how: String }
            /// One merged result: `score` 0..1 after normalisation; `locations`: every node that holds it.
            pub struct Merged { pub kind: String, pub key: String, pub title: String, pub snippet: String, pub score: f64, pub locations: Vec<Location>, pub sources: Vec<String> }
            /// One node's part in a query. `ok` false: `err` says why (no answer before the deadline, refused, unreachable).
            pub struct NodeAnswer { pub node: String, pub id: String, pub addr: String, pub ok: bool, pub hits: u32, pub took_ms: f64, pub err: String }
            /// `answered` of `asked` nodes answered; `text` is the answer as the `ce-search` CLI prints it.
            pub struct MeshResp { pub q: String, pub hits: Vec<Merged>, pub nodes: Vec<NodeAnswer>, pub answered: u32, pub asked: u32, pub took_ms: f64, pub text: String }
            pub struct SourceInfo { pub key: String, pub reads: String }
            pub enum SearchError { Invalid(String), Limited(String), Failed(String) }
        }
        ops {
            1 => HealthOp(()) -> Health, needs "search.mesh:health";
            // This node's own index only; never asks another node.
            2 => Local(LocalReq) -> Result<LocalResp, SearchError>, needs "search.mesh:local", deadline_ms 5000;
            // Every reachable node's Local at once, merged.
            3 => Mesh(MeshReq) -> Result<MeshResp, SearchError>, needs "search.mesh:find", deadline_ms 10000;
            4 => Sources(()) -> Vec<SourceInfo>, needs "search.mesh:read";
        }
    }
}

/// ref.resolve's contract (repo:live-refs contracts/ref_resolve.rs, major 1), copied verbatim like door_api.
#[allow(dead_code)]
mod ref_resolve_api {
    ce_wire::api! {
    api RefResolve = "ref.resolve", version 1;
    types {
        /// Health contract: these three fields, in this order, in every cell.
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        pub struct ResolveReq { pub reference: String }
        /// One place the thing is now: the node, its key, and what it holds there (a content hash for bytes).
        pub struct Holder { pub node: String, pub node_key: String, pub sha256: Option<String>, pub size: u64, pub since_ms: u64, pub live: bool }
        /// `source` is "mesh" when a holder answered now, "local index" when none could be reached; `as_of_ms` is
        /// then the time of the newest announcement the local index holds.
        pub struct Located { pub id: String, pub kind: String, pub holders: Vec<Holder>, pub pin: Option<String>, pub source: String, pub as_of_ms: u64, pub detail: String }
        pub enum Resolution { Located(Located), Ambiguous { written: String, reason: String, candidates: Vec<String> }, Unknown { written: String, reason: String } }
        pub struct Announce { pub id: String, pub kind: String, pub node: String, pub sha256: Option<String>, pub size: u64, pub at_ms: u64, pub gone: bool, pub detail: String }
        pub enum RefError { Invalid(String), Unavailable(String) }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "ref.resolve:health";
        2 => Resolve(ResolveReq) -> Result<Resolution, RefError>, needs "ref.resolve:read";
        3 => Publish(Announce) -> Result<u64, RefError>, needs "ref.resolve:announce";
    }
    }
}

/// ref.pin's contract (repo:live-refs contracts/ref_pin.rs, major 1), copied verbatim like door_api.
#[allow(dead_code)]
mod ref_pin_api {
    ce_wire::api! {
    api RefPin = "ref.pin", version 1;
    types {
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        pub struct PinReq { pub reference: String }
        pub struct PinnedRef { pub reference: String, pub sha256: String, pub size: u64, pub node: String }
        pub struct VerifyReq { pub pinned: String }
        pub enum Verdict { Ok { sha256: String }, Refused { want: String, got: String } }
        pub struct FetchReq { pub pinned: String }
        pub enum Fetched { Ok { sha256: String, bytes: Vec<u8> }, Refused { want: String, got: String } }
        pub struct PutReq { pub id: String, pub bytes: Vec<u8> }
        pub enum PinError { Invalid(String), NotHere(String) }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "ref.pin:health";
        2 => Pin(PinReq) -> Result<PinnedRef, PinError>, needs "ref.pin:read";
        3 => Verify(VerifyReq) -> Result<Verdict, PinError>, needs "ref.pin:read";
        4 => Fetch(FetchReq) -> Result<Fetched, PinError>, needs "ref.pin:read";
        5 => Put(PutReq) -> Result<PinnedRef, PinError>, needs "ref.pin:write";
    }
    }
}

/// The most of a file the viewer shows.
const VIEW_MAX: usize = 512 * 1024;

/// One file through the cell reference system, as the viewer: ref.resolve says where `repo:<repo>/<path>@<commit>`
/// lives and its content hash, ref.pin hands over only the bytes with that hash. No commit: the HEAD of `~/dev/<repo>`
/// here, the checkout both cells read on this node. -> (status, body).
async fn view_ref(person: &str, abilities: &[String], r: &str) -> (&'static str, serde_json::Value) {
    use serde_json::json;
    let Some((repo, path, commit)) = parse_view_ref(r) else {
        return ("400 Bad Request", json!({"error": "ref: repo:<repo>/<path>[@<commit>]"}));
    };
    if for_cell(abilities, "ref.resolve", "ref.resolve:read").is_none() || for_cell(abilities, "ref.pin", "ref.pin:read").is_none() {
        return ("403 Forbidden", json!({"error": "the file viewer is not open to you"}));
    }
    let commit = match commit {
        Some(c) => c,
        None => {
            let o = tokio::process::Command::new("git").arg("-C").arg(format!("{}/dev/{repo}", home())).args(["rev-parse", "HEAD"]).output().await;
            match o {
                Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
                _ => return ("404 Not Found", json!({"error": format!("{repo} has no checkout on {} (where ref.pin reads)", node_name())})),
            }
        }
    };
    let written = format!("repo:{repo}/{path}@{commit}");
    let rc = match client::<ref_resolve_api::RefResolve>(person, abilities).await {
        Ok(c) => c,
        Err(e) => return ("503 Service Unavailable", json!({"error": format!("ref.resolve: {e}")})),
    };
    let loc = match rc.call::<ref_resolve_api::Resolve>(&ref_resolve_api::ResolveReq { reference: written.clone() }).await {
        Ok(Ok(ref_resolve_api::Resolution::Located(l))) => l,
        Ok(Ok(ref_resolve_api::Resolution::Unknown { reason, .. })) => return ("404 Not Found", json!({"error": reason, "ref": written})),
        Ok(Ok(ref_resolve_api::Resolution::Ambiguous { reason, candidates, .. })) => return ("409 Conflict", json!({"error": reason, "ref": written, "candidates": candidates})),
        Ok(Err(e)) => return ("503 Service Unavailable", json!({"error": format!("ref.resolve: {e:?}")})),
        Err(e) => return ("503 Service Unavailable", json!({"error": format!("ref.resolve: {e}")})),
    };
    let Some(pin) = loc.pin.clone() else { return ("404 Not Found", json!({"error": "ref.resolve gave no content hash", "ref": written})) };
    let pinned = format!("{written} {pin}");
    let pc = match client::<ref_pin_api::RefPin>(person, abilities).await {
        Ok(c) => c,
        Err(e) => return ("503 Service Unavailable", json!({"error": format!("ref.pin: {e}")})),
    };
    let bytes = match pc.call::<ref_pin_api::Fetch>(&ref_pin_api::FetchReq { pinned: pinned.clone() }).await {
        Ok(Ok(ref_pin_api::Fetched::Ok { bytes, .. })) => bytes,
        Ok(Ok(ref_pin_api::Fetched::Refused { want, got })) => return ("409 Conflict", json!({"error": format!("changed since it was resolved: want {want}, got {got}; open it again"), "ref": pinned})),
        Ok(Err(e)) => return ("404 Not Found", json!({"error": format!("ref.pin: {e:?}"), "ref": pinned})),
        Err(e) => return ("503 Service Unavailable", json!({"error": format!("ref.pin: {e}")})),
    };
    let size = bytes.len();
    let binary = bytes.iter().take(8192).any(|b| *b == 0);
    let text = if binary { String::new() } else { String::from_utf8_lossy(&bytes[..size.min(VIEW_MAX)]).into_owned() };
    let holder = loc.holders.first().map(|h| h.node.clone()).unwrap_or_default();
    ("200 OK", json!({"ref": pinned, "id": loc.id, "repo": repo, "path": path, "commit": commit, "node": holder, "source": loc.source,
        "sha256": pin, "size": size, "binary": binary, "truncated": !binary && size > VIEW_MAX, "text": text,
        "via": ["ref.resolve", "ref.pin"]}))
}

fn node_name() -> String {
    std::fs::read_to_string("/etc/hostname").unwrap_or_default().trim().to_string()
}

/// One search over both: search.hybrid as the viewer (their exact private reads ride in the minted grant) and
/// search.mesh's merged answer from every node, at once. A side that fails is named in `notes`, never the whole answer.
/// search.mesh is asked only for a viewer holding `search.mesh:find`: its nodes index internal repos and docs.
/// Each group keeps its own `k` (Private, Network, Public): for a viewer with private reads search.hybrid is asked
/// twice, once as them and once with `search.hybrid:find` alone (public only), so their private hits never fill Public.
async fn search(person: &str, abilities: &[String], q: &str, k: u32) -> serde_json::Value {
    use serde_json::json;
    let t0 = Instant::now();
    let find = |ab: Vec<String>, k: u32| {
        let q = q.to_string();
        async move {
            for_cell(&ab, "search.hybrid", "search.hybrid:find").ok_or("not open to you".to_string())?;
            let c = client::<hybrid_api::SearchHybrid>(person, &ab).await?;
            let r = c.call::<hybrid_api::Find>(&hybrid_api::FindReq { q, k, sources: vec![], rerank: true }).await.map_err(|e| e.to_string())?;
            r.map_err(|e| format!("{e:?}"))
        }
    };
    let pub_ab = public_only(abilities);
    // as the viewer: enough to fill Private after the public hits in it are taken out
    let hybrid = find(abilities.to_vec(), if pub_ab.is_some() { (k * 2).min(50) } else { k });
    let public_ask = async {
        match pub_ab.clone() {
            Some(ab) => Some(find(ab, k).await),
            None => None,
        }
    };
    let mesh = async {
        for_cell(abilities, "search.mesh", "search.mesh:find").ok_or("not open to you".to_string())?;
        let c = client::<mesh_api::SearchMesh>(person, abilities).await?;
        let r = c.call::<mesh_api::Mesh>(&mesh_api::MeshReq { q: q.into(), k, deadline_ms: 2500, sources: vec![] }).await.map_err(|e| e.to_string())?;
        r.map_err(|e| format!("{e:?}"))
    };
    let (h, p, m) = tokio::join!(
        tokio::time::timeout(Duration::from_secs(22), hybrid),
        tokio::time::timeout(Duration::from_secs(22), public_ask),
        tokio::time::timeout(Duration::from_secs(12), mesh)
    );
    let (mut private, mut network, mut public, mut notes) = (vec![], vec![], vec![], vec![]);
    let hrow = |x: &hybrid_api::Hit| {
        json!({"title": x.title, "snippet": x.snippet, "source": x.source, "machine": x.machine, "path": x.path,
            "score": x.score, "open": open_of(&x.source, "", &x.path, ""), "from": "search.hybrid"})
    };
    match h {
        Ok(Ok(r)) => {
            notes.push(json!({"cell": "search.hybrid", "ok": true, "hits": r.hits.len(), "took_ms": r.took_ms as u64, "reranked": r.reranked}));
            for x in &r.hits {
                if group_of(&x.acl) == "public" { public.push(hrow(x)) } else { private.push(hrow(x)) }
            }
        }
        Ok(Err(e)) => notes.push(json!({"cell": "search.hybrid", "ok": false, "err": e.chars().take(200).collect::<String>()})),
        Err(_) => notes.push(json!({"cell": "search.hybrid", "ok": false, "err": "no answer in 22 s"})),
    }
    match p {
        Ok(None) => {}
        Ok(Some(Ok(r))) => {
            notes.push(json!({"cell": "search.hybrid", "ask": "public", "ok": true, "hits": r.hits.len(), "took_ms": r.took_ms as u64}));
            // the public-only answer first: it is ranked among public hits alone
            let mut both: Vec<serde_json::Value> = r.hits.iter().filter(|x| group_of(&x.acl) == "public").map(hrow).collect();
            both.append(&mut public);
            public = both;
        }
        Ok(Some(Err(e))) => notes.push(json!({"cell": "search.hybrid", "ask": "public", "ok": false, "err": e.chars().take(200).collect::<String>()})),
        Err(_) => notes.push(json!({"cell": "search.hybrid", "ask": "public", "ok": false, "err": "no answer in 22 s"})),
    }
    match m {
        Ok(Ok(r)) => {
            notes.push(json!({"cell": "search.mesh", "ok": true, "hits": r.hits.len(), "took_ms": r.took_ms as u64, "answered": r.answered, "asked": r.asked,
                "nodes": r.nodes.iter().map(|n| json!({"node": n.node, "ok": n.ok, "hits": n.hits})).collect::<Vec<_>>()}));
            for x in r.hits {
                let first = x.locations.first();
                let (machine, path) = first.map(|l| (l.node.clone(), l.path.clone())).unwrap_or_default();
                network.push(json!({"title": x.title, "snippet": x.snippet, "source": x.sources.join(", "), "kind": x.kind, "machine": machine, "path": path,
                    "also": x.locations.iter().skip(1).map(|l| l.node.clone()).collect::<Vec<_>>(), "score": x.score,
                    "open": open_of("", &x.kind, &path, &x.key), "origin": if x.kind == "repo" { origin_of(&x.key) } else { None }, "from": "search.mesh"}));
            }
        }
        Ok(Err(e)) => notes.push(json!({"cell": "search.mesh", "ok": false, "err": e.chars().take(200).collect::<String>()})),
        Err(_) => notes.push(json!({"cell": "search.mesh", "ok": false, "err": "no answer in 12 s"})),
    }
    let key = |v: &serde_json::Value| format!("{}\n{}\n{}", v["path"], v["title"], v["machine"]);
    let n = k as usize;
    let (mut private, mut network, mut public) = (first_unique(private, n, key), first_unique(network, n, key), first_unique(public, n, key));
    pin_links(&mut [&mut private, &mut network, &mut public]).await;
    json!({"q": q, "per_group": k, "private": private, "network": network, "public": public,
        "notes": notes, "took_ms": t0.elapsed().as_millis() as u64})
}

/// Every viewer link (`/search/?ref=repo:<repo>/<path>`) pinned to what the viewer can open: ref.pin reads a repo
/// file at a commit from this node's checkout `~/dev/<repo>`, so a link is kept, as `repo:<repo>/<path>@<HEAD>`, only
/// when that checkout holds the file at HEAD (one `git rev-parse` and one `git cat-file --batch-check` per repo). Any
/// other row opens at its real origin when search.mesh gave one (`origin`), else has no link: never a dead or guessed
/// one (a copy only in search.hybrid's mirror of another machine is not a checkout ref.pin can read).
async fn pin_links(groups: &mut [&mut Vec<serde_json::Value>]) {
    let mut want: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for g in groups.iter() {
        for row in g.iter() {
            if let Some((repo, path, _)) = row["open"].as_str().and_then(|o| o.strip_prefix("/search/?ref=")).and_then(parse_view_ref) {
                want.entry(repo).or_default().push(path);
            }
        }
    }
    let dev = format!("{}/dev", home());
    let held: BTreeMap<String, (String, std::collections::BTreeSet<String>)> = tokio::task::spawn_blocking(move || {
        use std::io::Write;
        let mut out = BTreeMap::new();
        for (repo, paths) in want {
            let dir = format!("{dev}/{repo}");
            if !std::path::Path::new(&dir).join(".git").exists() {
                continue;
            }
            let Ok(o) = std::process::Command::new("git").args(["-C", &dir, "rev-parse", "HEAD"]).output() else { continue };
            let commit = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !o.status.success() || commit.len() != 40 {
                continue;
            }
            let Ok(mut c) = std::process::Command::new("git")
                .args(["-C", &dir, "cat-file", "--batch-check=%(objecttype)"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
            else {
                continue;
            };
            if let Some(mut i) = c.stdin.take() {
                for p in &paths {
                    let _ = writeln!(i, "{commit}:{p}");
                }
            }
            let Ok(o) = c.wait_with_output() else { continue };
            let ok: std::collections::BTreeSet<String> =
                paths.iter().zip(String::from_utf8_lossy(&o.stdout).lines()).filter(|(_, l)| *l == "blob").map(|(p, _)| p.clone()).collect();
            out.insert(repo, (commit, ok));
        }
        out
    })
    .await
    .unwrap_or_default();
    for g in groups.iter_mut() {
        for row in g.iter_mut() {
            let Some((repo, path, _)) = row["open"].as_str().and_then(|o| o.strip_prefix("/search/?ref=")).and_then(parse_view_ref) else { continue };
            match held.get(&repo).filter(|(_, ok)| ok.contains(&path)) {
                Some((commit, _)) => row["open"] = format!("/search/?ref=repo:{repo}/{path}@{commit}").into(),
                None => row["open"] = row.get("origin").cloned().unwrap_or(serde_json::Value::Null),
            }
        }
    }
}

/// One viewer's grant, minted by the node through wire.ws-door: its key and chain, for exactly `abilities`.
struct ViewerGrant {
    abilities: Vec<String>,
    key: ce_gate::ce_identity::Identity,
    chain: Vec<ce_gate::ce_cap::SignedCapability>,
    not_after: u64,
}

type Grants = Arc<RwLock<BTreeMap<String, Arc<ViewerGrant>>>>;

static GRANTS: std::sync::OnceLock<Grants> = std::sync::OnceLock::new();

fn b64u(s: &str) -> Option<Vec<u8>> {
    let v = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        } as u32)
    };
    let s = s.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for ch in s.chunks(4) {
        let mut n = 0u32;
        for (i, c) in ch.iter().enumerate() {
            n |= v(*c)? << (18 - 6 * i);
        }
        out.extend_from_slice(&n.to_be_bytes()[1..ch.len()]);
    }
    Some(out)
}

const VIEWER_TTL_S: u32 = 900;

/// The viewer's grant for these abilities: the cached one while it has 2 minutes left and the same abilities (a
/// people-file change mints a new one at once), else a fresh Mint.
async fn viewer(person: &str, abilities: &[String]) -> Result<Arc<ViewerGrant>, String> {
    use door_api::*;
    let want = viewer_grant(abilities);
    if want.is_empty() {
        return Err("refused".into());
    }
    let grants = GRANTS.get_or_init(Arc::default).clone();
    let slot = format!("{person}\n{}", want.join(","));
    if let Some(g) = grants.read().await.get(&slot) {
        if g.abilities == want && g.not_after > now_s() + 120 {
            return Ok(g.clone());
        }
    }
    let mine: Vec<&str> = GRANT.iter().copied().filter(|a| a.starts_with("wire.ws-door:")).collect();
    let tok = ui_chain::source_token::<WireWsDoor>(&mine, 120, None)?;
    let mut c = tokio::time::timeout(Duration::from_secs(4), ce_wire::Client::<WireWsDoor>::by_name(tok)).await.map_err(|_| "wire.ws-door: no answer in 4 s".to_string())?.map_err(|e| format!("wire.ws-door: {e}"))?;
    c.deadline = Duration::from_secs(8);
    let m = c
        .call::<Mint>(&MintReq { page: "https://apps.ce-net.com/approvals/".into(), abilities: want.clone(), ttl_s: VIEWER_TTL_S })
        .await
        .map_err(|e| format!("wire.ws-door: {e}"))?
        .map_err(|e| format!("wire.ws-door: {e:?}"))?;
    let g = m.link.split_once("#g=").map(|(_, f)| f).ok_or("wire.ws-door: no grant in the link")?;
    let (seed, chain) = g.split_once('.').ok_or("wire.ws-door: a grant without a chain")?;
    let seed: [u8; 32] = b64u(seed).and_then(|b| b.try_into().ok()).ok_or("wire.ws-door: a bad key")?;
    let chain = ce_gate::ce_cap::decode_chain_bytes(&b64u(chain).ok_or("wire.ws-door: a bad chain")?).map_err(|e| format!("{e:?}"))?;
    let vg = Arc::new(ViewerGrant { abilities: want, key: ce_gate::ce_identity::Identity::from_secret_bytes(&seed), chain, not_after: m.not_after });
    eprintln!("apps-shell: minted a viewer grant for {person}: {} abilities, until {}", vg.abilities.len(), vg.not_after);
    {
        let mut w = grants.write().await;
        w.retain(|_, g| g.not_after > now_s());
        w.insert(slot, vg.clone());
    }
    Ok(vg)
}

/// A client of `A` as the viewer (their minted grant), found by name.
async fn client<A: ce_wire::Api>(person: &str, abilities: &[String]) -> Result<ce_wire::Client<A>, String> {
    let g = viewer(person, abilities).await?;
    let tok = ce_gate::proof::<A>(&g.key, &g.chain);
    let mut c = tokio::time::timeout(Duration::from_secs(4), ce_wire::Client::<A>::by_name(tok)).await.map_err(|_| "no replica answered in 4 s".to_string())?.map_err(|e| e.to_string())?;
    c.deadline = Duration::from_secs(10);
    Ok(c)
}

async fn badge(person: &str, abilities: &[String]) -> Option<u32> {
    use approve_inbox_api as ai;
    for_cell(abilities, "approve.inbox", "approve.inbox:read")?;
    let c = client::<ai::ApproveInbox>(person, abilities).await.ok()?;
    c.call::<ai::Badge>(&()).await.ok().map(|w| w.waiting)
}

async fn tabs(f: &Face, abilities: &[String], person: &str) -> TabsOut {
    let live = f.live.read().await.clone();
    let rows = tabs_for(abilities, &live, ui_chain::now_ms());
    let mut tabs = Vec::new();
    for t in rows {
        let b = if t.system == "APPROVALS" { badge(person, abilities).await } else { None };
        tabs.push(TabOut { view: view_of(&t.route).map(String::from), system: t.system, title: t.title, route: t.route, badge: b });
    }
    TabsOut { person: person.into(), here: f.route.clone(), tabs, at_ms: ui_chain::now_ms() }
}

async fn api(f: &Face, person: &str, route: &str, path: &str, abilities: &[String]) -> (&'static str, Vec<u8>) {
    use approve_inbox_api as ai;
    use site_cellmap_api as sc;
    let refused = ("403 Forbidden", b"{\"error\":\"refused\"}".to_vec());
    let down = |e: String| ("503 Service Unavailable", json(&serde_json::json!({ "error": e.chars().take(300).collect::<String>() })));
    // a viewer may call a tab's data only while that tab is theirs (allowed and live)
    let live = f.live.read().await.clone();
    let mine = tabs_for(abilities, &live, ui_chain::now_ms());
    let has = |r: &str| mine.iter().any(|t| t.route == r);
    match route {
        "/api/inbox" | "/api/card" if has("/approvals/") => {
            if for_cell(abilities, "approve.inbox", "approve.inbox:read").is_none() {
                return refused;
            }
            let c = match client::<ai::ApproveInbox>(person, abilities).await {
                Ok(c) => c,
                Err(e) => return down(e),
            };
            if route == "/api/inbox" {
                let q = ai::InboxQuery { about: query(path, "about"), kind: query(path, "kind") };
                match c.call::<ai::List>(&q).await {
                    Ok(rows) => ("200 OK", json(&rows)),
                    Err(e) => down(e.to_string()),
                }
            } else {
                let id = query(path, "b").unwrap_or_default();
                match c.call::<ai::Bundle>(&id).await {
                    Ok(Ok(card)) => ("200 OK", json(&card)),
                    Ok(Err(_)) => ("404 Not Found", b"{\"error\":\"no such bundle\"}".to_vec()),
                    Err(e) => down(e.to_string()),
                }
            }
        }
        "/api/cellmap" if has("/core/") => {
            if for_cell(abilities, "site.cellmap", "site.cellmap:read").is_none() {
                return refused;
            }
            match client::<sc::SiteCellmap>(person, abilities).await {
                Ok(c) => match c.call::<sc::Map>(&()).await {
                    Ok(m) => ("200 OK", json(&m)),
                    Err(e) => down(e.to_string()),
                },
                Err(e) => down(e),
            }
        }
        "/api/search" if has("/search/") => {
            let q = query_text(path, "q").unwrap_or_default();
            let q = q.trim();
            if q.is_empty() || q.chars().count() > 300 {
                return ("400 Bad Request", b"{\"error\":\"q: 1 to 300 characters\"}".to_vec());
            }
            let k = query(path, "k").and_then(|k| k.parse().ok()).unwrap_or(PER_GROUP).clamp(1, 50);
            ("200 OK", json(&search(person, abilities, q, k).await))
        }
        "/api/ref" if has("/search/") => {
            let r = query_text(path, "r").unwrap_or_default();
            let (st, body) = view_ref(person, abilities, r.trim()).await;
            (st, json(&body))
        }
        _ => refused,
    }
}

async fn one(f: Arc<Face>, mut s: TcpStream) {
    let mut buf = Vec::with_capacity(2048);
    loop {
        let mut chunk = [0u8; 2048];
        let n = match tokio::time::timeout(Duration::from_secs(20), s.read(&mut chunk)).await {
            Ok(Ok(n)) if n > 0 => n,
            _ => return,
        };
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|x| x == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 16 * 1024 {
            return reply(&mut s, "431 Request Header Fields Too Large", "text/plain", b"", false).await;
        }
    }
    let head = String::from_utf8_lossy(&buf).to_string();
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or("/").to_string());
    let is_head = method == "HEAD";
    if method != "GET" && !is_head {
        return reply(&mut s, "405 Method Not Allowed", "text/plain", b"", false).await;
    }
    let route = path.split(['?', '#']).next().unwrap_or("/").to_string();
    if route == "/" || route == "/index.html" {
        return reply(&mut s, "200 OK", "text/html; charset=utf-8", file("index.html").unwrap_or_default(), is_head).await;
    }
    if let Some(k) = route.strip_prefix("/_k/") {
        return match file(&format!("_k/{k}")) {
            Some(b) => reply(&mut s, "200 OK", kind(k), b, is_head).await,
            None => reply(&mut s, "404 Not Found", "text/plain", b"no such file", is_head).await,
        };
    }
    let person = site_gateway::read_key(&f.person_key).and_then(|k| site_gateway::signed_person(&head, &k, now_s())).unwrap_or_default();
    if person.is_empty() {
        return reply(&mut s, "401 Unauthorized", "application/json", b"{\"error\":\"sign in first\"}", is_head).await;
    }
    let ab = f.abilities(&person);
    if route == "/tabs.json" {
        let t = tabs(&f, &ab, &person).await;
        eprintln!("apps-shell: {person} {}: {} tab(s)", f.route, t.tabs.len());
        return reply(&mut s, "200 OK", "application/json", &json(&t), is_head).await;
    }
    if let Some(v) = route.strip_prefix("/v/") {
        let live = f.live.read().await.clone();
        let mine = tabs_for(&ab, &live, ui_chain::now_ms());
        let name = format!("v/{v}");
        // the same 404 for a view that does not exist and one this viewer may not have
        return match mine.iter().any(|t| view_of(&t.route) == Some(name.as_str())).then(|| file(&name)).flatten() {
            Some(b) => reply(&mut s, "200 OK", "text/javascript; charset=utf-8", b, is_head).await,
            None => reply(&mut s, "404 Not Found", "text/plain", b"no such file", is_head).await,
        };
    }
    if route.starts_with("/api/") {
        let (st, body) = api(&f, &person, &route, &path, &ab).await;
        return reply(&mut s, st, "application/json", &body, is_head).await;
    }
    reply(&mut s, "404 Not Found", "text/plain", b"no such page", is_head).await
}

async fn listen(addr: String, f: Arc<Face>) -> Result<(), String> {
    let sa: std::net::SocketAddr = addr.parse().map_err(|e| format!("{addr}: {e}"))?;
    if !sa.ip().is_loopback() {
        return Err(format!("{addr}: the face binds loopback only (the sign-in gate is in front)"));
    }
    let sock = tokio::net::TcpSocket::new_v4().map_err(|e| e.to_string())?;
    sock.set_reuseaddr(true).map_err(|e| e.to_string())?;
    // a restart starts the new process before the old one stops (cell.install Restart): both hold the port meanwhile
    sock.set_reuseport(true).map_err(|e| e.to_string())?;
    sock.bind(sa).map_err(|e| format!("{addr}: {e}"))?;
    let l: TcpListener = sock.listen(1024).map_err(|e| e.to_string())?;
    loop {
        let (s, _) = l.accept().await.map_err(|e| e.to_string())?;
        tokio::spawn(one(f.clone(), s));
    }
}

async fn serve() -> Result<(), String> {
    let me = ui_chain::init(CELL)?.root();
    let started = Instant::now();
    let live: Live = Arc::default();
    tokio::spawn(liveness(live.clone()));
    let person_key: PathBuf = env_or("CE_GATE_PERSON_KEY", format!("{}/.config/work-show/person-key", home())).into();
    let people: PathBuf = env_or("APPS_SHELL_PEOPLE", format!("{}/.config/apps-shell/people.json", home())).into();
    for pair in env_or("APPS_SHELL_HTTP", "127.0.0.1:8881=/approvals/,127.0.0.1:8882=/core/,127.0.0.1:8883=/search/".into()).split(',') {
        let (addr, route) = pair.split_once('=').ok_or_else(|| format!("APPS_SHELL_HTTP: {pair}: want <addr>=<route>"))?;
        let f = Arc::new(Face { route: route.into(), live: live.clone(), person_key: person_key.clone(), people: people.clone() });
        let addr = addr.to_string();
        tokio::spawn(async move {
            if let Err(e) = listen(addr, f).await {
                eprintln!("apps-shell http: {e}");
                std::process::exit(1)
            }
        });
    }
    let (l1, p1) = (live.clone(), people.clone());
    let gate = ce_gate::Gate::<AppsShell>::new(me, ce_gate::mesh_roots()).into_fn();
    Server::<AppsShell>::new(gate)
        .on::<HealthOp, _, _>(move |_: (), _| async move { Ok(Health { ok: true, version: env!("CARGO_PKG_VERSION").into(), uptime_s: started.elapsed().as_secs() }) })
        .on::<Tabs, _, _>(move |_: (), who| {
            let (live, _people) = (l1.clone(), p1.clone());
            async move {
                // a typed caller's own chain is its abilities (no people file): what its grant says it holds
                let ab: Vec<String> = who.rights.iter().map(|r| r.strip_prefix(ce_wire::ABILITY_RIGHT).unwrap_or(r).to_string()).collect();
                Ok(tabs_for(&ab, &*live.read().await, ui_chain::now_ms()))
            }
        })
        .audit(ce_gate::audit::file())
        .serve_door()
        .await
        .map_err(|e| e.to_string())
}

fn main() {
    let rt = || tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| e.to_string());
    let r = match std::env::args().nth(1).as_deref() {
        Some("serve") => rt().and_then(|rt| rt.block_on(serve())),
        Some("live") => rt().and_then(|rt| {
            rt.block_on(async {
                ui_chain::init(CELL)?;
                for (sys, t) in all_tabs() {
                    let t0 = Instant::now();
                    let ok = shell_ok(t.shell.name).await;
                    println!("{sys:<14} {:<14} {:<16} {} {} ms", t.title, t.shell.name, if ok { "live" } else { "DOWN" }, t0.elapsed().as_millis());
                }
                Ok(())
            })
        }),
        _ => Err("usage: apps-shell serve | live".into()),
    };
    if let Err(e) = r {
        eprintln!("apps-shell: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod contracts {
    const REF_RESOLVE: &str = "0aa2e2474931699b";
    const REF_PIN: &str = "966357c40f1b9fff";
    /// The copies must hash as the live cells do, or every call is refused as another api.
    #[test]
    fn copied_search_contracts_match_the_live_cells() {
        assert_eq!(<super::hybrid_api::SearchHybrid as ce_wire::Api>::SCHEMA, 0x2e6c7e98274d467f);
        assert_eq!(<super::mesh_api::SearchMesh as ce_wire::Api>::SCHEMA, 0xb78f3a1097281268);
        assert_eq!(format!("{:016x}", <super::ref_resolve_api::RefResolve as ce_wire::Api>::SCHEMA), REF_RESOLVE);
        assert_eq!(format!("{:016x}", <super::ref_pin_api::RefPin as ce_wire::Api>::SCHEMA), REF_PIN);
    }
}

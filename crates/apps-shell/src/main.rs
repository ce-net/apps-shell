//! `apps-shell serve`: the apps.shell cell behind this machine's door, its liveness loop, and its gateway face.
//!
//! The face listens on loopback once per tab route it serves (`$APPS_SHELL_HTTP`, default
//! `127.0.0.1:8881=/approvals/,127.0.0.1:8882=/core/`); the apps.ce-net.com sign-in gate sends `/<route>*` there with
//! the prefix cut and the person signed (`X-Ce-Person*`, the gate's per-host key, repo:ui-cells site-gateway's check).
//! - `GET /` the page (the same bytes for everyone; it names no system), `/_k/*` the kit;
//! - `GET /tabs.json` this viewer's tabs: allowed by their abilities and live (their shell answered HealthOp by name
//!   within LIVE_MS; asked every PROBE_MS); a tab's `view` only when this site carries it;
//! - `GET /v/<tab>.js` a tab's view module, only to a viewer whose tabs include that tab (404 to anyone else);
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
            let mut c = tokio::time::timeout(Duration::from_millis(1500), ce_wire::Client::<$api>::by_name(tok)).await.ok()?.ok()?;
            c.deadline = Duration::from_millis(1500);
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
    if let Some(g) = grants.read().await.get(person) {
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
    grants.write().await.insert(person.to_string(), vg.clone());
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
    for pair in env_or("APPS_SHELL_HTTP", "127.0.0.1:8881=/approvals/,127.0.0.1:8882=/core/".into()).split(',') {
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

//! `apps-shell serve`: the apps.shell cell behind this machine's door, its liveness loop, and its gateway face.
//!
//! The face listens on loopback once per tab route it serves (`$APPS_SHELL_HTTP`, default
//! `127.0.0.1:8881=/approvals/,127.0.0.1:8882=/core/,127.0.0.1:8883=/search/,127.0.0.1:8884=/feed/,127.0.0.1:8885=/home/,127.0.0.1:8886=/conversations/,127.0.0.1:8887=/architecture/`); the apps.ce-net.com sign-in gate sends `/<route>*` there with
//! the prefix cut and the person signed (`X-Ce-Person*`, the gate's per-host key, repo:ui-cells site-gateway's check).
//! - `GET /` the page (the same bytes for everyone; it names no system), `/_k/*` the kit;
//! - `GET /tabs.json` this viewer's tabs: allowed by their abilities and live (their shell answered HealthOp by name
//!   within LIVE_MS; asked every PROBE_MS); a tab's `view` only when this site carries it;
//! - `GET /v/<tab>.js` a tab's view module, only to a viewer whose tabs include that tab (404 to anyone else);
//! - `GET /api/search?q=<words>`: the Search tab, search.hybrid `Find` and search.mesh `Mesh` at once, in three groups:
//!   `private` (search.hybrid by the viewer's exact read grants), `network` (search.mesh: every node's own index of
//!   repos, cells and docs; internal, only with `search.mesh:find`) and `public` (what cells published for everyone);
//!   each group its own [`PER_GROUP`] (default 10, `k=` 1 to 50): an owner's private hits never push Public to 0;
//! - `GET /api/rec?r=<person|promise|call|meet>:<id>[&q=<words>]`: the Search tab's record screen for a CRM or call
//!   hit, read as the viewer from the cell that owns it (crm.people, crm.promises, crm.calls `Get`); a call recording
//!   (`meet:`) shows search.hybrid's passage of it, asked as the viewer with `search.hybrid:read:calls`;
//! - `GET /api/ref?r=repo:<repo>/<path>[@<commit>]`: the Search tab's file viewer, through the cell reference system
//!   as the viewer: ref.resolve `Resolve` (where it lives, its ce-ref:// id and content hash) and ref.pin `Fetch` (only
//!   the bytes with that hash); with no commit, the HEAD of the checkout those cells read on this node;
//! - the Feed tab (`/feed/`): `GET /v/feed.js` is the view module the feed cell itself ships (its op `Ui`, asked as the
//!   viewer; 404 to a viewer without the tab), `GET /_c/<sha256>.js` its immutable cached copy, and `POST
//!   /api/cell/<cell>/<Op>` the data relay for the ops Page, Topic, Changes, Mark and Close only: each is called with a
//!   grant wire.ws-door minted for the viewer (their own `feed.view:*` rights and a `feed.view:where:person=<them>`
//!   caveat), so the feed cell's own gate decides, never this one;
//! - `GET /o/<ce-ref>`: the same page, with the pane open on that ref (the ce-net browser's transport on this host);
//! - `GET /api/open?r=<ce-ref>&mode=view&device=Desk`: open.with `Open` as the viewer (repo:open-with): the view that
//!   opens it for them, a refusal naming the right and an ask ref, no opener, or the resolver's reason;
//! - `GET /api/layout?device=&from=&to=`: the Home tab, home.shell `Layout` as the viewer, then each slot's `when` op
//!   asked as the viewer at once (crm.calls today, crm.promises due, approve.inbox Badge): a slot with nothing for the
//!   viewer is dropped, one whose cell does not answer stays and shows its own state;
//! - `GET /api/calls?from=&to=`, `/api/promises`, `/api/links`: the Home slots' data, read as the viewer
//!   (crm.calls List, crm.promises List, the node's home-links rows);
//! - the Architecture view under Core (`/core/?view=architecture`, for a viewer holding `arch.map:see`): `GET
//!   /v/architecture.js` is the module arch.map ships (its `Ui`, as the viewer), `/_c/<sha256>.js` its copy, and `POST
//!   /api/cell/arch.map/<Op>` (Core and Architecture faces) relays GraphFull, DetailFull, Sources, Graph, Detail, Story,
//!   Progress and Changes, and `POST /api/cell/arch.notes/<Op>` the notes strip's Put, List and Close (the viewer's
//!   grant carries `arch.notes:where:author=<them>`); the Architecture tab (face 8887, `/architecture/`) serves the same
//!   module and relays, and links to `/core/?view=architecture` until the gate routes it; each with a grant wire.ws-door mints for
//!   the viewer, so arch.map's own gate decides (docs/ARCHITECTURE.md);
//! - `GET /api/decisions`: the Approvals tab's cards, decision.card `BoardOp` as the viewer (repo:decision-card): the few
//!   decisions the owner must answer, ranked, and how many asks closed with a receipt; the badge counts those cards;
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
        "home.shell" => health!(home_shell_api::HomeShell, home_shell_api::HealthOp, "home.shell:health"),
        "feed.view" => health!(feed_view_api::FeedView, feed_view_api::HealthOp, "feed.view:health"),
        "ai.session" => health!(ai_session_api::AiSession, ai_session_api::HealthOp, "ai.session:health"),
        "arch.map" => health!(arch_map_api::ArchMap, arch_map_api::HealthOp, "arch.map:health"),
        "person.now" => health!(person_now_api::PersonNow, person_now_api::HealthOp, "person.now:health"),
        "business.view" => health!(business_monitor_api::view::BusinessView, business_monitor_api::view::HealthOp, "business.view:health"),
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
    reply_cached(s, status, ty, body, head, "no-store").await
}

async fn reply_cached(s: &mut TcpStream, status: &str, ty: &str, body: &[u8], head: bool, cache: &str) {
    let csp = "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self' https://ce-net.com https://fonts.gstatic.com; img-src 'self' data:; connect-src 'self'; frame-src 'self' https://maps.ce-net.com; base-uri 'self'; form-action 'none'; frame-ancestors 'none'";
    let h = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ty}\r\nContent-Length: {}\r\nCache-Control: {cache}\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: {csp}\r\nVary: X-Ce-Person\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(h.as_bytes()).await;
    if !head {
        let _ = s.write_all(body).await;
    }
}

/// The kit's hash: every file of the site (16 hex), so a new build is a new address and an old one is never served
/// from a stale cache.
fn kit_hash() -> &'static str {
    static H: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    H.get_or_init(|| {
        // FNV-1a over names and bytes: the same build, the same hash on every node and every start
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for (n, b) in site::SITE.iter() {
            for x in n.as_bytes().iter().chain([0u8].iter()).chain(b.iter()) {
                h ^= u64::from(*x);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
        format!("{h:016x}")
    })
}

/// `/_h/<16 hex>/<rest>` -> (`/<rest>`, whether the hash is this build's kit); any other route as it is.
fn unhash(route: String) -> (String, bool) {
    match route.strip_prefix("/_h/").and_then(|r| r.split_once('/')) {
        Some((h, rest)) if h.len() == 16 && h.bytes().all(|b| b.is_ascii_hexdigit()) => (format!("/{rest}"), h == kit_hash()),
        _ => (route, false),
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
    /// where the tab opens while the gate does not serve its route (NOT_ROUTED_YET); None = its route
    href: Option<String>,
    view: Option<String>,
    badge: Option<u32>,
}

#[derive(serde::Serialize)]
struct TabsOut {
    person: String,
    here: String,
    /// the extra views of this tab open to this viewer (`?view=<name>`; EXTRA_VIEWS)
    views: Vec<String>,
    /// the notes cell the Architecture view may write to for this viewer (only with arch.notes:write)
    notes: Option<String>,
    tabs: Vec<TabOut>,
    at_ms: u64,
}

/// arch.notes's contract (repo:arch-notes crates/arch-notes-api, major 1, frozen whole at 534c49f), copied verbatim:
/// the notes strip's relay (0xa4dc01ad901bfcda, pinned by a test).
#[allow(dead_code)]
mod arch_notes_api {
    ce_wire::api! {
        api ArchNotes = "arch.notes", version 1;
        types {
            /// Health contract: these three fields, in this order, in every cell.
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            /// Hybrid logical clock: wall milliseconds, then a counter for writes in the same millisecond. Orders notes.
            pub struct Hlc { pub ms: u64, pub n: u32 }
            pub enum NoteState { Open, Taken, Closed }
            /// One note. `id` "" on a first Put: the cell names it. `target` is the atlas id the note is on; `section` a doc
            /// ce-ref with `#anchor` when the note is about a part of a document. `at`, `state`, `taken_by`, `closed_at`
            /// are the cell's: a Put sets `at`, keeps the stored state, and opens a new note.
            pub struct Note { pub id: String, pub target: String, pub section: Option<String>, pub text: String,
                              pub author: String, pub at: Hlc, pub state: NoteState, pub taken_by: Option<String>,
                              pub closed_at: Option<Hlc> }
            /// `target` None = every target; `open_only` leaves out Closed; `since` only notes changed after it; `max` 0 = 100
            /// (at most 500). Newest first.
            pub struct ListReq { pub target: Option<String>, pub open_only: bool, pub since: Option<Hlc>, pub max: u32 }
            pub struct Taken { pub id: String, pub by: String }
            /// `seq`: state.store's write sequence of the row.
            pub struct Stored { pub id: String, pub at: Hlc, pub seq: u64 }
            pub enum NoteError { Invalid(String), Denied(String), NoSuchNote(String), Store(String) }
        }
        ops {
            1 => HealthOp(()) -> Health,                     needs "arch.notes:health";
            2 => Put(Note) -> Result<Stored, NoteError>,     needs "arch.notes:write";   // where author == caller
            3 => List(ListReq) -> Vec<Note>,                 needs "arch.notes:read";
            4 => Take(Taken) -> Result<(), NoteError>,       needs "arch.notes:take";    // a lane claims it
            5 => Close(String) -> Result<(), NoteError>,     needs "arch.notes:write";   // its author, or a holder of take
        }
        streams {
            6 => Watch(ListReq) -> stream Note,              needs "arch.notes:read";
        }
    }
}

/// arch.map's contract (repo:arch-map crates/arch-map-api, major 1, frozen whole at f5dc421), copied verbatim: another
/// hash and every call is refused as another api (0x934e4e8a827ab7ab, pinned by a test).
#[allow(dead_code)]
mod arch_map_api {
    ce_wire::api! {
        api ArchMap = "arch.map", version 1;
        types {
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            pub enum Lens { Layers, Systems, Machines, Caps, Pipeline, Migration }
            pub enum Class { Machine, Wire, Cell, System, Face, Cap, Piece }
            pub enum State { Live, Installed, Declared, Designed, Down, Retired }
            pub enum EdgeKind { Nest, Needs, Call, Runs, Offers, Holds, Serves, Next, Doc }
            pub struct View { pub lens: Lens, pub target: bool, pub root: Option<String>, pub filter: String,
                              pub max_nodes: u32 }
            pub struct GNode { pub id: String, pub class: Class, pub layer: u8, pub label: String, pub sub: String,
                               pub state: State, pub parent: Option<String>, pub cells: u32, pub live: u32,
                               pub want: u32, pub calls_min: u32, pub notes: u32, pub docs: u32 }
            pub struct GEdge { pub from: String, pub to: String, pub kind: EdgeKind, pub count: u32, pub label: String }
            pub struct LayerCount { pub layer: u8, pub name: String, pub live: u32, pub known: u32, pub target: u32 }
            pub struct Source { pub key: String, pub cell: String, pub op: String, pub ok: bool, pub rows: u32,
                                pub age_s: u64, pub pin: String, pub err: String }
            pub struct ArchGraph { pub seq: u64, pub as_of_ms: u64, pub nodes: Vec<GNode>, pub edges: Vec<GEdge>,
                               pub layers: Vec<LayerCount>, pub hidden: u32, pub sources_ok: u32, pub sources: u32 }
            pub struct OpRow { pub n: u32, pub name: String, pub req: String, pub resp: String, pub right: String }
            pub struct Place { pub node: String, pub class: String, pub processes: u32, pub sha256: String, pub seen_s: u64 }
            pub struct Caller { pub from: String, pub count: u32, pub window_s: u32 }
            pub struct DocRef { pub title: String, pub to: String, pub public: bool }
            pub struct DetailDoc { pub node: GNode, pub repo: String, pub owner: String, pub version: String,
                                   pub schema: String, pub ops: Vec<OpRow>, pub on: Vec<Place>, pub callers: Vec<Caller>,
                                   pub needs: Vec<String>, pub needed_by: Vec<String>, pub pieces: Vec<String>,
                                   pub docs: Vec<DocRef> }
            pub struct StoryStep { pub n: u32, pub title: String, pub caption: String, pub lens: Lens, pub target: bool,
                                   pub focus: Vec<String> }
            pub struct StepRow { pub id: String, pub title: String, pub state: State, pub probe: String,
                                 pub open: bool, pub doc: Option<DocRef> }
            pub struct Track { pub key: String, pub title: String, pub closed: u32, pub total: u32, pub steps: Vec<StepRow> }
            pub struct Measure { pub name: String, pub before: String, pub now: String, pub target: String }
            pub struct ArchProgress { pub tracks: Vec<Track>, pub measures: Vec<Measure> }
            pub struct ChangesReq { pub after: u64, pub view: View, pub wait_ms: u32 }
            pub struct Delta { pub seq: u64, pub put: Vec<GNode>, pub gone: Vec<String>, pub edges: Vec<GEdge>,
                               pub layers: Vec<LayerCount> }
            pub struct UiReq { pub view: String }
            pub struct Bundle { pub sha256: String, pub speaks: Vec<u16>, pub kit: u16, pub bytes: Vec<u8> }
            pub struct ImportReq { pub doc: String, pub dry: bool }
            pub struct Imported { pub pieces: u32, pub links: u32, pub changed: u32, pub skipped: Vec<String> }
            pub struct Alias { pub id: String, pub label: String, pub caption: String, pub show: bool }
            pub enum ArchError { Source(String), Store(String), NoSuchNode(String), NoSuchView(String), Invalid(String) }
        }
        ops {
            1  => HealthOp(()) -> Health,                               needs "arch.map:health";
            2  => Graph(View) -> Result<ArchGraph, ArchError>,              needs "arch.map:see";       // demo projection
            3  => Detail(String) -> Result<DetailDoc, ArchError>,       needs "arch.map:see";       // demo projection
            4  => Story(()) -> Result<Vec<StoryStep>, ArchError>,       needs "arch.map:see";
            5  => Progress(()) -> Result<ArchProgress, ArchError>,          needs "arch.map:see";       // demo projection
            6  => Ui(UiReq) -> Result<Bundle, ArchError>,               needs "arch.map:see";
            7  => GraphFull(View) -> Result<ArchGraph, ArchError>,          needs "arch.map:internal";
            8  => DetailFull(String) -> Result<DetailDoc, ArchError>,   needs "arch.map:internal";
            9  => ProgressFull(()) -> Result<ArchProgress, ArchError>,      needs "arch.map:internal";
            10 => Sources(()) -> Vec<Source>,                           needs "arch.map:internal";
            11 => Changes(ChangesReq) -> Result<Delta, ArchError>,      needs "arch.map:internal";
            12 => Import(ImportReq) -> Result<Imported, ArchError>,     needs "arch.map:curate";
            13 => PutAlias(Alias) -> Result<(), ArchError>,             needs "arch.map:curate";
            14 => PutStory(StoryStep) -> Result<(), ArchError>,         needs "arch.map:curate";
        }
    }
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

/// ai.session's contract (repo:ai-session crates/ai-session-api at ce6eef1, major 1), copied verbatim like
/// feed_view_api: its doc comments are part of the schema hash (0x711b33ac59622d2b, pinned by a test), so not a
/// character may change. The Conversations tab relays only the ops of AI_OPS, and Events as server-sent events.
#[allow(dead_code)]
mod ai_session_api {
    ce_wire::api! {
        api AiSession = "ai.session", version 1;
        types {
            /// Health contract: these three fields, in this order, in every cell.
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            /// Where a shown value came from (ce-ref): kind msg, goal, commit, file, url, api or cell.
            pub struct Ref { pub label: String, pub kind: String, pub to: String }
            pub enum Role { User, Assistant, Tool }
            /// One piece of a message. `Attachment` is a ce-ref. Tool args and outputs are JSON text.
            pub enum Part { Text(String), Attachment(String), ToolCall { id: String, api: String, op: String, arg: String }, ToolResult { id: String, ok: bool, out: String }, Thinking(String) }
            pub struct Message { pub id: String, pub role: Role, pub parts: Vec<Part>, pub at_ms: u64, pub by: String }
            /// Tokens of one turn as the provider reported them; `input` excludes the cache columns.
            pub struct Usage { pub input: u64, pub output: u64, pub cache_read: u64, pub cache_write: u64, pub cost_usd: f64 }
            /// `Bare` (default): the provider's own model, nothing injected. `Code`: the provider's coding tools in `cwd`.
            pub enum Harness { Bare, Code { cwd: String, edit: bool, shell: bool } }
            pub enum Mode { Headless, Attached, Import }
            /// 0 = no cap for that column.
            pub struct Budget { pub max_tokens: u64, pub max_cost_usd: f64, pub max_wall_s: u32 }
            /// The tool's own session id on the node that holds its history file.
            pub struct NativeSession { pub provider: String, pub node: String, pub id: String }
            pub enum OffReason { NoAuth, NotInstalled, RealmRule { rule: String }, NeedsYes { who: String }, TermsUnverified, Unproven }
            pub enum Need { Tools, Vision, LongContext, Local }
            pub struct ModelRow { pub id: String, pub context: u32, pub needs: Vec<Need>, pub paid: bool }
            pub enum HarnessKind { Bare, Code }
            pub struct ImportSource { pub format: String, pub roots: Vec<String>, pub content_default: bool }
            /// Where the secret lives: a config dir that holds a login, or a key-stack NAME (never a value).
            pub enum AuthMethod { SubscriptionLogin { config_dir: String }, ApiKey { key: String }, OAuthDevice, LocalNone, ImportOnly }
            pub enum ModelList { Fixed(Vec<ModelRow>), FromProvider { allow: Vec<String>, deny: Vec<String> } }
            pub struct SpendCap { pub usd: f64, pub window: String, pub shared_pool: String }
            pub struct Limits { pub max_parallel: u32, pub max_turn_s: u32, pub max_tokens_turn: u64, pub rpm: u32, pub spend: Option<SpendCap> }
            pub struct Placement { pub node: String, pub bin: Option<String>, pub min_version: Option<String>, pub modes: Vec<Mode>, pub weight: u32 }
            pub enum Egress { ModelRoute, NoEgress }
            pub enum TermsFlag { OwnerOnly, TenantAutomation, Unverified }
            /// One row per (tenant, provider), section 2. `accounts` are names of logins or key accounts, in preference order.
            pub struct ProviderConfig { pub tenant: String, pub provider: String, pub enabled: bool, pub off: Option<OffReason>, pub auth: AuthMethod, pub accounts: Vec<String>, pub models: ModelList, pub default_model: Option<String>, pub limits: Limits, pub placement: Vec<Placement>, pub never_on: Vec<String>, pub harness: Vec<HarnessKind>, pub egress: Egress, pub import: Option<ImportSource>, pub terms: TermsFlag }
            /// `Named("claude.terminal")`, or any enabled provider that has every need.
            pub enum ProviderPick { Named(String), Any(Vec<Need>) }
            /// A capability the model may call: one (api, op) and the caller's chain attenuated to it (hex).
            pub struct ToolGrant { pub api: String, pub op: String, pub chain: String }
            /// `system` and `profile` are ce-refs. Both None and `harness: Bare` = the first tokens the model sees are the
            /// caller's first `Send`. Resolvable today: `inline:<text>` and `file:<absolute path on the session's node>`.
            pub struct OpenReq { pub provider: ProviderPick, pub model: Option<String>, pub account: Option<String>, pub system: Option<String>, pub profile: Option<String>, pub tools: Vec<ToolGrant>, pub harness: Harness, pub place: Option<String>, pub title: Option<String>, pub parent: Option<String>, pub budget: Budget }
            pub struct Opened { pub conversation: String, pub provider: String, pub account: String, pub model: String, pub node: String, pub refs: Vec<Ref> }
            /// `wait: true` answers the assistant message; `false` answers at once, read `Events`.
            pub struct SendReq { pub conversation: String, pub parts: Vec<Part>, pub expect: Option<String>, pub wait: bool }
            pub struct Sent { pub conversation: String, pub turn: u32, pub message: Option<Message>, pub usage: Option<Usage>, pub refs: Vec<Ref> }
            pub enum State { Idle, Busy, WaitingTool, WaitingApproval, Capped { resets_s: u32 }, Closed, Lost }
            /// Typed refusals; never a prose error.
            pub enum Refusal { NoAccount, Capped { resets_s: u32 }, NoPath(String), Shape(String), Budget(String), Denied { api: String, op: String }, Policy { reason: String }, NotFound(String), Invalid(String), Busy, Failed(String) }
            pub enum Event { Delta { text: String }, ToolCall { id: String, api: String, op: String, arg: String }, ToolResult { id: String, ok: bool, out: String }, Turn { message: Message, usage: Usage }, State(State), Refused(Refusal) }
            pub struct ReadReq { pub conversation: String, pub from: u32, pub limit: u32 }
            pub struct Turns { pub conversation: String, pub turns: Vec<Message>, pub next: Option<u32>, pub refs: Vec<Ref> }
            /// A new conversation from turns 0..=`at` of an old one, optionally on another provider (history replayed).
            pub struct ForkReq { pub conversation: String, pub at: u32, pub provider: Option<ProviderPick>, pub account: Option<String>, pub model: Option<String>, pub title: Option<String> }
            pub struct ConvReq { pub conversation: String }
            /// Every filter is optional; `limit` 0 = 200 (at most 1000). `q` matches title, cwd and id.
            pub struct ListReq { pub provider: Option<String>, pub account: Option<String>, pub node: Option<String>, pub kind: Option<String>, pub source: Option<String>, pub parent: Option<String>, pub q: Option<String>, pub limit: u32 }
            /// One conversation row of `ai.store` (metadata; turns are read with `Read`). `kind`: chat, worker, lane, session.
            pub struct Conversation { pub id: String, pub tenant: String, pub owner: String, pub title: String, pub provider: String, pub account: String, pub model: String, pub node: String, pub profile: Option<String>, pub parent: Option<String>, pub state: State, pub kind: String, pub source: String, pub cwd: String, pub turns: u32, pub opened_ms: u64, pub last_ms: u64, pub usage: Usage, pub native: Option<NativeSession> }
            pub struct Count { pub key: String, pub n: u32 }
            /// Newest first. `total` and `by_source` count every match, not just the page.
            pub struct Listed { pub conversations: Vec<Conversation>, pub total: u32, pub by_source: Vec<Count>, pub refs: Vec<Ref> }
            pub struct ConfigKey { pub provider: String }
            pub struct ConfigOff { pub provider: String, pub off: OffReason }
            /// Run each named adapter's read-only `Import` (empty = every enabled provider with an import source).
            pub struct ImportRunReq { pub providers: Vec<String>, pub since_ms: Option<u64> }
            pub struct Imported { pub seen: u32, pub new: u32, pub updated: u32, pub skipped: u32, pub by_source: Vec<Count>, pub refs: Vec<Ref> }
            /// An agent's profile as a row (`prof`, key `<tenant>/<name>`; AAC slice 5, migration doc section 1). `system`
            /// and `settings` are ce-refs (`inline:` or `file:<absolute path>` today): the agent's whole prompt and its Claude
            /// Code settings file (hooks). `Open{profile: <name>}` resolves the row; the profile decides provider, harness,
            /// system, settings and tools, the request may narrow account, model and budget.
            pub struct Profile { pub name: String, pub system: Option<String>, pub tools: Vec<ToolGrant>, pub harness: Harness, pub settings: Option<String>, pub provider: String, pub account: Option<String>, pub model: Option<String>, pub budget: Budget }
            pub struct ProfileKey { pub name: String }
            /// `Ask` (section 12.1): `Open` + `Send{wait: true}` + `Close` in one call, one row of kind `ask`; `keep: true`
            /// leaves it open to continue. No `provider` = claude.terminal. Its own right, so a grant can allow one-shot
            /// questions without long sessions.
            pub struct AskReq { pub text: String, pub attachments: Vec<String>, pub provider: Option<ProviderPick>, pub model: Option<String>, pub account: Option<String>, pub system: Option<String>, pub profile: Option<String>, pub expect: Option<String>, pub budget: Budget, pub keep: bool }
            pub struct Asked { pub conversation: String, pub message: Message, pub usage: Usage, pub refs: Vec<Ref> }
        }
        ops {
            1 => HealthOp(()) -> Health, needs "ai.session:health";
            2 => Open(OpenReq) -> Result<Opened, Refusal>, needs "ai.session:open", deadline_ms 120000;
            3 => Send(SendReq) -> Result<Sent, Refusal>, needs "ai.session:send", deadline_ms 1800000;
            4 => Read(ReadReq) -> Result<Turns, Refusal>, needs "ai.session:read";
            5 => Fork(ForkReq) -> Result<Opened, Refusal>, needs "ai.session:open", deadline_ms 120000;
            6 => Interrupt(ConvReq) -> Result<State, Refusal>, needs "ai.session:send";
            7 => Close(ConvReq) -> Result<State, Refusal>, needs "ai.session:close";
            8 => List(ListReq) -> Result<Listed, Refusal>, needs "ai.session:list";
            9 => Get(ConvReq) -> Result<Conversation, Refusal>, needs "ai.session:read";
            10 => ConfigList(()) -> Result<Vec<ProviderConfig>, Refusal>, needs "ai.session:config.read";
            11 => ConfigGet(ConfigKey) -> Result<ProviderConfig, Refusal>, needs "ai.session:config.read";
            12 => ConfigPut(ProviderConfig) -> Result<ProviderConfig, Refusal>, needs "ai.session:config.write";
            13 => ConfigEnable(ConfigKey) -> Result<ProviderConfig, Refusal>, needs "ai.session:config.write";
            14 => ConfigDisable(ConfigOff) -> Result<ProviderConfig, Refusal>, needs "ai.session:config.write";
            15 => ImportRun(ImportRunReq) -> Result<Imported, Refusal>, needs "ai.session:import", deadline_ms 3600000;
            // slice 3 from the tab (additive; ce docs/AI-AS-CELLS-2026-11-18.md section 12.1)
            17 => Continue(ConvReq) -> Result<Opened, Refusal>, needs "ai.session:send", deadline_ms 120000;
            18 => Ask(AskReq) -> Result<Asked, Refusal>, needs "ai.session:ask", deadline_ms 1800000;
            // profile rows (additive; 30.. so the import and schedule ops keep 16..29)
            30 => ProfilePut(Profile) -> Result<Profile, Refusal>, needs "ai.session:profile.write";
            31 => ProfileGet(ProfileKey) -> Result<Profile, Refusal>, needs "ai.session:profile.read";
            32 => ProfileList(()) -> Result<Vec<Profile>, Refusal>, needs "ai.session:profile.read";
        }
        streams {
            20 => Events(ConvReq) -> stream Event, needs "ai.session:read";
        }
    }
}

/// feed.view's contract (repo:feed-view crates/feed-view-api, major 2), copied verbatim like door_api: its doc comments
/// are part of the schema hash (0xf226abb16f72c818, pinned by a test), so not a character may change.
#[allow(dead_code)]
mod feed_view_api {
    ce_wire::api! {
        api FeedView = "feed.view", version 2;
        types {
            /// Health contract: these three fields, in this order, in every cell.
            pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
            /// The class a rule gave a row; `Duplicate` is applied after the topic is known (folded, not drawn).
            pub enum Class { Ask, Result, Update, Alarm, Monitor, Receipt, AgentNote, Own, Duplicate }
            pub enum Tab { ForYou, Alarms, Everything }
            /// Why a topic is closed.
            pub enum Why { Held, Dismissed, Quiet, Recovered }
            /// `before`: a card id; the page holds the cards after it in page order. `limit` 0 means 50, at most 200.
            pub struct Filter { pub tab: Tab, pub unread_only: bool, pub lane: Option<String>,
                                pub before: Option<String>, pub limit: u32 }
            /// `kind`: ref | url | join.
            pub struct Link { pub label: String, pub target: String, pub kind: String }
            pub struct Alarm { pub rings: u32, pub first_at_utc: String, pub last_at_utc: String, pub join: Option<Link> }
            /// One topic. `lines`: rows drawn in its thread; `folded`: rows folded into it; `unread`: drawn lines after
            /// the viewer's mark; `needs`: it holds an Ask; `long`: the newest line is longer than title plus summary.
            pub struct Card {
                pub id: String, pub class: Class, pub title: String, pub summary: String, pub lane: String,
                pub who: String, pub first_n: u64, pub last_n: u64, pub at_utc: String, pub lines: u32, pub folded: u32,
                pub unread: u32, pub needs: bool, pub closed: Option<Why>, pub links: Vec<Link>, pub alarm: Option<Alarm>,
                pub long: bool
            }
            /// Rows of the window by what keeps them out of the cards: agent notes (rules to-lane, harness), monitor
            /// lines, receipts, and repeats folded into a card.
            pub struct Hidden { pub agent_notes: u32, pub monitor: u32, pub receipts: u32, pub duplicates: u32 }
            /// `more`: the card id to pass as `before` for the next page. `unread_total`: unread cards under For you.
            /// `rules`: the rule version the cards were built with.
            pub struct FeedPage { pub cards: Vec<Card>, pub more: Option<String>, pub cursor: String, pub unread_total: u32,
                              pub hidden: Hidden, pub rules: String }
            /// One row of a thread with the class and rule that placed it; `delivered`: the receipt that followed it.
            pub struct Line { pub n: u64, pub at_utc: String, pub who: String, pub lane: String, pub text: String,
                              pub class: Class, pub rule: String, pub delivered: Option<String> }
            pub struct Thread { pub card: Card, pub lines: Vec<Line> }
            pub struct MarkReq { pub ids: Vec<String>, pub read: bool }
            pub struct Marked { pub unread_total: u32 }
            pub struct CloseReq { pub id: String, pub why: Why }
            /// Answers at once when the filtered cards differ from `after`, else holds up to `wait_ms` (at most 25,000).
            pub struct ChangesReq { pub after: String, pub filter: Filter, pub wait_ms: u32 }
            pub struct Delta { pub put: Vec<Card>, pub gone: Vec<String>, pub cursor: String, pub unread_total: u32 }
            pub struct UiReq { pub view: String }
            pub struct Bundle { pub sha256: String, pub speaks: Vec<u16>, pub kit: u16, pub bytes: Vec<u8> }
            /// Row numbers, both ends included.
            pub struct Window { pub from_n: u64, pub to_n: u64 }
            /// Every row talk.messages List shows in the window is in exactly one card, folded into one, or hidden with a
            /// rule id: `rows == in_cards + folded + hidden`, `unaccounted` (row numbers) empty.
            pub struct Report { pub rows: u64, pub in_cards: u64, pub folded: u64, pub hidden: u64, pub unaccounted: Vec<u64> }
            pub enum FeedError { Source(String), Store(String), NoSuchTopic(String), NoSuchView(String) }
        }
        ops {
            1 => HealthOp(()) -> Health,                       needs "feed.view:health";
            2 => Page(Filter) -> Result<FeedPage, FeedError>,  needs "feed.view:read", deadline_ms 20000;
            3 => Topic(String) -> Result<Thread, FeedError>,   needs "feed.view:read", deadline_ms 20000;
            4 => Changes(ChangesReq) -> Result<Delta, FeedError>, needs "feed.view:read", deadline_ms 45000;
            5 => Mark(MarkReq) -> Result<Marked, FeedError>,   needs "feed.view:mark", deadline_ms 20000;
            6 => Close(CloseReq) -> Result<Card, FeedError>,   needs "feed.view:close", deadline_ms 20000;
            7 => Ui(UiReq) -> Result<Bundle, FeedError>,       needs "feed.view:read";
            8 => Compare(Window) -> Result<Report, FeedError>, needs "feed.view:verify", deadline_ms 60000;
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

/// cell.search's contract (repo:cell-search crates/cell-search-api, major 1), copied verbatim like door_api.
#[allow(dead_code)]
mod cell_search_api {
    ce_wire::api! {
    api CellSearch = "cell.search", version 1;
    types {
        /// Health contract: these three fields, in this order, in every cell.
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        /// `q` = plain words ("database", "print farm"); `k` = most hits wanted (0 = 10, at most 50).
        pub struct SearchReq { pub q: String, pub k: u32 }
        /// One op of a cell: wire id, name, request and answer types as the api! block writes them, the right it needs.
        pub struct Op { pub id: u32, pub name: String, pub req: String, pub resp: String, pub right: String }
        /// A known advisory (osv.dev: RUSTSEC / GHSA / CVE) against one crate at the version the cell's Cargo.lock pins.
        pub struct Advisory { pub id: String, pub krate: String, pub version: String }
        /// Why a cell can be trusted, from records, never from its own words:
        /// `signer` = "ours" (published under the ce-net owners) or "community"; `publisher` = its GitHub owner;
        /// `source` = git url; `commit` = the 40-hex commit indexed ("" = no checkout seen);
        /// `review` = the deploy.review verdict for exactly that commit (pass | refuse | advisory | unreviewed),
        /// `review_id` = "<api>@<commit 12 hex>", `reviewed_at` = unix s (0 = never);
        /// `deps` = crates in its Cargo.lock, `registry_deps` = of those from crates.io (each checked against osv.dev),
        /// `advisories` = the open ones, `audited_at` = unix s of that check (0 = not yet).
        pub struct Trust {
            pub signer: String, pub publisher: String, pub source: String, pub commit: String,
            pub review: String, pub review_id: String, pub reviewed_at: u64,
            pub deps: u32, pub registry_deps: u32, pub advisories: Vec<Advisory>, pub audited_at: u64,
        }
        /// One cell. `score` = 0..1 blend of `text_score` (BM25 over name, title, about and ops) and `meaning_score`
        /// (cosine of search.embed vectors); `running_on` = devices running it now.
        pub struct Hit {
            pub api: String, pub repo: String, pub title: String, pub about: String, pub owner: String, pub version: u32,
            pub ops: Vec<Op>, pub depends_on: Vec<String>, pub running_on: u32, pub private: bool,
            pub score: f32, pub text_score: f32, pub meaning_score: f32, pub trust: Trust,
        }
        /// `took_us` = time inside the cell; `indexed` = cells the caller may see; `index_at` = unix s of the last feed.
        pub struct SearchResp { pub hits: Vec<Hit>, pub took_us: u64, pub indexed: u32, pub model: String, pub index_at: u64 }
        /// `changed` = cells re-read in the last feed (unchanged commit + row = reused); `feed_ms` = its wall time.
        pub struct Stats {
            pub cells: u32, pub private: u32, pub with_ops: u32, pub with_lock: u32, pub audited: u32, pub with_advisories: u32,
            pub reviewed: u32, pub crates_checked: u32, pub changed: u32, pub feed_ms: u64, pub index_at: u64, pub errors: Vec<String>,
        }
        pub enum SearchError { Invalid(String), Failed(String) }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "cell.search:health";
        2 => Search(SearchReq) -> Result<SearchResp, SearchError>, needs "cell.search:search";
        3 => Get(String) -> Option<Hit>, needs "cell.search:search";
        4 => StatsOp(()) -> Stats, needs "cell.search:stats";
        5 => Reindex(()) -> Result<Stats, SearchError>, needs "cell.search:reindex", deadline_ms 300000;
    }
}
}

/// crm.people's contract (repo:crm-people crates/crm-people-api, major 1), copied verbatim like door_api.
#[allow(dead_code)]
mod crm_people_api {
    ce_wire::api! {
    // accepts: the schema before SetProjects/Projects (two ce-wire hashings of it), so every older caller still connects
    api CrmPeople = "crm.people", version 1, accepts [0x8a8fedb2fa7ed346, 0xdfc777d5b87fc2d7];
    types {
        /// Health contract: these three fields, in this order, in every cell.
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        /// Who did it: we (Leif and every lane writing as him), they, or both.
        pub enum Side { Us, Them, Both }
        /// What an event was. Reschedule, NoShow and Cancel carry `by` so a reschedule by us counts against us.
        pub enum Kind { Message, Meeting, Call, Reschedule, NoShow, Cancel, Quote, Sample, Order, Payment, Complaint, Apology, Question, Other }
        /// One person (or one company contact). `id` is a slug ("richard-wizard3d"). `aliases` are crm.dump refs
        /// (exact, or a prefix ending in `*`) whose events belong to this person. `cursor_ts` is the newest dump event
        /// already extracted; `summary` and `next_step` are rewritten by the extractor.
        pub struct Person {
            pub id: String,
            pub name: String,
            pub org: String,
            pub role: String,
            pub aliases: Vec<String>,
            pub summary: String,
            pub next_step: String,
            pub owner_lane: String,
            pub cursor_ts: i64,
            pub updated_s: i64,
        }
        /// One meaningful event with a person, typed. `event_id` is the crm.dump event it came from.
        pub struct Interaction { pub person: String, pub ts: i64, pub kind: Kind, pub by: Side, pub summary: String, pub event_id: String }
        pub enum PromiseStatus { Open, Kept, Broken, Dropped }
        /// Something someone said they would do. `owner` Us = we owe it.
        pub struct Promise { pub id: String, pub person: String, pub owner: Side, pub text: String, pub due_s: Option<i64>, pub status: PromiseStatus, pub made_ts: i64, pub evidence: String }
        /// How they feel toward us at `ts`: -2 angry, -1 annoyed, 0 neutral, 1 warm, 2 enthusiastic, with their words.
        pub struct Mood { pub person: String, pub ts: i64, pub score: i32, pub evidence: String, pub event_id: String }
        pub struct Profile { pub person: Person, pub mood: Option<Mood>, pub open_promises: Vec<Promise>, pub interactions: u32, pub last_ts: i64 }
        /// One person who needs care now, with every reason and a score (higher = sooner).
        pub struct AtRisk { pub person: String, pub name: String, pub org: String, pub score: u32, pub reasons: Vec<String>, pub mood: Option<Mood>, pub next_step: String }
        /// `person`: one id (else every person). `auto`: also make a Person for every dump ref with an inbound message
        /// that no alias covers. `reset`: drop the person's derived rows and extract from the start. `max_people`:
        /// at most this many people per run (0 = 25).
        pub struct ExtractReq { pub person: Option<String>, pub auto: bool, pub reset: bool, pub max_people: u32 }
        pub struct ExtractResp { pub people: u32, pub events: u32, pub interactions: u32, pub promises: u32, pub moods: u32, pub errors: Vec<String> }
        pub enum PeopleError { Invalid(String), NotFound(String), Store(String), Extract(String) }
        /// The projects a person is part of (slugs: "led-frame", "bar-cocktail-pour"), so a message about one project is
        /// never sent to someone outside it (lead m:bd0cf89f; the corner piece that went to the wrong Hans).
        pub struct ProjectsReq { pub person: String, pub projects: Vec<String> }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "crm.people:health";
        // Make or change a person (aliases, name, org); the extractor's cursor and summary are kept.
        2 => Upsert(Person) -> Result<Person, PeopleError>, needs "crm.people:write";
        // By id, alias or a unique name match.
        3 => Get(String) -> Result<Profile, PeopleError>, needs "crm.people:read";
        // Every interaction with the person, oldest first.
        4 => Timeline(String) -> Result<Vec<Interaction>, PeopleError>, needs "crm.people:read";
        // Open promises, of one person or everyone, soonest due first.
        5 => OpenPromises(Option<String>) -> Result<Vec<Promise>, PeopleError>, needs "crm.people:read";
        // People whose mood is low or falling, or whom we rescheduled, missed or owe overdue promises; highest first.
        6 => AtRiskList(()) -> Result<Vec<AtRisk>, PeopleError>, needs "crm.people:read";
        // Case-insensitive words over name, org, role, aliases and summary.
        7 => Search(String) -> Result<Vec<Person>, PeopleError>, needs "crm.people:read";
        // Read new crm.dump events and update the typed rows (DeepSeek Flash).
        8 => Extract(ExtractReq) -> Result<ExtractResp, PeopleError>, needs "crm.people:extract";
        // the person's projects: set (replaces the list) and read; an unknown person is NotFound
        9 => SetProjects(ProjectsReq) -> Result<Vec<String>, PeopleError>, needs "crm.people:write";
        10 => Projects(String) -> Result<Vec<String>, PeopleError>, needs "crm.people:read";
    }
}
}

/// crm.promises's contract (repo:crm-promises crates/crm-promises-api, major 1), copied verbatim like door_api.
#[allow(dead_code)]
mod crm_promises_api {
    ce_wire::api! {
    api CrmPromises = "crm.promises", version 1;
    types {
        /// Health contract: these three fields, in this order, in every cell.
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        /// Open until done (Kept), missed (Broken) or no longer wanted (Dropped).
        pub enum Status { Open, Kept, Broken, Dropped }
        /// One promise. `id` is the object id `deal.promise:<tenant>:<local>` (the ops.graph id form; tenant
        /// `[a-z0-9-]{1,32}`, local `[A-Za-z0-9._-]{1,64}`); Put upserts by it. `to` is who it was made to (an email
        /// or a name), `to_name` the person's name, `company` theirs. Times are unix ms (UTC); `due` 0 = no date.
        /// `source` is where it was made (`gmail:sent:4348`, a WeChat thread). `note` says why the status last
        /// changed. `updated_at` is set by the cell.
        pub struct Promise {
            pub id: String,
            pub text: String,
            pub to: String,
            pub to_name: String,
            pub company: String,
            pub made_at: u64,
            pub due: u64,
            pub status: Status,
            pub source: String,
            pub note: String,
            pub updated_at: u64,
        }
        /// Every field optional (empty = any): `person` matches `to` or `to_name`, `company` the company, both
        /// case-insensitive substrings; `tenant` exact. Sorted by `made_at`, oldest first.
        pub struct ListReq { pub person: String, pub company: String, pub status: Option<Status>, pub tenant: String }
        pub struct SetStatusReq { pub id: String, pub status: Status, pub note: String }
        pub enum PromiseError { Invalid(String), NotFound(String), Store(String) }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "crm.promises:health";
        2 => Put(Promise) -> Result<Promise, PromiseError>, needs "crm.promises:write";
        3 => Get(String) -> Result<Promise, PromiseError>, needs "crm.promises:read";
        4 => List(ListReq) -> Result<Vec<Promise>, PromiseError>, needs "crm.promises:read";
        5 => SetStatus(SetStatusReq) -> Result<Promise, PromiseError>, needs "crm.promises:write";
    }
}
}

/// state.store's contract (repo:state-store-cell crates/state-store-api at 0ca497e, v2), copied verbatim like the others:
/// the Home links slot reads its rows (`home/links`) with this cell's own read grant for that one collection.
#[allow(dead_code)]
mod state_store_api {
    ce_wire::api! {
        api StateStore = "state.store", version 2, accepts [0x17de2d4fee4776ea];
        types {
            // Coll: one collection of one app. Every request also carries `cap`: the hex ce-cap chain that grants
            // `state.<app>.<coll>.read|write|own` (what `ce-iam wallet show state.store:<app>` prints). The Hello proves
            // which node calls; `cap` proves what that node may touch. An empty cap is refused.
            pub struct Coll { pub app: String, pub coll: String }
            // Doc: the stored record. value is JSON text; seq is the global write sequence (List/Watch cursor);
            // version counts writes of this key from 1; deleted marks a tombstone (Watch/include_deleted only).
            pub struct Doc { pub key: String, pub value: String, pub seq: u64, pub version: u64, pub deleted: bool }
            pub struct GetReq { pub at: Coll, pub key: String, pub cap: String }
            // PutReq: if_version = Some(v) is compare-and-set (refused unless the stored version is v; Some(0) means
            // "must not exist"); None replaces unconditionally. insert_only = create if absent, else return the stored doc.
            pub struct PutReq { pub at: Coll, pub key: String, pub value: String, pub if_version: Option<u64>, pub insert_only: bool, pub cap: String }
            // PatchReq: merge the top-level fields of `fields` (a JSON object) into the stored object.
            pub struct PatchReq { pub at: Coll, pub key: String, pub fields: String, pub cap: String }
            pub struct DeleteReq { pub at: Coll, pub key: String, pub cap: String }
            // ListReq: docs with seq > since whose key starts with prefix, in seq order, at most limit (<= MAX_PAGE).
            pub struct ListReq { pub at: Coll, pub prefix: String, pub since: u64, pub limit: u32, pub include_deleted: bool, pub cap: String }
            // Page: next = Some(cursor) when more remain; pass it as since.
            pub struct Page { pub docs: Vec<Doc>, pub next: Option<u64> }
            // WatchReq: every change (tombstones included) with seq > since, then each new one.
            pub struct WatchReq { pub at: Coll, pub since: u64, pub cap: String }
            pub struct SchemaReq { pub app: String, pub cap: String }
            // SchemaPutReq: schema is the JSON of {app, collections:{...}}; needs `state.<app>.admin`.
            pub struct SchemaPutReq { pub app: String, pub schema: String, pub cap: String }
            // PeerPull: one replication peer. last_ok_pull_at = unix ms of the last pull that applied cleanly, 0 = none
            // since this server started (held in memory); age_ms = now - last_ok_pull_at on this server's clock, u64::MAX
            // when none. A reader that needs a fresh replica (cap.admit: under 10 s) checks age_ms.
            pub struct PeerPull { pub peer: String, pub seq: u64, pub last_ok_pull_at: u64, pub age_ms: u64 }
            pub struct ReplStatus { pub node: String, pub peers: Vec<PeerPull> }
            pub struct StoreHealth { pub backend: String, pub records: u64, pub head_seq: u64, pub audit_rows: u64, pub schema_hash: u64 }
        }
        ops {
            1 => Get(GetReq) -> Option<Doc>, needs "state.store:read";
            2 => Put(PutReq) -> Doc, needs "state.store:write";
            3 => Delete(DeleteReq) -> (), needs "state.store:write";
            4 => List(ListReq) -> Page, needs "state.store:read";
            5 => Health(()) -> StoreHealth, needs "state.store:read";
            7 => Patch(PatchReq) -> Doc, needs "state.store:write";
            8 => Schema(SchemaReq) -> Option<String>, needs "state.store:read";
            9 => SchemaPut(SchemaPutReq) -> (), needs "state.store:write";
            10 => ReplHealth(()) -> ReplStatus, needs "state.store:read";
        }
        streams {
            6 => Watch(WatchReq) -> stream Doc, needs "state.store:read";
        }
    }
}

/// crm.calls's contract (repo:crm-calls crates/crm-calls-api, major 2), copied verbatim like door_api.
#[allow(dead_code)]
mod crm_calls_api {
    ce_wire::api! {
    api CrmCalls = "crm.calls", version 2;
    types {
        /// Health contract: these three fields, in this order, in every cell.
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        /// How the call happens. `Other` names anything else ("zoom", "whatsapp").
        pub enum Channel { Jitsi, Wechat, Phone, InPerson, Other(String) }
        /// `Booked` only when the other person said yes (their words are in `confirmed_by`). `Proposed`: we asked, no yes
        /// yet ("awaiting approval", Leif 2026-11-05: "if the other person has not agreed, it's not booked"). `Event`: a
        /// public event or meetup with nobody to confirm.
        pub enum Status { Booked, Held, Missed, Cancelled, Moved, Proposed, Event }
        /// One call with one person. `id` is stable (a ce-schedule task id or a slug) and Put upserts by it. Times are
        /// unix seconds (UTC); `tz` is the person's zone for display ("Asia/Shanghai"). `owner` is the lane that holds
        /// the call. `source` says where the record came from (ce-schedule, a mail, a WeChat thread). `updated_s` is
        /// set by the cell. `confirmed_by` is where the other person said yes (a Message-ID, a WeChat/X thread, a
        /// calendar invite id); `None` means no one has.
        pub struct CallRecord {
            pub id: String,
            pub person: String,
            pub company: String,
            pub channel: Channel,
            pub at_s: i64,
            pub tz: String,
            pub status: Status,
            pub owner: String,
            pub link: String,
            pub brief: String,
            pub outcome: String,
            pub next_step: String,
            pub next_step_at_s: Option<i64>,
            pub source: String,
            pub updated_s: i64,
            pub confirmed_by: Option<String>,
        }
        /// Every field optional: `person` matches person or company, case-insensitive substring; `since_s` keeps
        /// calls at or after that time. Results are sorted by `at_s`, oldest first.
        pub struct ListReq { pub person: Option<String>, pub status: Option<Status>, pub since_s: Option<i64> }
        /// Counts by status, plus how many calls still carry a next step.
        pub struct CallStats { pub total: u32, pub booked: u32, pub held: u32, pub missed: u32, pub cancelled: u32, pub moved: u32, pub open_next_steps: u32, pub proposed: Option<u32>, pub event: Option<u32> }
        pub enum CallsError { Invalid(String), NotFound(String), Store(String) }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "crm.calls:health";
        2 => Put(CallRecord) -> Result<CallRecord, CallsError>, needs "crm.calls:write";
        3 => Get(String) -> Result<CallRecord, CallsError>, needs "crm.calls:read";
        4 => List(ListReq) -> Result<Vec<CallRecord>, CallsError>, needs "crm.calls:read";
        5 => Stats(()) -> Result<CallStats, CallsError>, needs "crm.calls:read";
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

/// unix seconds or ms as "YYYY-MM-DD HH:MM UTC" ("" for 0).
fn when(t: i64) -> String {
    if t <= 0 {
        return String::new();
    }
    let s = if t > 100_000_000_000 { t / 1000 } else { t };
    let (days, rem) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    // civil from days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, rem % 3600 / 60)
}

/// One CRM or call record for the Search tab's record screen, read as the viewer from the cell that owns it
/// (`person:` crm.people Get, `promise:` crm.promises Get, `call:` crm.calls Get). `meet:` has no owning cell yet:
/// search.hybrid is asked as the viewer for that recording's passage (sources `calls`, its private label), with the
/// viewer's query `q` first. -> (status, body) with `title`, `fields` ([label, value]) and `text`.
async fn view_rec(person: &str, abilities: &[String], r: &str, q: &str) -> (&'static str, serde_json::Value) {
    use serde_json::json;
    let Some((kind, id)) = parse_rec(r) else {
        return ("400 Bad Request", json!({"error": "rec: person:<id>, promise:<id>, call:<id> or meet:<recording>"}));
    };
    let closed = |cell: &str| ("403 Forbidden", json!({"error": format!("{cell} is not open to you")}));
    let down = |cell: &str, e: String| ("503 Service Unavailable", json!({"error": format!("{cell}: {}", e.chars().take(200).collect::<String>())}));
    let missing = |cell: &str, e: String| ("404 Not Found", json!({"error": format!("{cell} has no {kind} {id}: {}", e.chars().take(200).collect::<String>())}));
    let opt = |s: &str| if s.is_empty() { None } else { Some(s.to_string()) };
    let f = |rows: Vec<(&str, Option<String>)>| rows.into_iter().filter_map(|(k, v)| v.map(|v| json!([k, v]))).collect::<Vec<_>>();
    match kind {
        "person" => {
            use crm_people_api as cp;
            if for_cell(abilities, "crm.people", "crm.people:read").is_none() {
                return closed("crm.people");
            }
            let c = match client::<cp::CrmPeople>(person, abilities).await {
                Ok(c) => c,
                Err(e) => return down("crm.people", e),
            };
            let p = match c.call::<cp::Get>(&id).await {
                Ok(Ok(p)) => p,
                Ok(Err(e)) => return missing("crm.people", format!("{e:?}")),
                Err(e) => return down("crm.people", e.to_string()),
            };
            let pp = &p.person;
            let mood = p.mood.as_ref().map(|m| format!("{} ({}): {}", m.score, when(m.ts), m.evidence));
            let promises: Vec<String> = p
                .open_promises
                .iter()
                .map(|x| {
                    let who = if matches!(x.owner, cp::Side::Us) { "We owe" } else { "They owe" };
                    format!("{who}: {}{}", x.text, x.due_s.map(|d| format!(" (due {})", when(d))).unwrap_or_default())
                })
                .collect();
            let title = if pp.org.is_empty() { pp.name.clone() } else { format!("{}, {}", pp.name, pp.org) };
            ("200 OK", json!({"kind": kind, "id": id.clone(), "cell": "crm.people", "title": title,
                "fields": f(vec![("role", opt(&pp.role)), ("next step", opt(&pp.next_step)), ("mood", mood), ("interactions", Some(p.interactions.to_string())),
                    ("last contact", opt(&when(p.last_ts))), ("lane", opt(&pp.owner_lane)), ("aliases", opt(&pp.aliases.join(", ")))]),
                "text": pp.summary, "list_title": "Open promises", "list": promises, "search": pp.name}))
        }
        "promise" => {
            use crm_promises_api as pr;
            if for_cell(abilities, "crm.promises", "crm.promises:read").is_none() {
                return closed("crm.promises");
            }
            let c = match client::<pr::CrmPromises>(person, abilities).await {
                Ok(c) => c,
                Err(e) => return down("crm.promises", e),
            };
            let p = match c.call::<pr::Get>(&id).await {
                Ok(Ok(p)) => p,
                Ok(Err(e)) => return missing("crm.promises", format!("{e:?}")),
                Err(e) => return down("crm.promises", e.to_string()),
            };
            let who = if p.to_name.is_empty() { p.to.clone() } else { p.to_name.clone() };
            let title = format!("Promise to {who}{}", if p.company.is_empty() { String::new() } else { format!(" ({})", p.company) });
            ("200 OK", json!({"kind": kind, "id": id.clone(), "cell": "crm.promises", "title": title,
                "fields": f(vec![("status", Some(format!("{:?}", p.status).to_lowercase())), ("due", opt(&when(p.due as i64))), ("made", opt(&when(p.made_at as i64))),
                    ("to", opt(&p.to)), ("made in", opt(&p.source)), ("why this status", opt(&p.note)), ("changed", opt(&when(p.updated_at as i64)))]),
                "text": p.text, "search": who}))
        }
        "call" => {
            use crm_calls_api as cc;
            if for_cell(abilities, "crm.calls", "crm.calls:read").is_none() {
                return closed("crm.calls");
            }
            let c = match client::<cc::CrmCalls>(person, abilities).await {
                Ok(c) => c,
                Err(e) => return down("crm.calls", e),
            };
            let x = match c.call::<cc::Get>(&id).await {
                Ok(Ok(x)) => x,
                Ok(Err(e)) => return missing("crm.calls", format!("{e:?}")),
                Err(e) => return down("crm.calls", e.to_string()),
            };
            let channel = match &x.channel {
                cc::Channel::Jitsi => "Jitsi".to_string(),
                cc::Channel::Wechat => "WeChat".to_string(),
                cc::Channel::Phone => "phone".to_string(),
                cc::Channel::InPerson => "in person".to_string(),
                cc::Channel::Other(o) => o.clone(),
            };
            let text = [("Brief", &x.brief), ("Outcome", &x.outcome)]
                .iter()
                .filter(|(_, v)| !v.is_empty())
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("\n\n");
            let title = format!("Call with {}{}, {}", x.person, if x.company.is_empty() { String::new() } else { format!(" ({})", x.company) }, when(x.at_s));
            ("200 OK", json!({"kind": kind, "id": id.clone(), "cell": "crm.calls", "title": title,
                "fields": f(vec![("status", Some(format!("{:?}", x.status).to_lowercase())), ("channel", Some(channel)), ("their zone", opt(&x.tz)),
                    ("next step", opt(&x.next_step)), ("next step by", x.next_step_at_s.map(when)), ("confirmed by", x.confirmed_by.clone()),
                    ("lane", opt(&x.owner)), ("from", opt(&x.source)), ("link", opt(&x.link))]),
                "text": text, "search": x.person}))
        }
        _ => {
            // meet: the recording's passage from search.hybrid, as the viewer (their exact read of the calls label)
            if for_cell(abilities, "search.hybrid", "search.hybrid:read:calls").is_none() || for_cell(abilities, "search.hybrid", "search.hybrid:find").is_none() {
                return closed("call transcripts");
            }
            let want = format!("meet/{id}");
            let room = id.split('/').next().unwrap_or("").splitn(3, '-').nth(2).unwrap_or("").replace('-', " ");
            let asks: Vec<String> =
                [q.to_string(), format!("call {room}"), id.replace(['/', '-', '.'], " ")].into_iter().filter(|s| !s.trim().is_empty()).collect();
            let c = match client::<hybrid_api::SearchHybrid>(person, abilities).await {
                Ok(c) => c,
                Err(e) => return down("search.hybrid", e),
            };
            let mut found: Option<hybrid_api::Hit> = None;
            for a in asks {
                match c.call::<hybrid_api::Find>(&hybrid_api::FindReq { q: a, k: 50, sources: vec!["calls".into()], rerank: false }).await {
                    Ok(Ok(r)) => {
                        if let Some(h) = r.hits.into_iter().find(|h| h.path == want) {
                            found = Some(h);
                            break;
                        }
                    }
                    Ok(Err(e)) => return down("search.hybrid", format!("{e:?}")),
                    Err(e) => return down("search.hybrid", e.to_string()),
                }
            }
            let Some(h) = found else { return missing("search.hybrid", "no passage of it is open to you".into()) };
            ("200 OK", json!({"kind": kind, "id": id.clone(), "cell": "search.hybrid", "title": h.title,
                "fields": f(vec![("recording", Some(id.clone())), ("indexed on", Some(h.machine.clone())), ("label", Some(h.acl.clone()))]),
                "text": h.snippet,
                "note": "The passage of this call that best matches your search. The whole transcript stays in meet-notes on the machine that recorded it; no cell serves it whole yet.",
                "search": room}))
        }
    }
}

fn node_name() -> String {
    std::fs::read_to_string("/etc/hostname").unwrap_or_default().trim().to_string()
}

/// One ask of the Search tab, once it is back: its answer, why it failed, or that it was not asked.
enum Ask<T> {
    Pending,
    Skip,
    Late(u64),
    Err(String),
    Ok(T),
}

impl<T> Ask<T> {
    fn of(r: Result<Result<T, String>, tokio::time::error::Elapsed>, s: u64) -> Ask<T> {
        match r {
            Ok(Ok(v)) => Ask::Ok(v),
            Ok(Err(e)) => Ask::Err(e),
            Err(_) => Ask::Late(s),
        }
    }
    fn of_opt(r: Result<Option<Result<T, String>>, tokio::time::error::Elapsed>, s: u64) -> Ask<T> {
        match r {
            Ok(None) => Ask::Skip,
            Ok(Some(x)) => Ask::of(Ok(x), s),
            Err(_) => Ask::Late(s),
        }
    }
    fn pending(&self) -> bool {
        matches!(self, Ask::Pending)
    }
}

/// The asks of one search as they come back.
struct Asks {
    h: Ask<hybrid_api::FindResp>,
    p: Ask<hybrid_api::FindResp>,
    m: Ask<mesh_api::MeshResp>,
    cs: Ask<cell_search_api::SearchResp>,
    cr: Ask<hybrid_api::FindResp>,
    cv: Ask<ai_session_api::Listed>,
}

/// The note a failed or waiting ask leaves (`pending`: still asking; the page says so and waits for the next part).
fn ask_note<T>(a: &Ask<T>, cell: &str, ask: Option<&str>) -> Option<serde_json::Value> {
    use serde_json::json;
    let mut n = match a {
        Ask::Ok(_) | Ask::Skip => return None,
        Ask::Pending => json!({"cell": cell, "ok": false, "pending": true, "err": "asking"}),
        Ask::Late(s) => json!({"cell": cell, "ok": false, "err": format!("no answer in {s} s")}),
        Ask::Err(e) => json!({"cell": cell, "ok": false, "err": e.chars().take(200).collect::<String>()}),
    };
    if let Some(x) = ask {
        n["ask"] = json!(x);
    }
    Some(n)
}

/// One search over both: search.hybrid as the viewer (their exact private reads ride in the minted grant) and
/// search.mesh's merged answer from every node, at once. A side that fails is named in `notes`, never the whole answer.
/// search.mesh is asked only for a viewer holding `search.mesh:find`: its nodes index internal repos and docs.
/// Each group keeps its own `k` (Private, Network, Public): for a viewer with private reads search.hybrid is asked
/// twice, once as them and once with `search.hybrid:find` alone (public only), so their private hits never fill Public.
/// Conversations: ai.session `List` with the words, as the viewer (its own rights decide which rows exist).
///
/// `parts`: each time an ask comes back the answer so far is sent there (S5 of repo:ce
/// docs/SPEED-AND-SEARCH-2026-11-21.md: the page showed nothing until the slowest of five asks was back); the last
/// answer is returned whole as before.
async fn search(person: &str, abilities: &[String], q: &str, k: u32, parts: Option<&tokio::sync::mpsc::UnboundedSender<serde_json::Value>>) -> serde_json::Value {
    let t0 = Instant::now();
    let find = |ab: Vec<String>, k: u32, sources: Vec<String>| {
        let q = q.to_string();
        async move {
            for_cell(&ab, "search.hybrid", "search.hybrid:find").ok_or("not open to you".to_string())?;
            let c = client::<hybrid_api::SearchHybrid>(person, &ab).await?;
            let r = c.call::<hybrid_api::Find>(&hybrid_api::FindReq { q, k, sources, rerank: true }).await.map_err(|e| e.to_string())?;
            r.map_err(|e| format!("{e:?}"))
        }
    };
    let pub_ab = public_only(abilities);
    // as the viewer: enough to fill Private after the public hits in it are taken out
    let hybrid = find(abilities.to_vec(), if pub_ab.is_some() { (k * 2).min(50) } else { k }, vec![]);
    let public_ask = async {
        match pub_ab.clone() {
            Some(ab) => Some(find(ab, k, vec![]).await),
            None => None,
        }
    };
    // CRM people, calls, promises and call transcripts: their own ask (sources calls and people, the viewer's exact
    // reads of those labels), so many file hits never push them out of Private
    let crm_open = ["search.hybrid:read:calls", "search.hybrid:read:people"].iter().any(|a| abilities.iter().any(|b| b == a));
    let crm_ask = async {
        if crm_open {
            Some(find(abilities.to_vec(), k, vec!["calls".into(), "people".into()]).await)
        } else {
            None
        }
    };
    let mesh = async {
        for_cell(abilities, "search.mesh", "search.mesh:find").ok_or("not open to you".to_string())?;
        let c = client::<mesh_api::SearchMesh>(person, abilities).await?;
        let r = c.call::<mesh_api::Mesh>(&mesh_api::MeshReq { q: q.into(), k, deadline_ms: 2500, sources: vec![] }).await.map_err(|e| e.to_string())?;
        r.map_err(|e| format!("{e:?}"))
    };
    // cell.search: every registered cell ranked by meaning and text, each with its trust record (Network group)
    let cellq = async {
        for_cell(abilities, "cell.search", "cell.search:search").ok_or("not open to you".to_string())?;
        let c = client::<cell_search_api::CellSearch>(person, abilities).await?;
        let r = c.call::<cell_search_api::Search>(&cell_search_api::SearchReq { q: q.into(), k }).await.map_err(|e| e.to_string())?;
        r.map_err(|e| format!("{e:?}"))
    };
    // conversations whose title, folder or id hold the words, only those ai.session lists for this viewer
    let conv = async {
        let Some(ab) = for_cell(abilities, "ai.session", "ai.session:list") else { return None };
        Some(async {
            let c = client::<ai_session_api::AiSession>(person, &ab).await?;
            let req = ai_session_api::ListReq { provider: None, account: None, node: None, kind: None, source: None, parent: None, q: Some(q.to_string()), limit: 5 };
            c.call::<ai_session_api::List>(&req).await.map_err(|e| e.to_string())?.map_err(|e| format!("{e:?}"))
        }.await)
    };
    let (hf, pf, mf, csf, crf, cvf) = (
        tokio::time::timeout(Duration::from_secs(22), hybrid),
        tokio::time::timeout(Duration::from_secs(22), public_ask),
        tokio::time::timeout(Duration::from_secs(12), mesh),
        tokio::time::timeout(Duration::from_secs(12), cellq),
        tokio::time::timeout(Duration::from_secs(22), crm_ask),
        tokio::time::timeout(Duration::from_secs(8), conv),
    );
    tokio::pin!(hf, pf, mf, csf, crf, cvf);
    let mut a = Asks { h: Ask::Pending, p: Ask::Pending, m: Ask::Pending, cs: Ask::Pending, cr: Ask::Pending, cv: Ask::Pending };
    loop {
        let part = tokio::select! {
            x = &mut hf, if a.h.pending() => { a.h = Ask::of(x, 22); "private" }
            x = &mut pf, if a.p.pending() => { a.p = Ask::of_opt(x, 22); "public" }
            x = &mut mf, if a.m.pending() => { a.m = Ask::of(x, 12); "network" }
            x = &mut csf, if a.cs.pending() => { a.cs = Ask::of(x, 12); "cells" }
            x = &mut crf, if a.cr.pending() => { a.cr = Ask::of_opt(x, 22); "crm" }
            x = &mut cvf, if a.cv.pending() => { a.cv = Ask::of_opt(x, 8); "conversations" }
        };
        let done = !(a.h.pending() || a.p.pending() || a.m.pending() || a.cs.pending() || a.cr.pending() || a.cv.pending());
        if done {
            return assemble(q, k, &a, t0).await;
        }
        if let Some(tx) = parts {
            // a part only when it changes what the page shows (a skipped ask adds nothing)
            let skipped = match part {
                "public" => matches!(a.p, Ask::Skip),
                "crm" => matches!(a.cr, Ask::Skip),
                "conversations" => matches!(a.cv, Ask::Skip),
                _ => false,
            };
            if !skipped {
                let mut v = assemble(q, k, &a, t0).await;
                v["part"] = serde_json::json!(part);
                v["partial"] = serde_json::json!(true);
                if tx.send(v).is_err() {
                    return serde_json::json!({});
                }
            }
        }
    }
}

/// The answer from the asks back so far: the groups, the fused Best list, the notes (a waiting ask says `pending`).
async fn assemble(q: &str, k: u32, a: &Asks, t0: Instant) -> serde_json::Value {
    use serde_json::json;
    let (mut private, mut network, mut public, mut notes, mut cells, mut crm, mut convs) = (vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
    let hrow = |x: &hybrid_api::Hit| {
        // a thing on several machines is one row (search.hybrid folds them): the first is where it opens
        let ms: Vec<&str> = x.machine.split(',').filter(|m| !m.is_empty()).collect();
        let machine = if ms.len() > 2 { format!("{} +{}", ms[0], ms.len() - 1) } else { x.machine.replace(',', ", ") };
        json!({"title": x.title, "snippet": x.snippet, "source": x.source, "machine": machine, "path": x.path,
            "also": if ms.len() > 2 { json!(ms[1..].to_vec()) } else { json!([]) },
            "score": x.score, "open": open_of(&x.source, "", &x.path, ""), "from": "search.hybrid"})
    };
    match &a.h {
        Ask::Ok(r) => {
            notes.push(json!({"cell": "search.hybrid", "ok": true, "hits": r.hits.len(), "took_ms": r.took_ms as u64, "reranked": r.reranked}));
            for x in &r.hits {
                if group_of(&x.acl) == "public" { public.push(hrow(x)) } else { private.push(hrow(x)) }
            }
        }
        other => notes.extend(ask_note(other, "search.hybrid", None)),
    }
    match &a.p {
        Ask::Ok(r) => {
            notes.push(json!({"cell": "search.hybrid", "ask": "public", "ok": true, "hits": r.hits.len(), "took_ms": r.took_ms as u64}));
            // the public-only answer first: it is ranked among public hits alone
            let mut both: Vec<serde_json::Value> = r.hits.iter().filter(|x| group_of(&x.acl) == "public").map(hrow).collect();
            both.append(&mut public);
            public = both;
        }
        other => notes.extend(ask_note(other, "search.hybrid", Some("public"))),
    }
    match &a.m {
        Ask::Ok(r) => {
            notes.push(json!({"cell": "search.mesh", "ok": true, "hits": r.hits.len(), "took_ms": r.took_ms as u64, "answered": r.answered, "asked": r.asked,
                "nodes": r.nodes.iter().map(|n| json!({"node": n.node, "ok": n.ok, "hits": n.hits, "took_ms": n.took_ms as u64, "err": n.err.chars().take(120).collect::<String>()})).collect::<Vec<_>>()}));
            for x in &r.hits {
                let first = x.locations.first();
                let (machine, path) = first.map(|l| (l.node.clone(), l.path.clone())).unwrap_or_default();
                let row = json!({"title": x.title, "snippet": x.snippet, "source": x.sources.join(", "), "kind": x.kind, "machine": machine, "path": path,
                    "also": x.locations.iter().skip(1).map(|l| l.node.clone()).collect::<Vec<_>>(), "score": x.score,
                    "open": open_of("", &x.kind, &path, &x.key), "origin": if x.kind == "repo" { origin_of(&x.key) } else { None }, "from": "search.mesh"});
                // every cell of the mesh (search.mesh's registry and cells sources) joins the Cells group
                if x.kind == "cell" { cells.push(row) } else { network.push(row) }
            }
        }
        other => notes.extend(ask_note(other, "search.mesh", None)),
    }
    match &a.cr {
        Ask::Ok(r) => {
            notes.push(json!({"cell": "search.hybrid", "ask": "crm", "ok": true, "hits": r.hits.len(), "took_ms": r.took_ms as u64}));
            crm = r.hits.iter().filter(|x| group_of(&x.acl) == "private").map(hrow).collect();
        }
        other => notes.extend(ask_note(other, "search.hybrid", Some("crm"))),
    }
    let mut cell_rows = vec![];
    match &a.cs {
        Ask::Ok(r) => {
            notes.push(json!({"cell": "cell.search", "ok": true, "hits": r.hits.len(), "took_ms": r.took_us / 1000, "indexed": r.indexed}));
            for x in r.hits.iter().take(k as usize) {
                let t = &x.trust;
                let mut trust = vec![if t.signer == "ours" { "signed by ce-net".to_string() } else { format!("{} signer", t.signer) }];
                if !t.review.is_empty() {
                    trust.push(format!("review {}", t.review));
                }
                if !t.advisories.is_empty() {
                    trust.push(format!("{} advisories", t.advisories.len()));
                }
                let ops: Vec<&str> = x.ops.iter().map(|o| o.name.as_str()).filter(|n| *n != "HealthOp").take(6).collect();
                cell_rows.push(json!({"title": if x.title.is_empty() || x.title == x.api { x.api.clone() } else { format!("{} · {}", x.api, x.title) },
                    "snippet": x.about, "source": "cell.search", "kind": "cell", "machine": format!("running on {}", x.running_on),
                    "path": format!("{}{}", x.repo, if ops.is_empty() { String::new() } else { format!(" · ops {}", ops.join(", ")) }),
                    "trust": trust, "score": x.score, "open": open_of("cells", "cell", "", &x.api), "from": "cell.search", "api": x.api}));
            }
        }
        other => notes.extend(ask_note(other, "cell.search", None)),
    }
    match &a.cv {
        Ask::Ok(r) => {
            notes.push(json!({"cell": "ai.session", "ok": true, "hits": r.conversations.len(), "total": r.total}));
            for c in &r.conversations {
                let title = if c.title.is_empty() { format!("{} conversation", c.provider) } else { c.title.clone() };
                convs.push(json!({"title": title, "snippet": format!("{} turns · {}", c.turns, c.cwd), "source": "ai.session", "kind": "conversation",
                    "machine": c.node, "path": format!("conv:{}", c.id), "open": format!("/conversations/?c={}", c.id), "from": "ai.session", "score": 0.0}));
            }
        }
        other => notes.extend(ask_note(other, "ai.session", None)),
    }
    let key = |v: &serde_json::Value| format!("{}\n{}\n{}", v["path"], v["title"], v["machine"]);
    let n = k as usize;
    // one row a record: crm.rs and search-hybrid-collect both index a call or person (crm/calls/<id>, calls/<id>)
    let crm = first_unique(crm, n, |v| v["open"].as_str().map(String::from).unwrap_or_else(|| key(v)));
    let in_crm: std::collections::BTreeSet<String> = crm.iter().map(key).collect();
    let private: Vec<serde_json::Value> = private.into_iter().filter(|v| !in_crm.contains(&key(v))).collect();
    // floors (section 8 fix 2): a CRM row only when a query word is in its name or its score clears 0.6; a cell only
    // when a query word is in its api or title, or its score clears 0.8 (cell.search scored paypal.payment 0.75 for
    // "gate deploy canary" by its ops' words)
    let words = name_words(q);
    let crm: Vec<serde_json::Value> = crm.into_iter().filter(|v| covers_name(&words, v["title"].as_str().unwrap_or("")) || v["score"].as_f64().unwrap_or(0.0) >= 0.6).collect();
    let mut all_cells: Vec<serde_json::Value> = cell_rows.into_iter().chain(cells).collect();
    all_cells.retain(|v| covers_name(&words, v["title"].as_str().unwrap_or("")) || v["score"].as_f64().unwrap_or(0.0) >= 0.8);
    let cells = first_unique(all_cells, n, thing_of);
    // papers in their own group (section 8 fix 7): "VIPER-seq" is not what "Vivi" asks for
    let (papers, public): (Vec<serde_json::Value>, Vec<serde_json::Value>) = public.into_iter().partition(is_paper);
    let (mut private, mut network, mut public, mut crm, mut papers) = (first_unique(private, n, key), first_unique(network, n, key), first_unique(public, n, key), crm, first_unique(papers, 5, key));
    let mut convs = convs;
    pin_links(&mut [&mut private, &mut network, &mut public, &mut crm, &mut papers, &mut convs]).await;
    let best = best_of(&[("private", 1.0, &private), ("crm", 1.0, &crm), ("conversations", 1.0, &convs), ("network", 1.0, &network), ("cells", 0.8, &cells), ("public", 0.6, &public)], n);
    json!({"q": q, "per_group": k, "best": best, "private": private, "network": network, "cells": cells, "crm": crm, "conversations": convs, "public": public, "papers": papers,
        "notes": notes, "took_ms": t0.elapsed().as_millis() as u64})
}

/// `GET /api/search?q=&stream=1`: the same answer as `/api/search`, as JSON lines: one line each time an ask comes back
/// (the answer so far, `"partial": true`, `"part"` naming the group), then the whole answer (`"part": "done"`). Same
/// asks as the same viewer; the connection closes after the last line.
async fn search_stream(s: &mut TcpStream, person: &str, abilities: &[String], q: &str, k: u32) {
    use tokio::io::AsyncWriteExt;
    let head = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nCache-Control: no-store\r\nX-Accel-Buffering: no\r\nX-Content-Type-Options: nosniff\r\nVary: X-Ce-Person\r\nConnection: close\r\n\r\n";
    if s.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<serde_json::Value>();
    let work = async {
        let mut v = search(person, abilities, q, k, Some(&tx)).await;
        v["part"] = serde_json::json!("done");
        drop(tx);
        v
    };
    tokio::pin!(work);
    let mut done: Option<serde_json::Value> = None;
    loop {
        tokio::select! {
            v = &mut work, if done.is_none() => { done = Some(v); }
            m = rx.recv() => match m {
                Some(v) => {
                    let mut line = serde_json::to_vec(&v).unwrap_or_default();
                    line.push(b'\n');
                    if s.write_all(&line).await.is_err() { return; }
                    let _ = s.flush().await;
                }
                None => break,
            }
        }
    }
    let v = match done {
        Some(v) => v,
        None => work.await,
    };
    let mut line = serde_json::to_vec(&v).unwrap_or_default();
    line.push(b'\n');
    let _ = s.write_all(&line).await;
    let _ = s.flush().await;
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

/// open.with `Open` as the viewer: the ce-ref checked here (either written form), the answer passed on as open.with
/// gave it (it never names an opener the viewer may not use). -> (status, body).
async fn open_ref(person: &str, abilities: &[String], r: &str, mode: &str, device: &str) -> (&'static str, serde_json::Value) {
    use open_with_api as ow;
    use serde_json::json;
    let parsed = match ow::CeRef::parse(r) {
        Ok(p) => p,
        Err(e) => return ("200 OK", json!({"Unknown": {"reason": e.to_string()}})),
    };
    let mode = if mode.eq_ignore_ascii_case("edit") { ow::Mode::Edit } else { ow::Mode::View };
    let device = match device {
        "Phone" => ow::Device::Phone,
        "Wall" => ow::Device::Wall,
        _ => ow::Device::Desk,
    };
    let c = match client::<ow::OpenWith>(person, abilities).await {
        Ok(c) => c,
        Err(e) => return ("503 Service Unavailable", json!({"error": format!("open.with: {e}")})),
    };
    match c.call::<ow::Open>(&ow::OpenReq { r: parsed, mode, device }).await {
        Ok(o) => ("200 OK", serde_json::to_value(&o).unwrap_or_default()),
        Err(e) => ("503 Service Unavailable", json!({"error": format!("open.with: {e}")})),
    }
}

fn status_name<T: std::fmt::Debug>(s: &T) -> String {
    let d = format!("{s:?}");
    d.split(['(', ' ', '{']).next().unwrap_or("").to_string()
}

/// One day's calls as the viewer (crm.calls List since `from`, cut at `to`). -> (status, {"calls": [..]}).
async fn home_calls(person: &str, abilities: &[String], path: &str) -> (&'static str, serde_json::Value) {
    use crm_calls_api as cc;
    use serde_json::json;
    if for_cell(abilities, "crm.calls", "crm.calls:read").is_none() {
        return ("403 Forbidden", json!({"error": "crm.calls is not open to you", "need": "crm.calls:read"}));
    }
    let (from, to) = day_range(query(path, "from").and_then(|v| v.parse().ok()), query(path, "to").and_then(|v| v.parse().ok()), now_s() as i64);
    let c = match client::<cc::CrmCalls>(person, abilities).await {
        Ok(c) => c,
        Err(e) => return ("503 Service Unavailable", json!({"error": format!("crm.calls: {e}")})),
    };
    match c.call::<cc::List>(&cc::ListReq { person: None, status: None, since_s: Some(from) }).await {
        Ok(Ok(rows)) => {
            let calls: Vec<serde_json::Value> = rows
                .into_iter()
                .filter(|x| x.at_s >= from && x.at_s < to)
                .map(|x| {
                    let channel = match &x.channel {
                        cc::Channel::Other(o) => o.clone(),
                        ch => status_name(ch),
                    };
                    json!({"id": x.id, "person": x.person, "company": x.company, "channel": channel, "at_s": x.at_s, "status": status_name(&x.status)})
                })
                .collect();
            ("200 OK", json!({"calls": calls, "from": from, "to": to}))
        }
        Ok(Err(e)) => ("503 Service Unavailable", json!({"error": format!("crm.calls: {e:?}")})),
        Err(e) => ("503 Service Unavailable", json!({"error": format!("crm.calls: {e}")})),
    }
}

/// How far ahead the Home promises slot looks.
const PROMISES_AHEAD_MS: u64 = 7 * 86_400_000;

/// Open promises due within a week or overdue, as the viewer (crm.promises List). -> (status, {"promises": [..]}).
async fn home_promises(person: &str, abilities: &[String]) -> (&'static str, serde_json::Value) {
    use crm_promises_api as cp;
    use serde_json::json;
    if for_cell(abilities, "crm.promises", "crm.promises:read").is_none() {
        return ("403 Forbidden", json!({"error": "crm.promises is not open to you", "need": "crm.promises:read"}));
    }
    let c = match client::<cp::CrmPromises>(person, abilities).await {
        Ok(c) => c,
        Err(e) => return ("503 Service Unavailable", json!({"error": format!("crm.promises: {e}")})),
    };
    let req = cp::ListReq { person: String::new(), company: String::new(), status: Some(cp::Status::Open), tenant: String::new() };
    match c.call::<cp::List>(&req).await {
        Ok(Ok(rows)) => {
            let dues: Vec<(u64, bool)> = rows.iter().map(|p| (p.due, p.status == cp::Status::Open)).collect();
            let out: Vec<serde_json::Value> = due_soon(&dues, ui_chain::now_ms(), PROMISES_AHEAD_MS)
                .into_iter()
                .map(|(i, overdue)| {
                    let p = &rows[i];
                    json!({"id": p.id, "text": p.text, "to": p.to, "to_name": p.to_name, "company": p.company, "due": p.due, "overdue": overdue})
                })
                .collect();
            ("200 OK", json!({"promises": out}))
        }
        Ok(Err(e)) => ("503 Service Unavailable", json!({"error": format!("crm.promises: {e:?}")})),
        Err(e) => ("503 Service Unavailable", json!({"error": format!("crm.promises: {e}")})),
    }
}

/// How long the shell waits for one slot's `when` op before it keeps the slot and lets it show its own state.
const WHEN_MS: u64 = 3_000;

/// The Home tab: home.shell `Layout` as the viewer, then each slot's `when` asked as the viewer at once. A slot whose
/// `when` answered "nothing" is dropped; one whose cell did not answer in [`WHEN_MS`] stays (a child fails alone).
/// Order: area, then urgency (overdue, due today, waiting on the viewer, news, quiet), then order.
async fn home_layout(person: &str, abilities: &[String], path: &str) -> (&'static str, serde_json::Value) {
    use home_shell_api as hs;
    use serde_json::json;
    let t0 = Instant::now();
    let device = match query(path, "device").as_deref() {
        Some("Phone") => hs::Device::Phone,
        Some("Wall") => hs::Device::Wall,
        _ => hs::Device::Desk,
    };
    let c = match client::<hs::HomeShell>(person, abilities).await {
        Ok(c) => c,
        Err(e) => return ("503 Service Unavailable", json!({"error": format!("home.shell: {e}")})),
    };
    let slots = match c.call::<hs::Layout>(&hs::LayoutReq { device }).await {
        Ok(s) => s,
        Err(e) => return ("503 Service Unavailable", json!({"error": format!("home.shell: {e}")})),
    };
    let names: Vec<String> = slots.iter().map(|s| s.name.clone()).collect();
    let has = |n: &str| names.iter().any(|x| x == n);
    let w = Duration::from_millis(WHEN_MS);
    // each when: Some(count, urgency) answered, None no answer in time
    let calls = async {
        if !has("calls") {
            return None;
        }
        let (st, v) = tokio::time::timeout(w, home_calls(person, abilities, path)).await.ok()?;
        (st == "200 OK").then(|| (v["calls"].as_array().map(|a| a.len()).unwrap_or(0) as u32, 1u8))
    };
    let promises = async {
        if !has("promises") {
            return None;
        }
        let (st, v) = tokio::time::timeout(w, home_promises(person, abilities)).await.ok()?;
        let a = v["promises"].as_array().cloned().unwrap_or_default();
        let overdue = a.iter().any(|p| p["overdue"] == true);
        (st == "200 OK").then(|| (a.len() as u32, if overdue { 0u8 } else { 1u8 }))
    };
    let approve = async {
        if !has("approve") {
            return None;
        }
        tokio::time::timeout(w, badge(person, abilities)).await.ok().flatten().map(|n| (n, 2u8))
    };
    let (calls, promises, approve) = tokio::join!(calls, promises, approve);
    let title = |n: &str| match n {
        "calls" => "Calls today",
        "promises" => "Promises due",
        "approve" => "Waiting for you",
        "cells" => "Cells and nodes",
        "links" => "Links",
        _ => "",
    };
    let area_rank = |a: &hs::Area| match a {
        hs::Area::Head => 0,
        hs::Area::Main => 1,
        hs::Area::Side => 2,
    };
    let mut out: Vec<(u8, u8, u16, serde_json::Value)> = Vec::new();
    let mut dropped = Vec::new();
    for s in slots {
        let offer = match s.name.as_str() {
            "calls" => calls,
            "promises" => promises,
            "approve" => approve,
            _ => None,
        };
        if matches!(offer, Some((0, _))) {
            dropped.push(s.name.clone());
            continue;
        }
        let urgency = offer.map(|(_, u)| u).unwrap_or(4);
        let area = match s.area {
            hs::Area::Head => "Head",
            hs::Area::Main => "Main",
            hs::Area::Side => "Side",
        };
        out.push((area_rank(&s.area), urgency, s.order, json!({"name": s.name, "title": title(&s.name), "cell": s.cell, "major": s.major, "view": s.view, "area": area,
            "order": s.order, "offer": offer.map(|(count, urgency)| json!({"count": count, "urgency": urgency}))})));
    }
    out.sort_by_key(|(a, u, o, _)| (*a, *u, *o));
    ("200 OK", json!({"slots": out.into_iter().map(|x| x.3).collect::<Vec<_>>(), "nothing_for_you": dropped, "took_ms": t0.elapsed().as_millis() as u64}))
}

/// The Home links rows: state.store `home/links` (one row a ref: `{"ref", "title", "note"}`), read with this cell's own
/// grant for that collection (`$APPS_SHELL_LINKS_CAP`, default `<cell keys>/apps.shell/state-home-links.hex`, a
/// `state.home.links.read` chain). Rows whose `ref` is not a ce-ref are dropped. -> (status, {"links": [..]}).
async fn home_links() -> (&'static str, serde_json::Value) {
    use serde_json::json;
    use state_store_api as ss;
    let cap_file = env_or("APPS_SHELL_LINKS_CAP", format!("{}/.local/share/ce/cell-keys/apps.shell/state-home-links.hex", home()));
    let Ok(cap) = std::fs::read_to_string(&cap_file) else {
        return ("503 Service Unavailable", json!({"error": "the links store is not granted on this node"}));
    };
    let Ok(tok) = ui_chain::source_token::<ss::StateStore>(&["state.store:read"], 30, None) else {
        return ("503 Service Unavailable", json!({"error": "state.store: no grant to read"}));
    };
    let mut c = match tokio::time::timeout(Duration::from_secs(4), ce_wire::Client::<ss::StateStore>::by_name(tok)).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return ("503 Service Unavailable", json!({"error": format!("state.store: {e}")})),
        Err(_) => return ("503 Service Unavailable", json!({"error": "state.store: no replica answered in 4 s"})),
    };
    c.deadline = Duration::from_secs(10);
    let req = ss::ListReq { at: ss::Coll { app: "home".into(), coll: "links".into() }, prefix: String::new(), since: 0, limit: 64, include_deleted: false, cap: cap.trim().to_string() };
    match c.call::<ss::List>(&req).await {
        Ok(page) => {
            let mut docs = page.docs;
            docs.sort_by(|a, b| a.key.cmp(&b.key));
            ("200 OK", json!({ "links": links_rows(docs.iter().map(|d| d.value.as_str())) }))
        }
        Err(e) => ("503 Service Unavailable", json!({"error": format!("state.store: {e}")})),
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

/// A person id safe to put in a caveat: 1 to 64 of `[a-z0-9._-]`.
fn person_ok(p: &str) -> bool {
    (1..=64).contains(&p.len()) && p.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(&c))
}

/// The viewer's grant for these abilities: the cached one while it has 2 minutes left and the same abilities (a
/// people-file change mints a new one at once), else a fresh Mint.
async fn viewer(person: &str, abilities: &[String]) -> Result<Arc<ViewerGrant>, String> {
    use door_api::*;
    let mut want = viewer_grant(abilities);
    if want.is_empty() {
        return Err("refused".into());
    }
    // the feed cell keeps read state per person: the verified person rides in the grant as a row caveat of that cell
    // (`feed.view:where:person=<id>`, covered by mint-for:feed.view); never a request field, never the people file's
    if want.iter().any(|a| a.starts_with("feed.view:")) && person_ok(person) {
        want.push(format!("feed.view:where:person={person}"));
        want.sort();
        want.dedup();
    }
    // the notes cell writes a note as its author only: the verified person rides in the grant as its author caveat
    if want.iter().any(|a| a.starts_with("arch.notes:")) && person_ok(person) {
        want.push(format!("arch.notes:where:author={person}"));
        want.sort();
        want.dedup();
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

/// The ops the Feed relay passes, and nothing else.
const FEED_OPS: &[&str] = &["Page", "Topic", "Changes", "Mark", "Close"];
const FEED_BODY_MAX: usize = 64 * 1024;

/// The view modules fetched from feed.view `Ui`, by sha256: the immutable copies at `/_c/<sha256>.js`.
static BUNDLES: std::sync::OnceLock<std::sync::Mutex<BTreeMap<String, Arc<Vec<u8>>>>> = std::sync::OnceLock::new();

/// `GET /v/feed.js`: the Feed view module as the viewer's own `Ui` call hands it over, checked against its hash and the
/// majors this shell speaks (contract 2, kit 1).
async fn feed_bundle(person: &str, abilities: &[String]) -> Result<(String, Arc<Vec<u8>>), String> {
    use feed_view_api::*;
    use sha2::Digest;
    let ab = for_cell(abilities, "feed.view", "feed.view:read").ok_or("refused")?;
    let c = client::<FeedView>(person, &ab).await?;
    let b = match c.call::<Ui>(&UiReq { view: "Feed".into() }).await {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return Err(format!("{e:?}")),
        Err(e) => return Err(e.to_string()),
    };
    let sha: String = sha2::Sha256::digest(&b.bytes).iter().map(|x| format!("{x:02x}")).collect();
    if sha != b.sha256 || !b.speaks.contains(&2) || b.kit != 1 || b.bytes.is_empty() || b.bytes.len() > VIEW_MAX {
        return Err("the bundle does not match its hash, or speaks other majors".into());
    }
    let bytes = Arc::new(b.bytes);
    BUNDLES.get_or_init(Default::default).lock().unwrap().insert(sha.clone(), bytes.clone());
    Ok((sha, bytes))
}

/// `POST /api/cell/feed.view/<Op>`: one op of the feed cell as the viewer (JSON in, JSON out). A refusal of the cell
/// is a 403 with no more than that; the cell's own errors keep their kind.
async fn feed_relay(person: &str, abilities: &[String], op: &str, body: &[u8]) -> (&'static str, Vec<u8>) {
    use feed_view_api::*;
    let err = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
    if !FEED_OPS.contains(&op) {
        return err("404 Not Found", "no such op".into());
    }
    let Some(ab) = for_cell(abilities, "feed.view", "feed.view:read") else { return err("403 Forbidden", "refused".into()) };
    let c = match client::<FeedView>(person, &ab).await {
        Ok(mut c) => {
            // Page may build the card index of a fresh process (about 10 s); Changes holds up to 25 s
            c.deadline = Duration::from_secs(if op == "Changes" { 40 } else { 25 });
            c
        }
        Err(e) if e == "refused" => return err("403 Forbidden", "refused".into()),
        Err(e) => return err("503 Service Unavailable", e),
    };
    fn out<T: serde::Serialize>(r: Result<Result<T, FeedError>, ce_wire::WireError>) -> (&'static str, Vec<u8>) {
        let e = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
        match r {
            Ok(Ok(v)) => ("200 OK", json(&v)),
            Ok(Err(FeedError::NoSuchTopic(t))) => e("404 Not Found", format!("no such topic {t}")),
            Ok(Err(FeedError::NoSuchView(t))) => e("404 Not Found", format!("no such view {t}")),
            Ok(Err(FeedError::Source(m))) | Ok(Err(FeedError::Store(m))) => e("502 Bad Gateway", m),
            Err(ce_wire::WireError::Refused(_)) => e("403 Forbidden", "refused".into()),
            Err(x) => e("503 Service Unavailable", x.to_string()),
        }
    }
    macro_rules! call_op {
        ($op:ty) => {{
            match serde_json::from_slice(body) {
                Ok(req) => out(c.call::<$op>(&req).await),
                Err(x) => err("400 Bad Request", format!("body: {x}")),
            }
        }};
    }
    match op {
        "Page" => call_op!(Page),
        "Topic" => call_op!(Topic),
        "Changes" => call_op!(Changes),
        "Mark" => call_op!(Mark),
        _ => call_op!(Close),
    }
}

/// The ops the Architecture relay passes (slice 1 the internal reads, slice 2 the demo reads and Changes), and nothing
/// else. The cell's own gate decides each (a `see`-only viewer gets 403 on the internal ones).
const ARCH_OPS: &[&str] = &["GraphFull", "DetailFull", "Sources", "Graph", "Detail", "Story", "Progress", "Changes"];

/// The Architecture view modules fetched from arch.map `Ui`, by sha256 (their `/_c/<sha256>.js` copies).
static ARCH_BUNDLES: std::sync::OnceLock<std::sync::Mutex<BTreeMap<String, Arc<Vec<u8>>>>> = std::sync::OnceLock::new();

/// `GET /v/architecture.js`: the view module arch.map ships, asked as the viewer (`Ui{view: "Architecture"}`), checked
/// against its own sha256 and the majors this shell speaks (contract 1, kit 1). None held: refused (404 to the browser).
async fn arch_bundle(person: &str, abilities: &[String]) -> Result<(String, Arc<Vec<u8>>), String> {
    use arch_map_api::*;
    use sha2::Digest;
    let ab = for_cell(abilities, "arch.map", "arch.map:see").ok_or("refused")?;
    let c = client::<ArchMap>(person, &ab).await?;
    let b = match c.call::<Ui>(&UiReq { view: "Architecture".into() }).await {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return Err(format!("{e:?}")),
        Err(e) => return Err(e.to_string()),
    };
    let sha: String = sha2::Sha256::digest(&b.bytes).iter().map(|x| format!("{x:02x}")).collect();
    if sha != b.sha256 || !b.speaks.contains(&1) || b.kit != 1 || b.bytes.is_empty() || b.bytes.len() > VIEW_MAX {
        return Err("the bundle does not match its hash, or speaks other majors".into());
    }
    let bytes = Arc::new(b.bytes);
    ARCH_BUNDLES.get_or_init(Default::default).lock().unwrap().insert(sha.clone(), bytes.clone());
    Ok((sha, bytes))
}

/// `POST /api/cell/arch.map/<Op>`: GraphFull, DetailFull or Sources as the viewer (JSON in, JSON out), with a grant
/// wire.ws-door minted for their own `arch.map:*` rights, so arch.map's gate decides (a viewer with `see` alone gets
/// 403 from the cell on these internal ops).
async fn arch_relay(person: &str, abilities: &[String], op: &str, body: &[u8]) -> (&'static str, Vec<u8>) {
    use arch_map_api::*;
    let err = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
    if !ARCH_OPS.contains(&op) {
        return err("404 Not Found", "no such op".into());
    }
    let Some(ab) = for_cell(abilities, "arch.map", "arch.map:see") else { return err("403 Forbidden", "refused".into()) };
    let mut c = match client::<ArchMap>(person, &ab).await {
        Ok(c) => c,
        Err(e) if e == "refused" => return err("403 Forbidden", "refused".into()),
        Err(e) => return err("503 Service Unavailable", e),
    };
    // Changes holds the call open until the map changes (at most 30 s)
    if op == "Changes" {
        c.deadline = Duration::from_secs(35);
    }
    fn out<T: serde::Serialize>(r: Result<Result<T, ArchError>, ce_wire::WireError>) -> (&'static str, Vec<u8>) {
        let e = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
        match r {
            Ok(Ok(v)) => ("200 OK", json(&v)),
            Ok(Err(ArchError::NoSuchNode(t))) => e("404 Not Found", format!("no such node {t}")),
            Ok(Err(ArchError::NoSuchView(t))) => e("404 Not Found", format!("no such view {t}")),
            Ok(Err(ArchError::Invalid(m))) => e("400 Bad Request", m),
            Ok(Err(ArchError::Source(m))) | Ok(Err(ArchError::Store(m))) => e("502 Bad Gateway", m),
            Err(ce_wire::WireError::Refused(_)) => e("403 Forbidden", "refused".into()),
            Err(x) => e("503 Service Unavailable", x.to_string()),
        }
    }
    match op {
        "GraphFull" => match serde_json::from_slice::<View>(body) {
            Ok(v) => out(c.call::<GraphFull>(&v).await),
            Err(x) => err("400 Bad Request", format!("body: {x}")),
        },
        "DetailFull" => match serde_json::from_slice::<String>(body) {
            Ok(id) => out(c.call::<DetailFull>(&id).await),
            Err(x) => err("400 Bad Request", format!("body: {x}")),
        },
        "Graph" => match serde_json::from_slice::<View>(body) {
            Ok(v) => out(c.call::<Graph>(&v).await),
            Err(x) => err("400 Bad Request", format!("body: {x}")),
        },
        "Detail" => match serde_json::from_slice::<String>(body) {
            Ok(id) => out(c.call::<Detail>(&id).await),
            Err(x) => err("400 Bad Request", format!("body: {x}")),
        },
        "Story" => out(c.call::<Story>(&()).await),
        "Progress" => out(c.call::<Progress>(&()).await),
        "Changes" => match serde_json::from_slice::<ChangesReq>(body) {
            Ok(r) => out(c.call::<Changes>(&ChangesReq { wait_ms: r.wait_ms.min(30_000), ..r }).await),
            Err(x) => err("400 Bad Request", format!("body: {x}")),
        },
        _ => match c.call::<Sources>(&()).await {
            Ok(v) => ("200 OK", json(&v)),
            Err(ce_wire::WireError::Refused(_)) => err("403 Forbidden", "refused".into()),
            Err(x) => err("503 Service Unavailable", x.to_string()),
        },
    }
}

/// `POST /api/cell/arch.notes/<Op>`: the notes strip's Put, List or Close as the viewer, with a grant wire.ws-door mints
/// for their own `arch.notes:*` rights plus `arch.notes:where:author=<them>` (viewer()), so arch.notes decides who may
/// write as whom. A viewer without the op's right gets 403 here, before any cell is asked.
async fn notes_relay(person: &str, abilities: &[String], op: &str, body: &[u8]) -> (&'static str, Vec<u8>) {
    use arch_notes_api::*;
    let err = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
    let Some((_, need)) = NOTES_OPS.iter().find(|(o, _)| *o == op) else { return err("404 Not Found", "no such op".into()) };
    let Some(ab) = for_cell(abilities, "arch.notes", need) else { return err("403 Forbidden", "refused".into()) };
    let c = match client::<ArchNotes>(person, &ab).await {
        Ok(c) => c,
        Err(e) if e == "refused" => return err("403 Forbidden", "refused".into()),
        Err(e) => return err("503 Service Unavailable", e),
    };
    fn out<T: serde::Serialize>(r: Result<Result<T, NoteError>, ce_wire::WireError>) -> (&'static str, Vec<u8>) {
        let e = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
        match r {
            Ok(Ok(v)) => ("200 OK", json(&v)),
            Ok(Err(NoteError::Denied(m))) => e("403 Forbidden", m),
            Ok(Err(NoteError::NoSuchNote(m))) => e("404 Not Found", format!("no such note {m}")),
            Ok(Err(NoteError::Invalid(m))) => e("400 Bad Request", m),
            Ok(Err(NoteError::Store(m))) => e("502 Bad Gateway", m),
            Err(ce_wire::WireError::Refused(_)) => e("403 Forbidden", "refused".into()),
            Err(x) => e("503 Service Unavailable", x.to_string()),
        }
    }
    match op {
        "Put" => match serde_json::from_slice::<Note>(body) {
            Ok(n) => out(c.call::<Put>(&n).await),
            Err(x) => err("400 Bad Request", format!("body: {x}")),
        },
        "List" => match serde_json::from_slice::<ListReq>(body) {
            Ok(r) => match c.call::<List>(&ListReq { max: r.max.min(100), ..r }).await {
                Ok(v) => ("200 OK", json(&v)),
                Err(ce_wire::WireError::Refused(_)) => err("403 Forbidden", "refused".into()),
                Err(x) => err("503 Service Unavailable", x.to_string()),
            },
            Err(x) => err("400 Bad Request", format!("body: {x}")),
        },
        _ => match serde_json::from_slice::<String>(body) {
            Ok(id) => out(c.call::<Close>(&id).await),
            Err(x) => err("400 Bad Request", format!("body: {x}")),
        },
    }
}

/// The Approvals badge: the cards waiting for this viewer (decision.card, shown and more), else the inbox's row count
/// while decision.card is not theirs or does not answer.
async fn badge(person: &str, abilities: &[String]) -> Option<u32> {
    if let Some(b) = decisions(person, abilities).await.ok().flatten() {
        return Some(b.cards.len() as u32 + b.more);
    }
    use approve_inbox_api as ai;
    for_cell(abilities, "approve.inbox", "approve.inbox:read")?;
    let c = client::<ai::ApproveInbox>(person, abilities).await.ok()?;
    c.call::<ai::Badge>(&()).await.ok().map(|w| w.waiting)
}

/// decision.card `BoardOp` as the viewer: Ok(None) when the viewer holds no `decision.card:read`.
async fn decisions(person: &str, abilities: &[String]) -> Result<Option<decision_card_api::Board>, String> {
    if for_cell(abilities, "decision.card", "decision.card:read").is_none() {
        return Ok(None);
    }
    let c = client::<decision_card_api::DecisionCard>(person, abilities).await?;
    c.call::<decision_card_api::BoardOp>(&()).await.map(Some).map_err(|e| e.to_string())
}

async fn tabs(f: &Face, abilities: &[String], person: &str) -> TabsOut {
    let live = f.live.read().await.clone();
    let rows = tabs_for(abilities, &live, ui_chain::now_ms());
    let mut tabs = Vec::new();
    for t in rows {
        let b = if t.system == "APPROVALS" { badge(person, abilities).await } else { None };
        tabs.push(TabOut { view: view_of(&t.route).map(String::from), href: href_of(&t.route).map(String::from), system: t.system, title: t.title, route: t.route, badge: b });
    }
    TabsOut { person: person.into(), here: f.route.clone(), views: extra_views(abilities, &f.route), notes: ARCH_FACES.contains(&f.route.as_str()).then(|| notes_cell(abilities)).flatten().map(String::from), tabs, at_ms: ui_chain::now_ms() }
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
        "/api/inbox" | "/api/card" if has("/approvals/") || has("/home/") => {
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
        "/api/decisions" if has("/approvals/") => match decisions(person, abilities).await {
            Ok(Some(b)) => ("200 OK", json(&b)),
            Ok(None) => refused,
            Err(e) => down(e),
        },
        "/api/cellmap" if has("/core/") || has("/home/") || (!mine.is_empty() && covers(abilities, "open.with:use")) => {
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
            ("200 OK", json(&search(person, abilities, q, k, None).await))
        }
        "/api/rec" if has("/search/") || (!mine.is_empty() && covers(abilities, "open.with:use")) => {
            let r = query_text(path, "r").unwrap_or_default();
            let q = query_text(path, "q").unwrap_or_default();
            let (st, body) = view_rec(person, abilities, r.trim(), q.trim()).await;
            (st, json(&body))
        }
        "/api/ref" if has("/search/") || (!mine.is_empty() && covers(abilities, "open.with:use")) => {
            let r = query_text(path, "r").unwrap_or_default();
            let (st, body) = view_ref(person, abilities, r.trim()).await;
            (st, json(&body))
        }
        "/api/open" if !mine.is_empty() && covers(abilities, "open.with:use") => {
            let r = query_text(path, "r").unwrap_or_default();
            let mode = query(path, "mode").unwrap_or_default();
            let device = query(path, "device").unwrap_or_default();
            let (st, body) = open_ref(person, abilities, r.trim(), &mode, &device).await;
            (st, json(&body))
        }
        "/api/layout" if has("/home/") => {
            let (st, body) = home_layout(person, abilities, path).await;
            (st, json(&body))
        }
        "/api/calls" if has("/home/") => {
            let (st, body) = home_calls(person, abilities, path).await;
            (st, json(&body))
        }
        "/api/promises" if has("/home/") => {
            let (st, body) = home_promises(person, abilities).await;
            (st, json(&body))
        }
        "/api/links" if has("/home/") && covers(abilities, "home.shell:links") => {
            let (st, body) = home_links().await;
            (st, json(&body))
        }
        _ => refused,
    }
}

/// `POST /api/cell/ai.session/<Op>`: an op of AI_OPS as the viewer (JSON in, JSON out), with a grant wire.ws-door
/// minted for their own `ai.session:*` rights, so ai.session's gate decides and its tenant rule cuts the rows. The
/// request is checked first (`apps_shell::ai_check`: Bare harness only, Send never waits).
async fn conv_relay(person: &str, abilities: &[String], op: &str, body: &[u8]) -> (&'static str, Vec<u8>) {
    use ai_session_api::*;
    let err = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
    let Some(need) = apps_shell::ai_need(op).filter(|_| op != "Events") else { return err("404 Not Found", "no such op".into()) };
    let mut v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(x) => return err("400 Bad Request", format!("body: {x}")),
    };
    if let Err(e) = apps_shell::ai_check(op, &mut v) {
        return err("400 Bad Request", e);
    }
    let Some(ab) = apps_shell::for_cell(abilities, "ai.session", need) else { return err("403 Forbidden", "refused".into()) };
    let c = match client::<AiSession>(person, &ab).await {
        Ok(mut c) => {
            // Read of an imported row reads the transcript on its node (one adapter call); Open and Fork start a
            // session; Ask waits for its answer
            c.deadline = Duration::from_secs(match op {
                "Ask" => 300,
                "Open" | "Fork" | "Continue" => 60,
                _ => 25,
            });
            c
        }
        Err(e) if e == "refused" => return err("403 Forbidden", "refused".into()),
        Err(e) => return err("503 Service Unavailable", e),
    };
    fn out<T: serde::Serialize>(r: Result<Result<T, Refusal>, ce_wire::WireError>) -> (&'static str, Vec<u8>) {
        let e = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
        match r {
            Ok(Ok(v)) => ("200 OK", json(&v)),
            Ok(Err(Refusal::NotFound(m))) => e("404 Not Found", m),
            Ok(Err(Refusal::Invalid(m))) | Ok(Err(Refusal::Shape(m))) => e("400 Bad Request", m),
            Ok(Err(Refusal::Denied { .. })) => e("403 Forbidden", "refused".into()),
            Ok(Err(Refusal::Busy)) => e("409 Conflict", "busy: a turn is running, or a live process holds this conversation".into()),
            Ok(Err(Refusal::Policy { reason })) => e("409 Conflict", reason),
            Ok(Err(Refusal::NoPath(m))) => e("502 Bad Gateway", m),
            Ok(Err(x)) => e("502 Bad Gateway", format!("{x:?}")),
            Err(ce_wire::WireError::Refused(_)) => e("403 Forbidden", "refused".into()),
            Err(x) => e("503 Service Unavailable", x.to_string()),
        }
    }
    macro_rules! call_op {
        ($op:ty) => {{
            match serde_json::from_value(v) {
                Ok(req) => out(c.call::<$op>(&req).await),
                Err(x) => err("400 Bad Request", format!("body: {x}")),
            }
        }};
    }
    match op {
        "List" => call_op!(List),
        "Get" => call_op!(Get),
        "Read" => call_op!(Read),
        "Open" => call_op!(Open),
        "Send" => call_op!(Send),
        "Continue" => call_op!(Continue),
        "Fork" => call_op!(Fork),
        "Interrupt" => call_op!(Interrupt),
        "Close" => call_op!(Close),
        _ => call_op!(Ask),
    }
}

/// `GET /api/cell/ai.session/Events?conversation=<id>`: one conversation's live events as server-sent events, over one
/// ce-wire stream opened as the viewer. A viewer who may not read the row gets 403 or 404 before any stream is opened
/// (Get first). Each event is one `data:` line of the Event's JSON; a comment line every 15 s keeps the connection; it
/// ends when the conversation closes, the cell ends the stream, the page goes away, or after 30 minutes. Reconnect:
/// Read from the last turn the page holds, then Events again.
async fn conv_events(s: &mut TcpStream, person: &str, abilities: &[String], path: &str) {
    use ai_session_api::*;
    use tokio::io::AsyncWriteExt;
    let Some(conversation) = query_text(path, "conversation").filter(|c| !c.is_empty() && c.len() <= 128 && c.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))) else {
        return reply(s, "400 Bad Request", "application/json", b"{\"error\":\"conversation=<id>\"}", false).await;
    };
    let Some(ab) = apps_shell::for_cell(abilities, "ai.session", "ai.session:read") else { return reply(s, "403 Forbidden", "application/json", b"{\"error\":\"refused\"}", false).await };
    let c = match client::<AiSession>(person, &ab).await {
        Ok(c) => c,
        Err(e) if e == "refused" => return reply(s, "403 Forbidden", "application/json", b"{\"error\":\"refused\"}", false).await,
        Err(e) => return reply(s, "503 Service Unavailable", "application/json", &json(&serde_json::json!({ "error": e })), false).await,
    };
    let req = ConvReq { conversation };
    match c.call::<Get>(&req).await {
        Ok(Ok(_)) => {}
        Ok(Err(Refusal::NotFound(_))) => return reply(s, "404 Not Found", "application/json", b"{\"error\":\"no such conversation\"}", false).await,
        Ok(Err(_)) | Err(ce_wire::WireError::Refused(_)) => return reply(s, "403 Forbidden", "application/json", b"{\"error\":\"refused\"}", false).await,
        Err(e) => return reply(s, "503 Service Unavailable", "application/json", &json(&serde_json::json!({ "error": e.to_string() })), false).await,
    }
    let mut st = match c.subscribe::<Events>(&req) {
        Ok(st) => st,
        Err(ce_wire::WireError::Refused(_)) => return reply(s, "403 Forbidden", "application/json", b"{\"error\":\"refused\"}", false).await,
        Err(e) => return reply(s, "503 Service Unavailable", "application/json", &json(&serde_json::json!({ "error": e.to_string() })), false).await,
    };
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nX-Accel-Buffering: no\r\nX-Content-Type-Options: nosniff\r\nVary: X-Ce-Person\r\nConnection: close\r\n\r\nretry: 3000\n\n";
    if s.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    let until = tokio::time::Instant::now() + Duration::from_secs(1800);
    loop {
        let next = tokio::time::timeout(Duration::from_secs(15), st.next());
        let line = match tokio::time::timeout_at(until, next).await {
            Err(_) => break,
            Ok(Err(_)) => ": keep\n\n".to_string(),
            Ok(Ok(Some(Ok(e)))) => {
                let end = matches!(e, Event::State(State::Closed));
                let l = format!("data: {}\n\n", String::from_utf8_lossy(&json(&e)));
                if s.write_all(l.as_bytes()).await.is_err() || end {
                    return;
                }
                continue;
            }
            Ok(Ok(Some(Err(e)))) => format!("event: error\ndata: {}\n\n", serde_json::json!({ "error": e.to_string() })),
            Ok(Ok(None)) => break,
        };
        if s.write_all(line.as_bytes()).await.is_err() || line.starts_with("event: error") {
            return;
        }
    }
    let _ = s.write_all(b"event: end\ndata: {}\n\n").await;
}

/// `POST /api/cell/term.session/<Op>` on the Conversations face: a tmux row opened live, on the node that holds the
/// pane (`term.session#<node>`), with a grant wire.ws-door minted for the viewer's own `term.session:*` rights (their
/// `term.session:where:session=` limits ride along), so term.session's gate and scope decide on that node. TERM_OPS
/// only; `Watch` long-polls the cell's Attach stream: the first screen whose hash is not `since`, or `same` after 20 s.
async fn term_relay(person: &str, abilities: &[String], op: &str, body: &[u8]) -> (&'static str, Vec<u8>) {
    use apps_shell::term_session_api::*;
    use apps_shell::{term_need, TermReq};
    let err = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
    let Some(need) = term_need(op) else { return err("404 Not Found", "no such op".into()) };
    let req: TermReq = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(x) => return err("400 Bad Request", format!("body: {x}")),
    };
    if let Err(e) = req.check(op) {
        return err("400 Bad Request", e);
    }
    let Some(mut ab) = for_cell(abilities, "term.session", need).and_then(apps_shell::term_scope) else { return err("403 Forbidden", "refused".into()) };
    // the grant is good on the row's node only: a dial that falls back to another node's term.session is refused there
    ab.push(format!("term.session:where:node={}", req.node));
    let g = match viewer(person, &ab).await {
        Ok(g) => g,
        Err(e) if e == "refused" => return err("403 Forbidden", "refused".into()),
        Err(e) => return err("503 Service Unavailable", e),
    };
    // a hello signed for the instance name, fresh for every door the dial tries
    let (gk, node) = (g.clone(), req.node.clone());
    let mint = move || ce_gate::proof_as::<TermSession>(&gk.key, &gk.chain, &node);
    let mut c = match tokio::time::timeout(Duration::from_secs(4), ce_wire::Client::<TermSession>::by_instance_minted(&req.node, mint)).await {
        Ok(Ok(c)) => c,
        Ok(Err(ce_wire::WireError::Refused(_))) => return err("403 Forbidden", "refused".into()),
        Ok(Err(e)) => return err("503 Service Unavailable", format!("term.session on {}: {e}", req.node)),
        Err(_) => return err("503 Service Unavailable", format!("term.session on {}: no answer in 4 s", req.node)),
    };
    c.deadline = Duration::from_secs(10);
    // a dial with no such instance falls back to any term.session: never show or type into another node's panes
    match c.call::<Node>(&()).await {
        Ok(at) if at == req.node => {}
        Ok(_) | Err(ce_wire::WireError::Refused(ce_wire::Refusal::UnknownOp(_))) => return err("503 Service Unavailable", format!("term.session is not running on {}", req.node)),
        Err(ce_wire::WireError::Refused(ce_wire::Refusal::Remote(m))) => return err("502 Bad Gateway", m),
        Err(ce_wire::WireError::Refused(_)) => return err("403 Forbidden", "refused".into()),
        Err(e) => return err("503 Service Unavailable", format!("term.session on {}: {e}", req.node)),
    }
    fn out<T: serde::Serialize>(r: Result<T, ce_wire::WireError>) -> (&'static str, Vec<u8>) {
        let e = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
        match r {
            Ok(v) => ("200 OK", json(&v)),
            // the cell's own answer: a pane outside the viewer's sessions answers like one that does not exist
            Err(ce_wire::WireError::Refused(ce_wire::Refusal::Remote(m))) if m.contains("can't find") || m.contains("outside this grant") => e("404 Not Found", "no such pane".into()),
            Err(ce_wire::WireError::Refused(ce_wire::Refusal::Remote(m))) if m.contains("Frozen") => e("409 Conflict", m),
            Err(ce_wire::WireError::Refused(ce_wire::Refusal::Remote(m))) => e("502 Bad Gateway", m),
            // the gate: the viewer does not hold the op's right there
            Err(ce_wire::WireError::Refused(_)) => e("403 Forbidden", "refused".into()),
            Err(x) => e("502 Bad Gateway", x.to_string()),
        }
    }
    let hash = |t: &str| -> String {
        use sha2::Digest;
        sha2::Sha256::digest(t.as_bytes()).iter().take(8).map(|x| format!("{x:02x}")).collect()
    };
    match op {
        "List" => out(c.call::<List>(&()).await),
        "Read" => out(c.call::<Read>(&ReadReq { pane: req.pane, lines: 0 }).await.map(|s| serde_json::json!({ "width": s.width, "height": s.height, "text": s.text, "hash": hash(&s.text) }))),
        "Keys" => {
            let mut r = Ok(());
            if !req.text.is_empty() {
                r = c.call::<Keys>(&SendReq { pane: req.pane.clone(), keys: vec![req.text.clone()], literal: true, enter: false }).await;
            }
            if r.is_ok() && (!req.keys.is_empty() || req.enter) {
                r = c.call::<Keys>(&SendReq { pane: req.pane.clone(), keys: req.keys.clone(), literal: false, enter: req.enter }).await;
            }
            out(r.map(|_| serde_json::json!({ "ok": true })))
        }
        "Interrupt" => out(c.call::<Interrupt>(&PaneReq { pane: req.pane }).await.map(|_| serde_json::json!({ "ok": true }))),
        _ => {
            let mut st = match c.subscribe::<Attach>(&AttachReq { pane: req.pane }) {
                Ok(st) => st,
                Err(e) => return out::<()>(Err(e)),
            };
            let until = tokio::time::Instant::now() + Duration::from_secs(20);
            loop {
                match tokio::time::timeout_at(until, st.next()).await {
                    Ok(Some(Ok(f))) => {
                        let h = hash(&f.text);
                        if h != req.since {
                            return out::<serde_json::Value>(Ok(serde_json::json!({ "width": f.width, "height": f.height, "text": f.text, "hash": h })));
                        }
                    }
                    Ok(Some(Err(e))) => return out::<()>(Err(e)),
                    Ok(None) => return err("502 Bad Gateway", "the stream ended".into()),
                    Err(_) => return out::<serde_json::Value>(Ok(serde_json::json!({ "same": true, "hash": req.since }))),
                }
            }
        }
    }
}

/// The ops the usage strip's relay passes (the `conv.usage` types in strip.rs; `ai.usage` Read, Conversation, Events
/// underneath), and nothing else.
const USAGE_OPS: &[&str] = &["StripOp", "CostsOp", "WaitOp"];

/// Which device this shell answers on (the strip's device chip when the page names none).
fn this_device() -> Option<String> {
    let h = std::env::var("HOSTNAME").ok().filter(|h| !h.is_empty()).or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|h| h.trim().to_string()));
    h.filter(|h| !h.is_empty())
}

fn epoch_s() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// `POST /api/cell/conv.usage/<StripOp|CostsOp|WaitOp>` on the Conversations face: the strip's data as the viewer.
/// Calls `ai.usage` with a grant wire.ws-door minted for the viewer's own `ai.usage:read` (Leif's home tenant: the
/// request names no tenant, the cell answers its own, so a viewer cannot ask for another). The page is given the typed
/// [`strip::Strip`] and never sees a Report.
async fn usage_relay(person: &str, abilities: &[String], op: &str, body: &[u8]) -> (&'static str, Vec<u8>) {
    use ai_usage_api::*;
    let err = |st: &'static str, m: String| (st, json(&serde_json::json!({ "error": m.chars().take(300).collect::<String>() })));
    if !USAGE_OPS.contains(&op) {
        return err("404 Not Found", "no such op".into());
    }
    let Some(ab) = for_cell(abilities, "ai.usage", "ai.usage:read") else { return err("403 Forbidden", "refused".into()) };
    let mut c = match client::<AiUsage>(person, &ab).await {
        Ok(c) => c,
        Err(e) if e == "refused" => return err("403 Forbidden", "refused".into()),
        Err(e) => return err("503 Service Unavailable", e),
    };
    c.deadline = Duration::from_secs(15);
    let wire = |e: ce_wire::WireError| match e {
        ce_wire::WireError::Refused(_) => err("403 Forbidden", "refused".into()),
        x => err("503 Service Unavailable", x.to_string()),
    };
    let usage = |e: UsageError| err("502 Bad Gateway", format!("{e:?}"));
    match op {
        "StripOp" => {
            let Ok(req) = serde_json::from_slice::<strip::StripReq>(body) else { return err("400 Bad Request", "body".into()) };
            let me = req.device.clone().or_else(this_device);
            match c.call::<Read>(&ReadReq { tenant: String::new(), scope: "all".into() }).await {
                Ok(Ok(r)) => ("200 OK", json(&strip::build(&r, epoch_s(), me.as_deref()))),
                Ok(Err(e)) => usage(e),
                Err(e) => wire(e),
            }
        }
        "CostsOp" => {
            let Ok(req) = serde_json::from_slice::<strip::CostsReq>(body) else { return err("400 Bad Request", "body".into()) };
            let mut rows = Vec::new();
            for id in req.ids.iter().take(120) {
                if id.is_empty() || id.len() > 160 || id.chars().any(|ch| ch.is_control()) {
                    continue;
                }
                match c.call::<Conversation>(&ConvReq { id: id.clone() }).await {
                    Ok(Ok(row)) => rows.push(strip::cost(id, row.as_ref())),
                    Ok(Err(e)) => return usage(e),
                    Err(e) => return wire(e),
                }
            }
            ("200 OK", json(&strip::Costs { rows }))
        }
        _ => {
            let Ok(req) = serde_json::from_slice::<strip::WaitReq>(body) else { return err("400 Bad Request", "body".into()) };
            // the Events stream, held open for at most wait_ms: one item means "read again"
            // `since` is required: None would replay the last 20 events at once
            let Some(since) = req.since.clone().filter(|s| strip::parse_time(s).is_some()) else { return err("400 Bad Request", "since: an RFC 3339 time".into()) };
            let mut sub = match c.subscribe::<Events>(&EventsReq { tenant: String::new(), since: Some(since) }) {
                Ok(s) => s,
                Err(e) => return wire(e),
            };
            let wait = Duration::from_millis(u64::from(req.wait_ms).clamp(1000, 30_000));
            let waited = match tokio::time::timeout(wait, sub.next()).await {
                Ok(Some(Ok(row))) => strip::Waited { changed: true, at: Some(row.at) },
                Ok(Some(Err(e))) => return wire(e),
                _ => strip::Waited { changed: false, at: None },
            };
            ("200 OK", json(&waited))
        }
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
    let route = path.split(['?', '#']).next().unwrap_or("/").to_string();
    // the page's base is /<tab>/_h/<kit hash>/ (P1 of repo:ce docs/SPEED-AND-SEARCH-2026-11-21.md): every kit file and
    // view the page loads has the hash in its address, so it is kept immutable; an older hash still answers, uncached
    let (route, hashed) = unhash(route);
    let kept = if hashed { "private, max-age=31536000, immutable" } else { "no-store" };
    // the one POST: the Feed data relay (JSON in, JSON out; a cross-site form cannot send application/json)
    // and the Conversations relay (ai.session List, Get, Read), on the face of /conversations/ only
    let relay = method == "POST" && (route.starts_with("/api/cell/feed.view/") || (route.starts_with("/api/cell/ai.session/") && f.route == "/conversations/") || (route.starts_with("/api/cell/conv.usage/") && f.route == "/conversations/") || (route.starts_with("/api/cell/term.session/") && f.route == "/conversations/"));
    // the Architecture relays (arch.map, and arch.notes for the notes strip), on the Core and Architecture faces
    let arch_face = ARCH_FACES.contains(&f.route.as_str());
    let arch_relay_req = method == "POST" && route.starts_with("/api/cell/arch.map/") && arch_face;
    let notes_req = method == "POST" && route.starts_with("/api/cell/arch.notes/") && arch_face;
    if method != "GET" && !is_head && !relay && !arch_relay_req && !notes_req {
        return reply(&mut s, "405 Method Not Allowed", "text/plain", b"", false).await;
    }
    if route == "/" || route == "/index.html" || route.starts_with("/o/") {
        // the page names its own tab route and the kit's hash as its base, so /o/<ref> loads the same kit and views and
        // a browser keeps them; for a signed-in viewer it also carries their tabs (what tabs.json would answer first)
        // and names its modules up front, so the kit, the tab's view and the tabs are one round trip, not four
        let person = site_gateway::read_key(&f.person_key).and_then(|k| site_gateway::signed_person(&head, &k, now_s())).unwrap_or_default();
        let mut extra = format!("<head>\n<base href=\"{}_h/{}/\">\n<link rel=\"modulepreload\" href=\"_k/shell.js\">\n<link rel=\"modulepreload\" href=\"_k/ref.js\">", f.route, kit_hash());
        let mut tail = String::new();
        if !person.is_empty() {
            let ab = f.abilities(&person);
            let t = tabs(&f, &ab, &person).await;
            if let Some(v) = t.tabs.iter().find(|x| x.route == t.here).and_then(|x| x.view.clone()).filter(|v| v != "v/feed.js" && v != "v/architecture.js") {
                extra.push_str(&format!("\n<link rel=\"modulepreload\" href=\"{v}\">"));
            }
            // JSON in a script element: "</" written "<\/" so no value can end the element
            tail = format!("<script type=\"application/json\" id=\"ce-tabs\">{}</script>\n", String::from_utf8_lossy(&json(&t)).replace("</", "<\\/"));
        }
        let page = String::from_utf8_lossy(file("index.html").unwrap_or_default()).replacen("<head>", &extra, 1).replacen("</body>", &format!("{tail}</body>"), 1);
        return reply(&mut s, "200 OK", "text/html; charset=utf-8", page.as_bytes(), is_head).await;
    }
    if let Some(k) = route.strip_prefix("/_k/") {
        return match file(&format!("_k/{k}")) {
            Some(b) => reply_cached(&mut s, "200 OK", kind(k), b, is_head, kept).await,
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
    if arch_relay_req || notes_req {
        let op = route.strip_prefix(if notes_req { "/api/cell/arch.notes/" } else { "/api/cell/arch.map/" }).unwrap_or("");
        if !covers(&ab, "arch.map:see") {
            return reply(&mut s, "403 Forbidden", "application/json", b"{\"error\":\"refused\"}", false).await;
        }
        let low = head.to_ascii_lowercase();
        let len: usize = low.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        let json_ct = low.lines().any(|l| l.starts_with("content-type:") && l.contains("application/json"));
        if !json_ct || len > FEED_BODY_MAX {
            return reply(&mut s, "400 Bad Request", "application/json", b"{\"error\":\"want application/json, at most 64 KB\"}", false).await;
        }
        let at = buf.windows(4).position(|x| x == b"\r\n\r\n").map_or(buf.len(), |i| i + 4);
        let mut body = buf[at..].to_vec();
        while body.len() < len {
            let mut chunk = [0u8; 4096];
            match tokio::time::timeout(Duration::from_secs(20), s.read(&mut chunk)).await {
                Ok(Ok(n)) if n > 0 => body.extend_from_slice(&chunk[..n]),
                _ => return,
            }
        }
        body.truncate(len);
        let (st, out) = if notes_req { notes_relay(&person, &ab, op, &body).await } else { arch_relay(&person, &ab, op, &body).await };
        return reply(&mut s, st, "application/json", &out, false).await;
    }
    if relay {
        let live = f.live.read().await.clone();
        let mine = tabs_for(&ab, &live, ui_chain::now_ms());
        let (cell, op) = route.strip_prefix("/api/cell/").and_then(|r| r.split_once('/')).unwrap_or(("", ""));
        let tab = if cell == "ai.session" || cell == "conv.usage" || cell == "term.session" { "/conversations/" } else { "/feed/" };
        if !mine.iter().any(|t| t.route == tab) {
            return reply(&mut s, "403 Forbidden", "application/json", b"{\"error\":\"refused\"}", false).await;
        }
        let low = head.to_ascii_lowercase();
        let len: usize = low.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        let json_ct = low.lines().any(|l| l.starts_with("content-type:") && l.contains("application/json"));
        if !json_ct || len > FEED_BODY_MAX {
            return reply(&mut s, "400 Bad Request", "application/json", b"{\"error\":\"want application/json, at most 64 KB\"}", false).await;
        }
        let at = buf.windows(4).position(|x| x == b"\r\n\r\n").map_or(buf.len(), |i| i + 4);
        let mut body = buf[at..].to_vec();
        while body.len() < len {
            let mut chunk = [0u8; 4096];
            match tokio::time::timeout(Duration::from_secs(20), s.read(&mut chunk)).await {
                Ok(Ok(n)) if n > 0 => body.extend_from_slice(&chunk[..n]),
                _ => return,
            }
        }
        body.truncate(len);
        let (st, out) = match cell {
            "ai.session" => conv_relay(&person, &ab, op, &body).await,
            "conv.usage" => usage_relay(&person, &ab, op, &body).await,
            "term.session" => term_relay(&person, &ab, op, &body).await,
            _ => feed_relay(&person, &ab, op, &body).await,
        };
        return reply(&mut s, st, "application/json", &out, false).await;
    }
    if let Some(sha) = route.strip_prefix("/_c/").and_then(|r| r.strip_suffix(".js")) {
        // the immutable copy of a bundle this node fetched, only for a viewer who has the tab
        let live = f.live.read().await.clone();
        let has = tabs_for(&ab, &live, ui_chain::now_ms()).iter().any(|t| t.route == "/feed/");
        let hit = has.then(|| BUNDLES.get().and_then(|m| m.lock().unwrap().get(sha).cloned())).flatten();
        // or the Architecture module, only for a viewer who holds arch.map:see
        let hit = hit.or_else(|| covers(&ab, "arch.map:see").then(|| ARCH_BUNDLES.get().and_then(|m| m.lock().unwrap().get(sha).cloned())).flatten());
        return match hit {
            Some(b) => reply_cached(&mut s, "200 OK", "text/javascript; charset=utf-8", &b, is_head, "private, max-age=31536000, immutable").await,
            None => reply(&mut s, "404 Not Found", "text/plain", b"no such file", is_head).await,
        };
    }
    if let Some(v) = route.strip_prefix("/v/") {
        let live = f.live.read().await.clone();
        let mine = tabs_for(&ab, &live, ui_chain::now_ms());
        let name = format!("v/{v}");
        if name == "v/feed.js" && mine.iter().any(|t| view_of(&t.route) == Some("v/feed.js")) {
            return match feed_bundle(&person, &ab).await {
                Ok((_, b)) => reply(&mut s, "200 OK", "text/javascript; charset=utf-8", &b, is_head).await,
                Err(e) => {
                    eprintln!("apps-shell: the Feed view for {person}: {e}");
                    reply(&mut s, "503 Service Unavailable", "text/plain", b"not answering", is_head).await
                }
            };
        }
        let arch_open = match f.route.as_str() {
            "/core/" => extra_views(&ab, &f.route).iter().any(|v| v == "architecture"),
            "/architecture/" => covers(&ab, "arch.map:see"),
            _ => false,
        };
        if name == "v/architecture.js" && arch_open {
            return match arch_bundle(&person, &ab).await {
                Ok((_, b)) => reply(&mut s, "200 OK", "text/javascript; charset=utf-8", &b, is_head).await,
                Err(e) => {
                    eprintln!("apps-shell: the Architecture view for {person}: {e}");
                    reply(&mut s, "503 Service Unavailable", "text/plain", b"not answering", is_head).await
                }
            };
        }
        // the same 404 for a view that does not exist and one this viewer may not have
        let open_to = mine.iter().any(|t| view_of(&t.route) == Some(name.as_str())) || (!mine.is_empty() && view_needs(&name).is_some_and(|n| covers(&ab, n)));
        return match open_to.then(|| file(&name)).flatten() {
            // the same bytes for everyone it is open to: kept by the browser under the hashed address
            Some(b) => reply_cached(&mut s, "200 OK", "text/javascript; charset=utf-8", b, is_head, kept).await,
            None => reply(&mut s, "404 Not Found", "text/plain", b"no such file", is_head).await,
        };
    }
    if route == "/api/cell/ai.session/Events" && method == "GET" && f.route == "/conversations/" {
        let live = f.live.read().await.clone();
        if !tabs_for(&ab, &live, ui_chain::now_ms()).iter().any(|t| t.route == "/conversations/") {
            return reply(&mut s, "403 Forbidden", "application/json", b"{\"error\":\"refused\"}", false).await;
        }
        return conv_events(&mut s, &person, &ab, &path).await;
    }
    if route == "/api/search" && query(&path, "stream").as_deref() == Some("1") && {
        let live = f.live.read().await.clone();
        tabs_for(&ab, &live, ui_chain::now_ms()).iter().any(|t| t.route == "/search/")
    } {
        let q = query_text(&path, "q").unwrap_or_default();
        let q = q.trim();
        if q.is_empty() || q.chars().count() > 300 {
            return reply(&mut s, "400 Bad Request", "application/json", b"{\"error\":\"q: 1 to 300 characters\"}", false).await;
        }
        let k = query(&path, "k").and_then(|k| k.parse().ok()).unwrap_or(PER_GROUP).clamp(1, 50);
        return search_stream(&mut s, &person, &ab, q, k).await;
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
    for pair in env_or("APPS_SHELL_HTTP", "127.0.0.1:8881=/approvals/,127.0.0.1:8882=/core/,127.0.0.1:8883=/search/,127.0.0.1:8884=/feed/,127.0.0.1:8885=/home/,127.0.0.1:8886=/conversations/,127.0.0.1:8887=/architecture/".into()).split(',') {
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
    // a proof copy beside the live cell (test people file, test person key, other ports): the faces and the liveness
    // loop only, never a second apps.shell at the door
    if std::env::var("APPS_SHELL_FACES_ONLY").is_ok_and(|v| v == "1") {
        eprintln!("apps-shell: faces only (APPS_SHELL_FACES_ONLY=1), not at the door");
        std::future::pending::<()>().await;
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
    /// apps.shell@1 as foreman-ops-1's door served it before the Home tab (2026-11-18).
    const APPS_SHELL: &str = "0eff16cb36c9fc92";
    /// The copies must hash as the live cells do, or every call is refused as another api.
    #[test]
    fn copied_search_contracts_match_the_live_cells() {
        assert_eq!(<super::hybrid_api::SearchHybrid as ce_wire::Api>::SCHEMA, 0x2e6c7e98274d467f);
        assert_eq!(<super::mesh_api::SearchMesh as ce_wire::Api>::SCHEMA, 0xb78f3a1097281268);
        assert_eq!(format!("{:016x}", <super::ref_resolve_api::RefResolve as ce_wire::Api>::SCHEMA), REF_RESOLVE);
        assert_eq!(format!("{:016x}", <super::ref_pin_api::RefPin as ce_wire::Api>::SCHEMA), REF_PIN);
    }
    /// cell.search, crm.people, crm.promises and crm.calls as foreman-ops-1's door listed them (`ce cell ops <api>
    /// --json`, 2026-11-17).
    /// feed.view major 2 as repo:feed-view crates/feed-view-api has it (commit 32b4c3a): another hash and every call is
    /// refused as another api. Its rights are what `needs` says; the relay passes five ops.
    #[test]
    /// ai.session major 1 as repo:ai-session crates/ai-session-api has it: another hash and every call is refused by
    /// the cell's door, so this pins it; the relay passes the three read ops only.
    fn copied_ai_session_contract_matches_the_cell() {
        use ce_wire::Api;
        assert_eq!(<super::ai_session_api::AiSession as Api>::SCHEMA, 0x711b33ac59622d2b);
        let ops: Vec<&str> = super::ai_session_api::AiSession::OPS.iter().map(|o| o.name).collect();
        let names: Vec<&str> = apps_shell::AI_OPS.iter().map(|(o, _)| *o).collect();
        for op in &names {
            assert!(ops.contains(op), "{op}");
        }
        for never in ["ConfigList", "ConfigGet", "ConfigPut", "ConfigEnable", "ConfigDisable", "ImportRun", "ProfilePut", "ProfileGet", "ProfileList", "Rehome"] {
            assert!(!names.contains(&never), "{never} is never passed from a page");
        }
        assert_eq!(names, ["List", "Get", "Read", "Events", "Open", "Send", "Continue", "Fork", "Interrupt", "Close", "Ask"]);
    }

    /// term.session major 1 as repo:term-session crates/term-session-api has it (its own test pins the same
    /// hash): the terminal relay passes List, Read, Keys, Interrupt and Watch (over Attach) only, never Open, Close,
    /// Freeze, Thaw, Resize or OpenWith.
    #[test]
    fn copied_term_session_contract_matches_the_cell() {
        use ce_wire::Api;
        use apps_shell::term_session_api::TermSession;
        assert_eq!(format!("{:016x}", <TermSession as Api>::SCHEMA), "7c4d50fa566fd6e8");
        let ops: Vec<&str> = TermSession::OPS.iter().map(|o| o.name).collect();
        for (op, _) in apps_shell::TERM_OPS.iter().filter(|(o, _)| *o != "Watch") {
            assert!(ops.contains(op), "{op}");
        }
        let names: Vec<&str> = apps_shell::TERM_OPS.iter().map(|(o, _)| *o).collect();
        assert_eq!(names, ["List", "Read", "Watch", "Keys", "Interrupt"]);
        for (op, need) in apps_shell::TERM_OPS {
            assert_eq!(*need, if matches!(*op, "Keys" | "Interrupt") { "term.session:write" } else { "term.session:read" });
        }
    }

    #[test]
    fn copied_feed_view_contract_matches_the_cell() {
        use ce_wire::Api;
        assert_eq!(<super::feed_view_api::FeedView as Api>::SCHEMA, 0xf226abb16f72c818);
        let ops: Vec<(u32, &str)> = super::feed_view_api::FeedView::OPS.iter().map(|o| (o.id, o.name)).collect();
        assert_eq!(ops, vec![(1, "HealthOp"), (2, "Page"), (3, "Topic"), (4, "Changes"), (5, "Mark"), (6, "Close"), (7, "Ui"), (8, "Compare")]);
        for op in super::FEED_OPS {
            assert!(ops.iter().any(|(_, n)| n == op), "{op}");
        }
        assert!(!super::FEED_OPS.contains(&"Ui") && !super::FEED_OPS.contains(&"Compare"));
    }
    /// arch.map major 1 as repo:arch-map crates/arch-map-api has it (f5dc421, frozen whole): another hash and every call
    /// is refused as another api. The relay passes the internal reads (slice 1), the demo reads and Changes (slice 2);
    /// never Ui (served as /v/architecture.js) nor the curate ops.
    #[test]
    fn copied_arch_map_contract_matches_the_cell() {
        use ce_wire::Api;
        assert_eq!(<super::arch_map_api::ArchMap as Api>::SCHEMA, 0x934e4e8a827ab7ab);
        let ops: Vec<&str> = super::arch_map_api::ArchMap::OPS.iter().map(|o| o.name).collect();
        assert_eq!(ops.len(), 14);
        for op in super::ARCH_OPS {
            assert!(ops.contains(op), "{op}");
        }
        assert_eq!(super::ARCH_OPS, &["GraphFull", "DetailFull", "Sources", "Graph", "Detail", "Story", "Progress", "Changes"]);
        for no in ["Ui", "Import", "PutAlias", "PutStory", "HealthOp"] {
            assert!(!super::ARCH_OPS.contains(&no), "{no}");
        }
    }
    /// arch.notes major 1 as repo:arch-notes 534c49f froze it; the notes relay passes Put, List and Close only, each
    /// needing the viewer's own right (Take and Watch are the lanes').
    #[test]
    fn copied_arch_notes_contract_matches_the_cell() {
        use ce_wire::Api;
        assert_eq!(<super::arch_notes_api::ArchNotes as Api>::SCHEMA, 0xa4dc01ad901bfcda);
        let ops: Vec<&str> = super::arch_notes_api::ArchNotes::OPS.iter().map(|o| o.name).collect();
        for (op, _) in apps_shell::NOTES_OPS {
            assert!(ops.contains(op), "{op}");
        }
        assert_eq!(apps_shell::NOTES_OPS, &[("Put", "arch.notes:write"), ("List", "arch.notes:read"), ("Close", "arch.notes:write")]);
    }
    #[test]
    fn copied_cell_and_crm_contracts_match_the_live_cells() {
        assert_eq!(<super::cell_search_api::CellSearch as ce_wire::Api>::SCHEMA, 0x86ace76bede120b1);
        assert_eq!(<super::crm_people_api::CrmPeople as ce_wire::Api>::SCHEMA, 0x448ba80c84fc9d29);
        assert_eq!(<super::crm_promises_api::CrmPromises as ce_wire::Api>::SCHEMA, 0xb536f482fae0a492);
        assert_eq!(<super::crm_calls_api::CrmCalls as ce_wire::Api>::SCHEMA, 0x75a354c0af4a012f);
    }
    /// This cell's own api is unchanged by the Home tab and the ce-net browser (additive routes only).
    #[test]
    fn copied_state_store_contract_matches_the_live_cell() {
        assert_eq!(<super::state_store_api::StateStore as ce_wire::Api>::SCHEMA, 0x054ed03aa6aa8fc1);
    }
    #[test]
    fn apps_shell_schema_is_unchanged() {
        assert_eq!(format!("{:016x}", <apps_shell_api::AppsShell as ce_wire::Api>::SCHEMA), APPS_SHELL);
    }
}

#[cfg(test)]
mod search_parts {
    use super::*;

    fn hit(path: &str, acl: &str, machine: &str) -> hybrid_api::Hit {
        hybrid_api::Hit { id: path.into(), source: "files".into(), machine: machine.into(), path: path.into(), title: path.rsplit('/').next().unwrap_or("").into(),
            snippet: String::new(), score: 0.5, acl: acl.into(), bm25_rank: 1, vec_rank: 1 }
    }

    #[test]
    fn the_kit_hash_is_in_the_address_and_only_this_builds_is_kept() {
        let h = kit_hash();
        assert_eq!(h.len(), 16);
        assert_eq!(unhash(format!("/_h/{h}/_k/shell.js")), ("/_k/shell.js".to_string(), true));
        assert_eq!(unhash("/_h/0123456789abcdef/_k/shell.js".to_string()), ("/_k/shell.js".to_string(), h == "0123456789abcdef"));
        assert_eq!(unhash("/_k/shell.js".to_string()), ("/_k/shell.js".to_string(), false));
        assert_eq!(unhash("/_h/nothex/_k/x".to_string()).1, false);
        assert_eq!(unhash(format!("/_h/{h}/api/search")).0, "/api/search");
    }

    /// The first part carries the group that is back and says which asks are still out; Best fuses what is back; a
    /// paper leaves Public for its own group; a CRM person no query word names is held back by the floor.
    #[tokio::test]
    async fn a_part_shows_what_is_back_and_waits_for_the_rest() {
        let found = hybrid_api::FindResp { hits: vec![hit("no-such-repo-c89c/docs/GATE-DEPLOY.md", "repo:x", "foreman-ops-1,debian,leif-desk,macbook"), hit("paper.feed/arxiv:1", "public", "foreman-ops-1"), hit("cells.catalog/deploy.review", "public", "foreman-ops-1")],
            took_ms: 12.0, chunks: 3, reranked: false, model: String::new() };
        let crm = hybrid_api::FindResp { hits: vec![{ let mut h = hit("people/kelly-chan", "people", "foreman-ops-1"); h.title = "Person: Kelly Chan".into(); h.score = 0.23; h }], took_ms: 3.0, chunks: 1, reranked: false, model: String::new() };
        let a = Asks { h: Ask::Ok(found), p: Ask::Pending, m: Ask::Pending, cs: Ask::Skip, cr: Ask::Ok(crm), cv: Ask::Skip };
        let v = assemble("gate deploy canary", 10, &a, Instant::now()).await;
        assert_eq!(v["private"].as_array().unwrap().len(), 1);
        assert_eq!(v["private"][0]["machine"], "foreman-ops-1 +3", "one row for four machines");
        assert_eq!(v["papers"].as_array().unwrap().len(), 1);
        assert_eq!(v["public"].as_array().unwrap().len(), 1);
        assert!(v["crm"].as_array().unwrap().is_empty(), "Kelly Chan is not what gate deploy canary asks for");
        assert_eq!(v["best"][0]["path"], "no-such-repo-c89c/docs/GATE-DEPLOY.md");
        let waiting: Vec<&str> = v["notes"].as_array().unwrap().iter().filter(|n| n["pending"] == true).map(|n| n["cell"].as_str().unwrap()).collect();
        assert_eq!(waiting, vec!["search.hybrid", "search.mesh"]);
    }
}

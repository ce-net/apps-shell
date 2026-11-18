//! The one function of `apps.shell`: which tabs a viewer gets (repo:ce/docs/SITES-ON-CELLS.md section 3).
//!
//! A tab exists only because a cell system of repo:ce-net-net `CE_NET` offers one. For a viewer it is kept only when
//! both hold: **allowed**, the viewer's abilities cover the tab's one `needs` right, and **live**, the tab's shell
//! cell answered HealthOp within [`LIVE_MS`]. Nothing else names a tab: a new system with a tab line appears for the
//! viewers it is meant for the moment its shell answers, and leaves when the last replica stops.
//!
//! Who holds what: the people file (`$APPS_SHELL_PEOPLE`, default `~/.config/apps-shell/people.json`, on the node,
//! never in this repo), read on every request so a grant or an end shows on open pages at the next poll:
//! `{"people": {"<person>": {"abilities": ["approve.inbox:read", "approve.inbox:where:requester=<p>"], "until": <unix s>}}}`.
//! With no file, `leif` holds [`ROOT`] and nobody else holds anything.
#![forbid(unsafe_code)]

use apps_shell_api::TabRow;
use ce_net_net::{Tab, CE_NET};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const CELL: &str = "apps.shell";
/// A shell that answered within this long is live.
pub const LIVE_MS: u64 = 8_000;
/// How often the gateway asks every tab's shell.
pub const PROBE_MS: u64 = 2_000;
/// How long one HealthOp may take (connect, then answer). A loaded node (foreman-ops-1 at load 45, 2026-11-16) answers
/// in seconds and is still live: a slow answer keeps the tab; only no answer within LIVE_MS drops it.
pub const HEALTH_MS: u64 = 5_000;

/// The one grant this cell's own key holds: asking every tab shell's health, and having wire.ws-door mint a viewer's
/// grant for the data cells of the tabs it serves (bounded Mint: only `approve.inbox` and `site.cellmap` rights, the
/// row caveats the people file gives that viewer included, minted by the node, never widened here). The data calls
/// are made with that viewer grant, so the data cell's own gate and row caveats see the viewer, never this cell.
/// It holds no data right itself, and never keys or cell.install.
pub const GRANT: &[&str] = &[
    "wire.ws-door:mint",
    "wire.ws-door:mint-for:approve.inbox",
    "wire.ws-door:mint-for:site.cellmap",
    "wire.ws-door:mint-for:search.hybrid",
    "wire.ws-door:mint-for:search.mesh",
    "wire.ws-door:mint-for:ref.resolve",
    "wire.ws-door:mint-for:ref.pin",
    "wire.ws-door:mint-for:cell.search",
    "wire.ws-door:mint-for:crm.people",
    "wire.ws-door:mint-for:crm.promises",
    "wire.ws-door:mint-for:crm.calls",
    "wire.ws-door:mint-for:open.with",
    "wire.ws-door:mint-for:home.shell",
    "wire.ws-door:mint-for:feed.view",
    "approve.inbox:health",
    "site.cellmap:health",
    "foreman.shell:health",
    "cr.shell:health",
    "search.hybrid:health",
    "home.shell:health",
    "state.store:read",
    "feed.view:health",
    "delegate:1",
];

/// The cells whose rights a viewer grant may carry (what [`GRANT`] lets wire.ws-door mint).
pub const DATA_CELLS: &[&str] = &[
    "approve.inbox", "site.cellmap", "search.hybrid", "search.mesh", "ref.resolve", "ref.pin", "cell.search", "crm.people", "crm.promises", "crm.calls",
    "open.with", "home.shell", "feed.view",
];

/// The abilities of the viewer grant to mint for these abilities: only the data cells' (row caveats kept), sorted.
pub fn viewer_grant(abilities: &[String]) -> Vec<String> {
    let mut v: Vec<String> = abilities
        .iter()
        .filter(|a| DATA_CELLS.iter().any(|c| a.starts_with(&format!("{c}:"))) && !a.contains('*'))
        .cloned()
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Leif's abilities when the people file does not name him.
pub const ROOT: &[&str] = &["approve.inbox:read", "site.cellmap:read", "cr.shell:open", "foreman.shell:open", "apps.shell:tabs", "search.hybrid:find", "search.mesh:find", "ref.resolve:read", "ref.pin:read",
    "cell.search:search", "crm.people:read", "crm.promises:read", "crm.calls:read", "open.with:use", "home.shell:open", "home.shell:links"];

/// The Search tab's private reads: search.hybrid opens a private label only for an ability naming it exactly (its
/// lib.rs PRIVATE), and every repo by the exact `read:repos`. A viewer's search carries only the ones they hold.
pub const SEARCH_PRIVATE: &[&str] = &[
    "search.hybrid:read:repos",
    "search.hybrid:read:words",
    "search.hybrid:read:mail",
    "search.hybrid:read:goals",
    "search.hybrid:read:rooms",
    "search.hybrid:read:inbox",
    "search.hybrid:read:calls",
    "search.hybrid:read:people",
];

/// Which group a search.hybrid hit belongs to on the Search tab: `public` for what cells published for everyone (acl
/// `public`) and capabilities any mesh caller may read, else `private` (only a viewer whose exact grant read it got it).
pub fn group_of(acl: &str) -> &'static str {
    if acl == "public" || acl == "caps:mesh" {
        "public"
    } else {
        "private"
    }
}

/// How many hits each group of the Search tab shows (Private, Network, Public each their own, so an owner's many
/// private hits never push Public to 0).
pub const PER_GROUP: u32 = 10;

/// The abilities for the Search tab's public-only ask: `search.hybrid:find` alone, no private read, so search.hybrid
/// answers only what cells published for everyone. None when the viewer holds no private read (their one ask is
/// public-only already) or no find.
pub fn public_only(abilities: &[String]) -> Option<Vec<String>> {
    let find = "search.hybrid:find";
    if !covers(abilities, find) || !abilities.iter().any(|a| a.starts_with("search.hybrid:read:")) {
        return None;
    }
    Some(vec![find.to_string()])
}

/// Keep the first `n` rows whose `key` is not seen yet.
pub fn first_unique<T>(rows: Vec<T>, n: usize, key: impl Fn(&T) -> String) -> Vec<T> {
    let mut seen = std::collections::BTreeSet::new();
    rows.into_iter().filter(|r| seen.insert(key(r))).take(n).collect()
}

fn name_ok(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('.') && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

/// The ce-ref a repo or file hit names: `repo:<repo>/<path>` (repo:ce-ref-id grammar; the commit is left for the viewer
/// to pin to the checkout ref.pin reads), or None for anything else. A repo itself names its README.md. `path` is where
/// the hit lives (`~/dev/<repo>/<file>`, `dev/..` or `<repo>/<file>`); with no usable path a git origin in `key` gives
/// the repo name (its last segment), never a guessed host.
pub fn ref_of(source: &str, kind: &str, path: &str, key: &str) -> Option<String> {
    if !(source == "files" || kind == "repo" || kind == "doc") {
        return None;
    }
    let p = path.trim_start_matches("~/dev/").trim_start_matches("dev/").trim_end_matches('/');
    let (repo, rest) = p.split_once('/').unwrap_or((p, ""));
    let (repo, rest) = if name_ok(repo) {
        (repo.to_string(), rest.to_string())
    } else if kind == "repo" {
        let r = key.trim_end_matches('/').trim_end_matches(".git").rsplit(['/', ':']).next().unwrap_or("").to_string();
        (r, String::new())
    } else {
        return None;
    };
    if !name_ok(&repo) || !(rest.is_empty() || rest.split('/').all(name_ok)) {
        return None;
    }
    Some(format!("repo:{repo}/{}", if rest.is_empty() { "README.md" } else { &rest }))
}

/// Where a search hit opens inside apps.ce-net.com: a repo or file in this tab's own ce-ref viewer (`/search/?ref=`,
/// ref.resolve and ref.pin asked as the viewer), a cell in the Core tab, a goal, room or inbox item in Foreman. Never a
/// GitHub link (Leif: references go through our cell reference system). None: no screen yet.
pub fn open_of(source: &str, kind: &str, path: &str, key: &str) -> Option<String> {
    if kind == "cell" || source == "cells" || path.starts_with("cells.catalog/") || path.starts_with("caps/cell/") {
        return Some("/core/".into());
    }
    if let Some(r) = rec_of(source, path) {
        return Some(format!("/search/?rec={r}"));
    }
    if ["goals", "rooms", "inbox", "calls", "words"].contains(&source) {
        return Some("/foreman/".into());
    }
    // names are [A-Za-z0-9._-] and '/', ':' only (ref_of), so nothing needs escaping in a query value
    ref_of(source, kind, path, key).map(|r| format!("/search/?ref={r}"))
}

/// The record a CRM or call hit of search.hybrid names, for the Search tab's record screen (`/search/?rec=`), read
/// from the cell that owns it as the viewer: `person:<id>` (crm.people Get: profile, open promises, mood),
/// `promise:<id>` (crm.promises Get), `call:<id>` (crm.calls Get), `meet:<recording>` (a call recording's notes and
/// transcript; search.hybrid's own passages, as no cell serves whole transcripts yet). Paths as search.hybrid writes
/// them: `people/<id>` and `calls/<id>` (search-hybrid-collect, from crm.people and crm.calls), `crm/people/<id>`,
/// `crm/calls/<id>`, `crm/promises/<id>` and `meet/<dir>[/<file>]` (crm.rs). A numbered `calls/<n>` row of the old
/// CALLS.md has no record: None.
pub fn rec_of(source: &str, path: &str) -> Option<String> {
    if !(source == "calls" || source == "people") {
        return None;
    }
    let (kind, id) = if let Some(id) = path.strip_prefix("crm/promises/") {
        ("promise", id)
    } else if let Some(id) = path.strip_prefix("crm/people/").or_else(|| path.strip_prefix("people/")) {
        ("person", id)
    } else if let Some(id) = path.strip_prefix("crm/calls/").or_else(|| path.strip_prefix("calls/")) {
        if id.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        ("call", id)
    } else if let Some(id) = path.strip_prefix("meet/") {
        ("meet", id)
    } else {
        return None;
    };
    let r = format!("{kind}:{id}");
    parse_rec(&r).map(|_| r)
}

/// A record screen's ref, checked: `<person|promise|call|meet>:<id>`, the id 1 to 200 of `[A-Za-z0-9._:@-]` (and
/// `/` for meet), never `..`. -> (kind, id).
pub fn parse_rec(r: &str) -> Option<(&'static str, String)> {
    let (k, id) = r.split_once(':')?;
    let kind = ["person", "promise", "call", "meet"].into_iter().find(|x| *x == k)?;
    let ok = |c: char| c.is_ascii_alphanumeric() || "._:@-".contains(c) || (kind == "meet" && c == '/');
    ((1..=200).contains(&id.len()) && id.chars().all(ok) && !id.contains("..") && !id.starts_with('/') && !id.ends_with('/'))
        .then(|| (kind, id.to_string()))
}

/// A viewer link's ref, checked: `repo:<repo>/<path>` with an optional `@<commit>` (4 to 40 lowercase hex), names as
/// in [`ref_of`]. -> (repo, path, commit).
pub fn parse_view_ref(r: &str) -> Option<(String, String, Option<String>)> {
    let rest = r.strip_prefix("repo:")?;
    let (rp, commit) = match rest.rsplit_once('@') {
        Some((rp, c)) if (4..=40).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) => (rp, Some(c.to_string())),
        Some(_) => return None,
        None => (rest, None),
    };
    let (repo, path) = rp.split_once('/')?;
    (name_ok(repo) && !path.is_empty() && path.split('/').all(name_ok)).then(|| (repo.to_string(), path.to_string(), commit))
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Person {
    #[serde(default)]
    pub abilities: Vec<String>,
    /// unix seconds after which the entry gives nothing (0 = no end)
    #[serde(default)]
    pub until: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct People {
    #[serde(default)]
    pub people: BTreeMap<String, Person>,
}

/// The abilities `person` holds now. An unreadable file gives nobody anything (fail closed), not even Leif.
pub fn abilities(file: Option<&str>, person: &str, now_s: u64) -> Vec<String> {
    let Some(text) = file else {
        return if person == "leif" { ROOT.iter().map(|s| s.to_string()).collect() } else { vec![] };
    };
    let Ok(p) = serde_json::from_str::<People>(text) else { return vec![] };
    match p.people.get(person) {
        Some(e) if e.until == 0 || e.until > now_s => e.abilities.clone(),
        _ => vec![],
    }
}

/// ce-cap's rule: equal, `*`, or `<domain>:*`; a caveat ability never covers a right.
pub fn covers(abilities: &[String], right: &str) -> bool {
    site_gateway::covers(abilities, right)
}

/// The abilities to present to cell `api` for an op needing `need` on the viewer's behalf: the viewer's own
/// abilities in that cell's domain (their row caveats included), or None when they do not cover `need`.
pub fn for_cell(abilities: &[String], api: &str, need: &str) -> Option<Vec<String>> {
    if !covers(abilities, need) {
        return None;
    }
    let head = format!("{api}:");
    let mut v: Vec<String> = abilities.iter().filter(|a| a.starts_with(&head) && !a.ends_with(":*")).cloned().collect();
    if !v.iter().any(|a| a == need) {
        v.push(need.to_string());
    }
    Some(v)
}

/// The tabs for a viewer: allowed by their abilities and live (`live`: shell api -> when it last answered, ms).
pub fn tabs_for(abilities: &[String], live: &BTreeMap<String, u64>, now_ms: u64) -> Vec<TabRow> {
    CE_NET
        .tabs()
        .into_iter()
        .filter(|(_, t)| covers(abilities, t.needs))
        .filter_map(|(sys, t)| {
            let at = *live.get(t.shell.name)?;
            (now_ms.saturating_sub(at) <= LIVE_MS).then(|| TabRow { system: sys.into(), title: t.title.into(), route: t.route.into(), order: t.order, live_ms: at })
        })
        .collect()
}

/// Every tab any system offers (what the liveness loop asks).
pub fn all_tabs() -> Vec<(&'static str, Tab)> {
    CE_NET.tabs()
}

/// The view module this gateway serves for a tab route (only the tabs whose views live in this site).
pub fn view_of(route: &str) -> Option<&'static str> {
    match route {
        "/home/" => Some("v/home.js"),
        "/approvals/" => Some("v/approvals.js"),
        "/core/" => Some("v/core.js"),
        "/search/" => Some("v/search.js"),
        // not a file of this site: the module the feed cell ships (`GET /v/feed.js` asks its `Ui`)
        "/feed/" => Some("v/feed.js"),
        _ => None,
    }
}

/// A repo's real origin from a search.mesh key (the answering node read it from that checkout's git remote): an
/// `https://` origin as it is, `git@<host>:<path>` as `https://<host>/<path>`; `.git` and credentials dropped. Never
/// built from a name: a key that is not an origin gives None.
pub fn origin_of(key: &str) -> Option<String> {
    let k = key.trim().trim_end_matches('/').trim_end_matches(".git");
    let (host, path) = if let Some(r) = k.strip_prefix("https://") {
        let r = r.rsplit_once('@').map(|(_, h)| h).unwrap_or(r);
        r.split_once('/')?
    } else if let Some(r) = k.strip_prefix("git@") {
        r.split_once(':')?
    } else {
        return None;
    };
    let host_ok = !host.is_empty() && host.contains('.') && host.chars().all(|c| c.is_ascii_alphanumeric() || "-.".contains(c));
    (host_ok && !path.is_empty() && path.split('/').all(name_ok)).then(|| format!("https://{host}/{path}"))
}

/// The view modules that are not a tab's own: the openers open.with may name (ref.view, site.cellmap, apps.shell
/// views) and the Home slots. Each is served only to a viewer whose chain holds its right (the same 404 otherwise), and
/// every data route it calls checks the viewer's right in that cell again.
pub fn view_needs(name: &str) -> Option<&'static str> {
    Some(match name {
        "v/ref-text.js" | "v/ref-raw.js" | "v/ref-record.js" | "v/cell.js" | "v/port.js" | "v/outside.js" | "v/mount.js" => "open.with:use",
        "v/calls-today.js" => "crm.calls:read",
        "v/promises-due.js" => "crm.promises:read",
        "v/pending.js" => "approve.inbox:read",
        "v/cells-summary.js" => "site.cellmap:read",
        "v/links.js" => "home.shell:links",
        _ => return None,
    })
}

/// A Home links row (state.store `home/links`, one row a ref): `{"ref": "assembly:huebot", "title": "Huebot assembly",
/// "note": "3D, step by step"}`. Rows whose `ref` is not a ce-ref are dropped, never shown as text that could be a URL.
#[derive(Debug, Clone, Default, Deserialize, serde::Serialize, PartialEq)]
pub struct Link {
    #[serde(rename = "ref")]
    pub r: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Links {
    #[serde(default)]
    links: Vec<Link>,
}

pub fn links(file: Option<&str>) -> Vec<Link> {
    let Some(t) = file else { return vec![] };
    let Ok(l) = serde_json::from_str::<Links>(t) else { return vec![] };
    l.links.into_iter().filter(|x| open_with_api::CeRef::parse(&x.r).is_ok()).take(64).collect()
}

/// The Home links from state.store rows (`home/links`, each value one [`Link`] as JSON); a row that does not parse or
/// whose `ref` is not a ce-ref is dropped.
pub fn links_rows<'a>(values: impl Iterator<Item = &'a str>) -> Vec<Link> {
    values.filter_map(|v| serde_json::from_str::<Link>(v).ok()).filter(|x| open_with_api::CeRef::parse(&x.r).is_ok()).take(64).collect()
}

/// The calls of one day for the Home slot: `[from, to)` in unix seconds, at most two days wide (the viewer's own day,
/// computed by its browser in its own zone), oldest first.
pub fn day_range(from: Option<i64>, to: Option<i64>, now_s: i64) -> (i64, i64) {
    let from = from.unwrap_or(now_s - now_s.rem_euclid(86_400));
    let to = to.filter(|t| *t > from && *t - from <= 2 * 86_400).unwrap_or(from + 86_400);
    (from, to)
}

/// Promises for the Home slot: open, with a due date, due within `ahead_ms` of now or overdue; soonest first.
/// -> (index, overdue) of the rows to show.
pub fn due_soon(dues: &[(u64, bool)], now_ms: u64, ahead_ms: u64) -> Vec<(usize, bool)> {
    let mut v: Vec<(usize, u64, bool)> = dues.iter().enumerate().filter(|(_, (d, open))| *open && *d > 0 && *d <= now_ms + ahead_ms).map(|(i, (d, _))| (i, *d, *d < now_ms)).collect();
    v.sort_by_key(|(_, d, _)| *d);
    v.into_iter().map(|(i, _, o)| (i, o)).collect()
}

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

pub mod ai_usage_api;
pub mod decision_card_api;
pub mod term_session_api;
pub mod strip;

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
    "wire.ws-door:mint-for:ai.session",
    // the usage strip of the Conversations tab: ai.usage Read, Conversation and Events, as the viewer
    "wire.ws-door:mint-for:ai.usage",
    // the Architecture view under Core (/core/?view=architecture): arch.map's Ui and its internal reads, each as the viewer
    "wire.ws-door:mint-for:arch.map",
    // the notes strip of the Architecture view: arch.notes Put, List, Close, as the viewer with their author caveat
    "wire.ws-door:mint-for:arch.notes",
    // the Approvals tab's cards: decision.card BoardOp as the viewer (their owner caveat kept)
    "wire.ws-door:mint-for:decision.card",
    // a tmux row of the Conversations tab, live: term.session List, Read, Attach, Keys, Interrupt, as the viewer
    "wire.ws-door:mint-for:term.session",
    "approve.inbox:health",
    "site.cellmap:health",
    "foreman.shell:health",
    "cr.shell:health",
    "search.hybrid:health",
    "home.shell:health",
    "state.store:read",
    "feed.view:health",
    "ai.session:health",
    "arch.map:health",
    "arch.notes:health",
    "decision.card:health",
    // the Now and Business monitor tabs are live while their shells answer
    "person.now:health",
    "business.view:health",
    // the Code tab (ce-net-net DEVENV) is live while dev.ws answers
    "dev.ws:health",
    "delegate:1",
];

/// The cells whose rights a viewer grant may carry (what [`GRANT`] lets wire.ws-door mint).
pub const DATA_CELLS: &[&str] = &[
    "approve.inbox", "site.cellmap", "search.hybrid", "search.mesh", "ref.resolve", "ref.pin", "cell.search", "crm.people", "crm.promises", "crm.calls",
    "open.with", "home.shell", "feed.view", "ai.session", "ai.usage", "arch.map", "arch.notes", "decision.card", "term.session",
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

/// The ops the Conversations tab's relay passes to ai.session (repo:ce docs/AI-AS-CELLS-2026-11-18.md sections 12.1, 13.8),
/// each with the right the viewer must hold, and nothing else: no `Config*`, `Profile*`, `ImportRun` or `Rehome` from
/// a page. `Events` is the one stream, served as server-sent events (GET); every other op is a POST.
pub const AI_OPS: &[(&str, &str)] = &[
    ("List", "ai.session:list"),
    ("Get", "ai.session:read"),
    ("Read", "ai.session:read"),
    ("Events", "ai.session:read"),
    ("Open", "ai.session:open"),
    ("Send", "ai.session:send"),
    ("Continue", "ai.session:send"),
    ("Fork", "ai.session:open"),
    ("Interrupt", "ai.session:send"),
    ("Close", "ai.session:close"),
    ("Ask", "ai.session:ask"),
    // section 13 (repo:ce docs/AI-AS-CELLS-2026-11-18.md 13.7-13.8): the Nodes block and "Load older"
    ("Coverage", "ai.session:coverage"),
    ("ListBefore", "ai.session:list"),
];

/// The right `op` of the ai.session relay needs, or None for an op it does not pass.
pub fn ai_need(op: &str) -> Option<&'static str> {
    AI_OPS.iter().find(|(o, _)| *o == op).map(|(_, n)| *n)
}

/// What the relay changes or refuses in a page's request before ai.session sees it (JSON, as the page sent it):
/// `Open` and `Ask` run the Bare harness only with no tool grants (a Code harness is a shell on the node: never from a
/// page); `Send` never waits (the answer comes over Events), with text and ce-ref attachments only.
pub fn ai_check(op: &str, body: &mut serde_json::Value) -> Result<(), String> {
    // Coverage takes no request: null (or {}) only
    if op == "Coverage" {
        if body.is_null() || body.as_object().is_some_and(|o| o.is_empty()) {
            *body = serde_json::Value::Null;
            return Ok(());
        }
        return Err("Coverage takes no request".into());
    }
    let o = body.as_object_mut().ok_or("a JSON object")?;
    // a profile row carries its own harness (a Code one, with shell, is a shell on the node): never from a page
    if matches!(op, "Open" | "Ask") && o.get("profile").is_some_and(|p| !p.is_null()) {
        return Err("a page opens no profile: a profile carries its own harness".into());
    }
    match op {
        "Open" => {
            if o.get("harness").is_some_and(|h| h != "Bare") {
                return Err("a page opens the Bare harness only".into());
            }
            if o.get("tools").and_then(|t| t.as_array()).is_some_and(|t| !t.is_empty()) {
                return Err("no tool grants from a page".into());
            }
        }
        "Ask" => {
            if o.get("text").and_then(|t| t.as_str()).is_none_or(|t| t.trim().is_empty() || t.len() > 32 * 1024) {
                return Err("text: 1 to 32768 bytes".into());
            }
        }
        "Send" => {
            o.insert("wait".into(), serde_json::Value::Bool(false));
            for p in o.get("parts").and_then(|p| p.as_array()).into_iter().flatten() {
                let ok = p.get("Text").is_some_and(|t| t.as_str().is_some_and(|t| t.len() <= 32 * 1024)) || p.get("Attachment").is_some_and(|t| t.is_string());
                if !ok {
                    return Err("parts: Text (at most 32768 bytes) or Attachment only".into());
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// The ops the Conversations tab's terminal relay passes to term.session, each with the right the viewer must hold;
/// nothing else (no Open, Close, Freeze or Resize from a page). `Watch` is the long poll over the cell's Attach stream.
pub const TERM_OPS: &[(&str, &str)] = &[
    ("List", "term.session:read"),
    ("Read", "term.session:read"),
    ("Watch", "term.session:read"),
    ("Keys", "term.session:write"),
    ("Interrupt", "term.session:write"),
];

/// The right `op` of the terminal relay needs, or None for an op it does not pass.
pub fn term_need(op: &str) -> Option<&'static str> {
    TERM_OPS.iter().find(|(o, _)| *o == op).map(|(_, n)| *n)
}

/// The viewer's term.session abilities as the grant carries them. A viewer grant never holds an ability with a `*`
/// (it would be dropped, and a dropped session limit widens to every session), so `where:session=<p>*` becomes
/// `where:session-prefix=<p>`; any other term.session ability holding a `*` refuses the call (None).
pub fn term_scope(ab: Vec<String>) -> Option<Vec<String>> {
    ab.into_iter()
        .map(|a| match a.strip_prefix("term.session:where:session=").and_then(|p| p.strip_suffix('*')) {
            Some(p) if !p.contains('*') => Some(format!("term.session:where:session-prefix={p}")),
            _ if a.contains('*') => None,
            _ => Some(a),
        })
        .collect()
}

/// One request of the terminal relay (JSON in): the node that holds the pane (its `term.session#<node>`), the pane
/// (`%3` or `session:window.pane`, as a tmux row's native id carries it), and for Keys the text and/or key names.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TermReq {
    pub node: String,
    #[serde(default)]
    pub pane: String,
    /// Keys: typed as-is (literal)
    #[serde(default)]
    pub text: String,
    /// Keys: tmux key names after the text (Enter, Up, BSpace, Tab, Escape, C-d ...)
    #[serde(default)]
    pub keys: Vec<String>,
    /// Keys: Enter after everything
    #[serde(default)]
    pub enter: bool,
    /// Watch: the hash of the screen the page shows; the answer waits until the screen differs (or 20 s)
    #[serde(default)]
    pub since: String,
}

impl TermReq {
    /// Checked before any call: a node name, a pane target, at most 4096 characters of text and 16 plain key names.
    pub fn check(&self, op: &str) -> Result<(), String> {
        let node_ok = !self.node.is_empty() && self.node.len() <= 64 && self.node.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
        if !node_ok {
            return Err("node: a node name".into());
        }
        let pane_ok = self.pane.len() < 128 && !self.pane.starts_with('-') && self.pane.chars().all(|c| c.is_alphanumeric() || "%@$:._-".contains(c));
        if !pane_ok || (op != "List" && self.pane.is_empty()) {
            return Err("pane: a pane id or session:window.pane".into());
        }
        if self.text.chars().count() > 4096 || self.keys.len() > 16 {
            return Err("at most 4096 characters and 16 keys".into());
        }
        if let Some(k) = self.keys.iter().find(|k| k.is_empty() || k.len() > 12 || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
            return Err(format!("key {k:?}: a tmux key name"));
        }
        Ok(())
    }
}

/// Leif's abilities when the people file does not name him.
pub const ROOT: &[&str] = &["approve.inbox:read", "site.cellmap:read", "cr.shell:open", "foreman.shell:open", "apps.shell:tabs", "search.hybrid:find", "search.mesh:find", "ref.resolve:read", "ref.pin:read",
    "cell.search:search", "crm.people:read", "crm.promises:read", "crm.calls:read", "open.with:use", "home.shell:open", "home.shell:links", "ai.session:list", "ai.session:read", "ai.usage:read", "decision.card:read"];

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

/// What one search row is, across the Search tab's groups (section 8 fix 1 and 3 of repo:ce
/// docs/SPEED-AND-SEARCH-2026-11-21.md): a file by its repo and path wherever it was found (`~/dev/<repo>/..`,
/// search.hybrid's `<repo>/..`, docs.index's `content/docs.index/<machine>/<repo>/..`), a cell by its api, a record by
/// where it opens; anything else by group, title and path.
pub fn thing_of(row: &serde_json::Value) -> String {
    let s = |k: &str| row[k].as_str().unwrap_or("").to_string();
    let (kind, source, path, title, open) = (s("kind"), s("source"), s("path"), s("title"), s("open"));
    if kind == "cell" || source == "cells" || source == "cell.search" {
        let api = title.split([' ', '\u{b7}']).next().unwrap_or("").trim().to_string();
        let api = path.strip_prefix("cells/").map(str::to_string).filter(|_| source == "cells").unwrap_or(api);
        return format!("cell:{api}");
    }
    if open.starts_with("/search/?rec=") {
        return open;
    }
    let p = path.trim_start_matches("~/dev/").trim_start_matches("dev/");
    let p = match p.strip_prefix("content/docs.index/").and_then(|r| r.split_once('/')) {
        Some((_, r)) => r,
        None => p,
    };
    if source == "files" || source == "content" || kind == "doc" || kind == "repo" || source.contains("files") || source.contains("docs") {
        return format!("doc:{}", p.trim_end_matches('/'));
    }
    format!("{source}:{p}:{title}")
}

/// The words of a query that can name something: three letters or more, not a common word.
pub fn name_words(q: &str) -> Vec<String> {
    const COMMON: &[&str] = &["the", "and", "for", "with", "from", "what", "who", "how", "where", "when", "which", "that", "this", "cell", "cells", "doc", "docs", "file", "files", "one", "all", "about", "show", "find", "get", "new"];
    q.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase).filter(|w| w.chars().count() >= 3 && !COMMON.contains(&w.as_str())).collect()
}

/// True when a query word is one of the words of `name` (a person's name, a cell's api or title): the CRM and Cells
/// groups show a row only then, or when its score clears their floor (section 8 fix 2).
pub fn covers_name(words: &[String], name: &str) -> bool {
    let toks: Vec<String> = name.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase).filter(|t| !t.is_empty()).collect();
    words.iter().any(|w| toks.iter().any(|t| t == w || (w.len() >= 5 && t.len() >= 5 && (t.starts_with(w.as_str()) || w.starts_with(t.as_str())))))
}

/// One ranked list over the groups ("Best", section 8 fix 1): reciprocal rank fusion (k = 10) by each row's place in
/// its own group, never by its score (the groups score on five scales), each group weighted; one row per thing
/// (`thing_of`), the row of the group that ranked it best, with `group` naming every group it came from.
pub fn best_of(groups: &[(&str, f32, &[serde_json::Value])], n: usize) -> Vec<serde_json::Value> {
    let mut at: BTreeMap<String, (f32, usize, usize, Vec<String>)> = BTreeMap::new(); // thing -> (score, best place, order, groups)
    let mut rows: Vec<serde_json::Value> = vec![];
    for (g, w, list) in groups {
        for (i, r) in list.iter().enumerate() {
            let t = thing_of(r);
            let add = w / (10.0 + i as f32 + 1.0);
            match at.get_mut(&t) {
                Some(e) => {
                    e.0 += add;
                    if !e.3.iter().any(|x| x == g) {
                        e.3.push(g.to_string());
                    }
                    if i < e.1 {
                        e.1 = i;
                        rows[e.2] = r.clone();
                    }
                }
                None => {
                    at.insert(t, (add, i, rows.len(), vec![g.to_string()]));
                    rows.push(r.clone());
                }
            }
        }
    }
    let mut v: Vec<(f32, usize, Vec<String>)> = at.into_values().map(|(s, _, o, g)| (s, o, g)).collect();
    // ties by first seen (group order, then place), so the same answers give the same list
    v.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    v.into_iter()
        .take(n)
        .map(|(s, o, g)| {
            let mut r = rows[o].clone();
            r["group"] = serde_json::json!(g);
            r["fused"] = serde_json::json!((s * 1000.0).round() / 1000.0);
            r
        })
        .collect()
}

/// A Public row that is a paper (paper.feed): papers have their own group, out of Public and Best (section 8 fix 7).
pub fn is_paper(row: &serde_json::Value) -> bool {
    row["path"].as_str().is_some_and(|p| p.starts_with("paper.feed/"))
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

/// The views a tab hosts besides its own (`<route>?view=<name>`), each opened only to a viewer who holds its right, its
/// module the cell's own (`GET /v/<name>.js` asks that cell's `Ui` as the viewer). The Architecture view lives under Core
/// until it has its own route (repo:ce/docs/ARCHITECTURE-ATLAS-APP-2026-11-18.md 7.3, slices 1 to 4).
pub const EXTRA_VIEWS: &[(&str, &str, &str)] = &[("/core/", "architecture", "arch.map:see")];

/// The faces that serve the Architecture view, its module and its relays: Core (`?view=architecture`) and its own
/// route (face 8887). Both stay until Core's `?view=` is retired, 14 days after the gate route is live.
pub const ARCH_FACES: &[&str] = &["/core/", "/architecture/"];

/// Tabs whose route the apps.ce-net.com gate does not serve yet, and where their view opens meanwhile. The
/// Architecture tab (ce-net-net ARCHITECTURE) is listed once arch.map answers; its link opens the view under Core
/// until the gate's quiet window adds `/architecture/` (repo:ce/docs/ARCHITECTURE-ATLAS-APP-2026-11-18.md 7.4).
pub const NOT_ROUTED_YET: &[(&str, &str)] = &[("/architecture/", "/core/?view=architecture")];

pub fn href_of(route: &str) -> Option<&'static str> {
    NOT_ROUTED_YET.iter().find(|(r, _)| *r == route).map(|(_, h)| *h)
}

/// The notes cell the Architecture view may write to for this viewer: only with `arch.notes:write`.
pub fn notes_cell(abilities: &[String]) -> Option<&'static str> {
    covers(abilities, "arch.notes:write").then_some("arch.notes")
}

/// The ops of the notes relay and the right each needs of the viewer.
pub const NOTES_OPS: &[(&str, &str)] = &[("Put", "arch.notes:write"), ("List", "arch.notes:read"), ("Close", "arch.notes:write")];

/// The extra views of `route` this viewer may open: their names.
pub fn extra_views(abilities: &[String], route: &str) -> Vec<String> {
    EXTRA_VIEWS.iter().filter(|(r, _, need)| *r == route && covers(abilities, need)).map(|(_, n, _)| n.to_string()).collect()
}

/// The view module this gateway serves for a tab route (only the tabs whose views live in this site).
pub fn view_of(route: &str) -> Option<&'static str> {
    match route {
        "/home/" => Some("v/home.js"),
        "/approvals/" => Some("v/approvals.js"),
        "/core/" => Some("v/core.js"),
        // the module is arch.map's own (its Ui as the viewer), never a file of this site
        "/architecture/" => Some("v/architecture.js"),
        "/search/" => Some("v/search.js"),
        // not a file of this site: the module the feed cell ships (`GET /v/feed.js` asks its `Ui`)
        "/feed/" => Some("v/feed.js"),
        "/conversations/" => Some("v/conversations.js"),
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
        // the two UI cells of the Conversations tab (v/conversations.js lays them out)
        "v/conv-list.js" => "ai.session:list",
        "v/conv-thread.js" => "ai.session:read",
        // the usage strip above them (conv.usage: ai.usage Read, Conversation, Events)
        "v/conv-usage.js" => "ai.usage:read",
        // a tmux row opened live (term.session on the pane's node)
        "v/conv-term.js" => "term.session:read",
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

#[cfg(test)]
mod best_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn one_file_found_by_two_groups_is_one_best_row_first() {
        let private = vec![
            json!({"title": "gate.py", "path": "ce-fleet/door/gate.py", "source": "files"}),
            json!({"title": "Gate deploy", "path": "content/docs.index/foreman-ops-1/ce-fleet/docs/GATE-DEPLOY.md", "source": "content"}),
        ];
        let network = vec![json!({"title": "GATE-DEPLOY.md", "path": "~/dev/ce-fleet/docs/GATE-DEPLOY.md", "source": "docs", "kind": "doc"})];
        let cells = vec![json!({"title": "paypal.payment", "path": "paypal-payment", "source": "cell.search", "kind": "cell"})];
        let b = best_of(&[("private", 1.0, &private), ("network", 1.0, &network), ("cells", 0.8, &cells)], 10);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0]["title"], "GATE-DEPLOY.md", "in two groups beats first in one: {b:?}");
        assert_eq!(b[0]["group"], json!(["private", "network"]));
        assert_eq!(b[1]["title"], "gate.py");
        // the same answers, the same list
        assert_eq!(b, best_of(&[("private", 1.0, &private), ("network", 1.0, &network), ("cells", 0.8, &cells)], 10));
    }

    #[test]
    fn crm_and_cells_floors_rest_on_a_named_word() {
        let w = name_words("gate deploy canary");
        assert!(!covers_name(&w, "Person: Kelly Chan, (AI services, HK)"));
        assert!(!covers_name(&w, "paypal.payment"));
        assert!(covers_name(&name_words("Vivi"), "Person: Vivi Wang"));
        assert!(covers_name(&name_words("send a mail"), "mail.send"));
        assert!(!covers_name(&name_words("send a mail"), "nest.outer"));
        assert!(covers_name(&name_words("screenshot of a page"), "page.shot · screenshot of a page") );
        assert_eq!(thing_of(&json!({"title": "search.hybrid cell", "path": "cells/search.hybrid", "source": "cells"})), "cell:search.hybrid");
        assert_eq!(thing_of(&json!({"title": "search.hybrid · BM25", "source": "cell.search", "kind": "cell"})), "cell:search.hybrid");
        assert!(is_paper(&json!({"path": "paper.feed/arxiv:2610.22667"})));
    }
}

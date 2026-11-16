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
    "approve.inbox:health",
    "site.cellmap:health",
    "foreman.shell:health",
    "cr.shell:health",
    "delegate:1",
];

/// The cells whose rights a viewer grant may carry (what [`GRANT`] lets wire.ws-door mint).
pub const DATA_CELLS: &[&str] = &["approve.inbox", "site.cellmap"];

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
pub const ROOT: &[&str] = &["approve.inbox:read", "site.cellmap:read", "cr.shell:open", "foreman.shell:open", "apps.shell:tabs"];

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
        "/approvals/" => Some("v/approvals.js"),
        "/core/" => Some("v/core.js"),
        _ => None,
    }
}

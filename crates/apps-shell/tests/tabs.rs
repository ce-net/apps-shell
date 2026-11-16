//! Emergent tabs: the same network gives three viewers three different bars, and a stopped shell takes its tab away.
use apps_shell::*;
use std::collections::BTreeMap;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn all_live(now: u64) -> BTreeMap<String, u64> {
    all_tabs().iter().map(|(_, t)| (t.shell.name.to_string(), now)).collect()
}

#[test]
fn tabs_follow_the_chain() {
    let now = 1_000_000;
    let live = all_live(now);
    let routes = |a: &[&str]| tabs_for(&s(a), &live, now).into_iter().map(|t| t.route).collect::<Vec<_>>();
    assert!(routes(&[]).is_empty(), "no rights: no tabs");
    assert_eq!(routes(&["approve.inbox:read", "approve.inbox:where:requester=t-approver"]), ["/approvals/"]);
    assert_eq!(routes(&["approve.inbox:read", "cr.shell:open"]), ["/approvals/", "/control-room/"]);
    assert_eq!(routes(ROOT), ["/foreman/", "/approvals/", "/control-room/", "/core/"]);
    assert!(routes(&["approve.inbox:where:requester=x"]).is_empty(), "a caveat alone opens nothing");
}

#[test]
fn a_tab_whose_shell_stopped_answering_is_gone() {
    let now = 1_000_000;
    let mut live = all_live(now);
    live.insert("approve.inbox".into(), now - LIVE_MS - 1);
    let r: Vec<String> = tabs_for(&s(ROOT), &live, now).into_iter().map(|t| t.route).collect();
    assert_eq!(r, ["/foreman/", "/control-room/", "/core/"]);
    live.remove("site.cellmap");
    assert!(!tabs_for(&s(ROOT), &live, now).iter().any(|t| t.route == "/core/"), "never answered: no tab");
}

#[test]
fn the_people_file_decides_and_fails_closed() {
    let f = r#"{"people":{"t-a":{"abilities":["approve.inbox:read"],"until":2000},"leif":{"abilities":["site.cellmap:read"]}}}"#;
    assert_eq!(abilities(Some(f), "t-a", 1999), s(&["approve.inbox:read"]));
    assert!(abilities(Some(f), "t-a", 2000).is_empty(), "ended");
    assert_eq!(abilities(Some(f), "leif", 0), s(&["site.cellmap:read"]), "the file is the word, even for leif");
    assert!(abilities(Some("{broken"), "leif", 0).is_empty());
    assert_eq!(abilities(None, "leif", 0).len(), ROOT.len());
    assert!(abilities(None, "t-a", 0).is_empty());
}

#[test]
fn a_call_carries_only_the_viewers_own_rights_in_that_cell() {
    let a = s(&["approve.inbox:read", "approve.inbox:where:requester=p", "site.cellmap:read"]);
    assert_eq!(for_cell(&a, "approve.inbox", "approve.inbox:read").unwrap(), s(&["approve.inbox:read", "approve.inbox:where:requester=p"]));
    assert_eq!(for_cell(&a, "site.cellmap", "site.cellmap:read").unwrap(), s(&["site.cellmap:read"]));
    assert!(for_cell(&s(&["approve.inbox:where:requester=p"]), "approve.inbox", "approve.inbox:read").is_none());
    assert_eq!(for_cell(&s(&["*"]), "site.cellmap", "site.cellmap:read").unwrap(), s(&["site.cellmap:read"]));
}

#[test]
fn every_tab_shell_is_granted_its_health_check() {
    for (_, t) in all_tabs() {
        let need = format!("{}:health", t.shell.name);
        assert!(covers(&s(GRANT), &need), "GRANT lacks {need}");
    }
}

#[test]
fn a_viewer_grant_carries_only_data_cell_rights_with_their_caveats() {
    let a = s(&["cr.shell:open", "approve.inbox:where:requester=p", "approve.inbox:read", "site.cellmap:*", "foreman.shell:open"]);
    assert_eq!(viewer_grant(&a), s(&["approve.inbox:read", "approve.inbox:where:requester=p"]));
    for c in DATA_CELLS {
        assert!(GRANT.contains(&format!("wire.ws-door:mint-for:{c}").as_str()));
    }
    assert!(!GRANT.iter().any(|g| g.contains('*') || g.ends_with(":read")), "the shell holds no data right itself");
}

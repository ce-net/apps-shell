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
    assert_eq!(routes(ROOT), ["/foreman/", "/search/", "/approvals/", "/control-room/", "/core/"]);
    assert_eq!(routes(&["search.hybrid:find"]), ["/search/"], "the Search tab needs only search.hybrid:find");
    assert!(routes(&["approve.inbox:where:requester=x"]).is_empty(), "a caveat alone opens nothing");
}

#[test]
fn a_tab_whose_shell_stopped_answering_is_gone() {
    let now = 1_000_000;
    let mut live = all_live(now);
    live.insert("approve.inbox".into(), now - LIVE_MS - 1);
    let r: Vec<String> = tabs_for(&s(ROOT), &live, now).into_iter().map(|t| t.route).collect();
    assert_eq!(r, ["/foreman/", "/search/", "/control-room/", "/core/"]);
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

#[test]
fn the_search_tab_is_live_only_while_search_hybrid_answers() {
    let now = 1_000_000;
    let mut live = all_live(now);
    let has = |l: &BTreeMap<String, u64>| tabs_for(&s(&["search.hybrid:find"]), l, now).iter().any(|t| t.route == "/search/");
    assert!(has(&live));
    live.insert("search.hybrid".into(), now - LIVE_MS - 1);
    assert!(!has(&live), "index not answering: no Search tab");
    assert_eq!(view_of("/search/"), Some("v/search.js"));
}

#[test]
fn private_reads_ride_only_for_a_viewer_who_holds_them() {
    // a viewer with only find: the minted grant carries find, nothing private and no network, so only `public`
    let stranger = s(&["search.hybrid:find"]);
    assert_eq!(viewer_grant(&stranger), s(&["search.hybrid:find"]));
    assert!(for_cell(&stranger, "search.mesh", "search.mesh:find").is_none(), "no search.mesh:find: the network is not asked");
    assert!(!viewer_grant(&stranger).iter().any(|a| SEARCH_PRIVATE.contains(&a.as_str())));
    // the owner: exactly his named reads, never a wildcard
    let mut owner = s(&["search.hybrid:find", "search.mesh:find"]);
    owner.extend(SEARCH_PRIVATE.iter().map(|a| a.to_string()));
    owner.push("search.hybrid:read:repo:*".into());
    let g = viewer_grant(&owner);
    assert!(SEARCH_PRIVATE.iter().all(|a| g.contains(&a.to_string())));
    assert!(!g.iter().any(|a| a.contains('*')));
}

#[test]
fn hits_group_and_open_where_they_live() {
    assert_eq!(group_of("public"), "public");
    assert_eq!(group_of("caps:mesh"), "public");
    for acl in ["words", "mail", "people", "repo:feed-search", "owner:abc", "grant:x:y", ""] {
        assert_eq!(group_of(acl), "private", "{acl}");
    }
    // repos and files open in the Search tab's own ce-ref viewer, never on GitHub
    assert_eq!(open_of("files", "", "fire-spread/README.md", "").as_deref(), Some("/search/?ref=repo:fire-spread/README.md"));
    assert_eq!(open_of("", "repo", "~/dev/ce-search", "https://github.com/Leif-Rydenfalk/ce-search.git").as_deref(), Some("/search/?ref=repo:ce-search/README.md"));
    assert_eq!(open_of("", "doc", "~/dev/ce-ref/docs/ADOPTION.md", "").as_deref(), Some("/search/?ref=repo:ce-ref/docs/ADOPTION.md"));
    assert_eq!(open_of("", "repo", "", "https://github.com/ce-net/live-refs.git").as_deref(), Some("/search/?ref=repo:live-refs/README.md"));
    assert_eq!(open_of("", "repo", "", "git@github.com:ce-net/ce-ref-id.git").as_deref(), Some("/search/?ref=repo:ce-ref-id/README.md"));
    for (s, k, p, key) in [("files", "", "a/b", ""), ("", "repo", "~/dev/x", "https://github.com/a/x"), ("", "repo", "", "https://github.com/a/x.git"), ("", "doc", "x/y.md", "")] {
        assert!(!open_of(s, k, p, key).unwrap_or_default().contains("github"), "{p} {key}");
    }
    assert_eq!(open_of("", "cell", "x", "search.hybrid").as_deref(), Some("/core/"));
    assert_eq!(open_of("goals", "", "goals/x", "").as_deref(), Some("/foreman/"));
    assert_eq!(open_of("mail", "", "mail/123", ""), None);
    assert_eq!(open_of("files", "", "../etc/passwd", ""), None);
}

#[test]
fn a_viewer_ref_is_checked_before_any_cell_is_asked() {
    assert_eq!(parse_view_ref("repo:apps-shell/docs/SEARCH.md"), Some(("apps-shell".into(), "docs/SEARCH.md".into(), None)));
    assert_eq!(parse_view_ref("repo:apps-shell/README.md@242f63a"), Some(("apps-shell".into(), "README.md".into(), Some("242f63a".into()))));
    for bad in ["repo:../x/y", "repo:x/../etc/passwd", "repo:x/.git/config", "repo:x", "repo:x/", "repo:x/y@HEAD", "repo:x/y@-rf", "ce-ref://repo/ab/c", "repo:x/y z", "repo:x//y"] {
        assert_eq!(parse_view_ref(bad), None, "{bad}");
    }
    // every link the hits make parses back
    for (s, k, p, key) in [("files", "", "fire-spread/README.md", ""), ("", "repo", "~/dev/ce-search", ""), ("", "doc", "dev/ce-ref/docs/A.md", "")] {
        let o = open_of(s, k, p, key).unwrap();
        assert!(parse_view_ref(o.strip_prefix("/search/?ref=").unwrap()).is_some(), "{o}");
    }
}

#[test]
fn each_search_group_keeps_its_own_quota() {
    assert_eq!(PER_GROUP, 10);
    let owner: Vec<String> = ["search.hybrid:find", "search.mesh:find", "search.hybrid:read:mail"].iter().map(|s| s.to_string()).collect();
    assert_eq!(public_only(&owner), Some(vec!["search.hybrid:find".to_string()]));
    let stranger = vec!["search.hybrid:find".to_string()];
    assert_eq!(public_only(&stranger), None, "a stranger's one ask is public only already");
    assert_eq!(public_only(&["search.hybrid:read:mail".to_string()]), None, "no find, no ask");
    let rows: Vec<(u32, &str)> = (0..30).map(|i| (i, if i % 3 == 0 { "dup" } else { "x" })).collect();
    let kept = first_unique(rows, 10, |r| if r.1 == "dup" { "dup".into() } else { r.0.to_string() });
    assert_eq!(kept.len(), 10);
    assert_eq!(kept.iter().filter(|r| r.1 == "dup").count(), 1);
}

#[test]
fn the_ref_viewer_cells_are_minted_for_and_only_read() {
    assert!(GRANT.contains(&"wire.ws-door:mint-for:ref.resolve") && GRANT.contains(&"wire.ws-door:mint-for:ref.pin"));
    let g = viewer_grant(&["ref.resolve:read".into(), "ref.pin:read".into(), "ref.pin:write".into(), "ref.*".into()]);
    assert!(g.contains(&"ref.pin:read".to_string()) && !g.iter().any(|a| a.contains('*')));
    assert!(ROOT.contains(&"ref.pin:read") && !ROOT.iter().any(|a| a.ends_with(":write")));
}

#[test]
fn a_fallback_origin_is_only_a_real_one() {
    assert_eq!(origin_of("https://github.com/ce-net/live-refs.git").as_deref(), Some("https://github.com/ce-net/live-refs"));
    assert_eq!(origin_of("git@github.com:ce-net/ce-ref-id.git").as_deref(), Some("https://github.com/ce-net/ce-ref-id"));
    assert_eq!(origin_of("https://user:tok@git.example.org/a/b").as_deref(), Some("https://git.example.org/a/b"));
    for k in ["ce-search", "search.hybrid", "~/dev/x", "http://github.com/a/b", "https://github.com/a/../b", "file:///x", ""] {
        assert_eq!(origin_of(k), None, "{k}");
    }
}

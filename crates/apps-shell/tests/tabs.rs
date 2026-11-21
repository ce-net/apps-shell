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
    assert_eq!(routes(ROOT), ["/home/", "/foreman/", "/conversations/", "/search/", "/approvals/", "/control-room/", "/core/"]);
    assert_eq!(routes(&["search.hybrid:find"]), ["/search/"], "the Search tab needs only search.hybrid:find");
    assert!(routes(&["approve.inbox:where:requester=x"]).is_empty(), "a caveat alone opens nothing");
}

#[test]
fn a_tab_whose_shell_stopped_answering_is_gone() {
    let now = 1_000_000;
    let mut live = all_live(now);
    live.insert("approve.inbox".into(), now - LIVE_MS - 1);
    let r: Vec<String> = tabs_for(&s(ROOT), &live, now).into_iter().map(|t| t.route).collect();
    assert_eq!(r, ["/home/", "/foreman/", "/conversations/", "/search/", "/control-room/", "/core/"]);
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
    // state.store:read is the door right to ask the store at all; which collection it may read is the separate
    // collection cap (home/links only, `state-home-links.hex`), so it is the one `:read` the shell holds
    assert!(!GRANT.iter().any(|g| g.contains('*') || (g.ends_with(":read") && *g != "state.store:read")), "the shell holds no data right itself");
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

#[test]
fn crm_and_call_hits_open_their_record_screen() {
    assert_eq!(open_of("calls", "", "crm/promises/deal.promise:ce-net:hooha-deco-visit", "").as_deref(), Some("/search/?rec=promise:deal.promise:ce-net:hooha-deco-visit"));
    assert_eq!(open_of("people", "", "people/deco-hooha", "").as_deref(), Some("/search/?rec=person:deco-hooha"));
    assert_eq!(open_of("people", "", "crm/people/deco-hooha", "").as_deref(), Some("/search/?rec=person:deco-hooha"));
    assert_eq!(open_of("calls", "", "calls/sched-1234-abc", "").as_deref(), Some("/search/?rec=call:sched-1234-abc"));
    assert_eq!(open_of("calls", "", "meet/20261117-063039-venoculus", "").as_deref(), Some("/search/?rec=meet:20261117-063039-venoculus"));
    assert_eq!(open_of("calls", "", "meet/calls/2026-11-04-acme.md", "").as_deref(), Some("/search/?rec=meet:calls/2026-11-04-acme.md"));
    // the old CALLS.md rows are numbered, not records: they still open Foreman
    assert_eq!(open_of("calls", "", "calls/7", "").as_deref(), Some("/foreman/"));
    // only the private CRM labels name records; a file called people/x is a file
    assert_eq!(rec_of("files", "people/x"), None);
    for bad in ["meet:../etc", "meet:/abs", "person:", "person:a b", "promise:x?y=1", "other:x", "person:a/b"] {
        assert!(parse_rec(bad).is_none(), "{bad}");
    }
    assert_eq!(parse_rec("meet:a/b.md"), Some(("meet", "a/b.md".to_string())));
}

#[test]
fn cell_search_and_the_crm_cells_are_minted_for_and_only_read() {
    for c in ["cell.search", "crm.people", "crm.promises", "crm.calls"] {
        assert!(DATA_CELLS.contains(&c) && GRANT.contains(&format!("wire.ws-door:mint-for:{c}").as_str()), "{c}");
    }
    for a in ["cell.search:search", "crm.people:read", "crm.promises:read", "crm.calls:read"] {
        assert!(ROOT.contains(&a), "{a}");
    }
    assert!(!ROOT.iter().any(|a| a.ends_with(":write") || a.ends_with(":reindex") || a.ends_with(":extract")));
    // a viewer grant never carries a wildcard into the CRM cells
    assert_eq!(viewer_grant(&s(&["crm.people:*", "crm.people:read"])), s(&["crm.people:read"]));
}

#[test]
fn the_feed_tab_needs_feed_view_read_and_its_view_is_the_cells_own() {
    let now = 1_000_000;
    let live = all_live(now);
    let routes = |a: &[&str]| tabs_for(&s(a), &live, now).into_iter().map(|t| t.route).collect::<Vec<_>>();
    assert_eq!(routes(&["feed.view:read"]), ["/feed/"]);
    assert_eq!(routes(&["feed.view:read", "feed.view:mark", "feed.view:close"]), ["/feed/"]);
    assert!(routes(&["feed.view:mark", "feed.view:close", "feed.view:health"]).is_empty(), "only :read opens the tab");
    assert!(routes(&["feed.view:where:person=x"]).is_empty(), "a caveat alone opens nothing");
    assert!(!routes(ROOT).contains(&"/feed/".to_string()), "shadow only: nothing Leif sees switches");
    assert_eq!(view_of("/feed/"), Some("v/feed.js"));
    // the module is not a file of this site: a viewer without the tab has nothing to ask for
    let site = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/v/feed.js");
    assert!(!site.exists(), "the Feed module is the feed cell's, served from its Ui, not from site/");
}

#[test]
fn the_shells_own_grant_gains_only_the_feed_mint_and_health() {
    let feed: Vec<&&str> = GRANT.iter().filter(|g| g.contains("feed")).collect();
    assert_eq!(feed, [&"wire.ws-door:mint-for:feed.view", &"feed.view:health"]);
    // a viewer grant carries the viewer's own feed rights and nothing of other cells
    let g = viewer_grant(&s(&["feed.view:read", "feed.view:mark", "foreman.shell:open", "feed.view:*"]));
    assert_eq!(g, s(&["feed.view:mark", "feed.view:read"]));
}

/// Everything this site sends to anyone (page, kit, shell script, views) must not name the feed cell or hold its
/// bundle: a stranger's browser learns nothing of it (the module itself comes only through the cell's `Ui`).
#[test]
fn no_file_of_the_site_names_the_feed_cell() {
    fn walk(d: &std::path::Path, hits: &mut Vec<String>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, hits);
            } else if let Ok(t) = std::fs::read_to_string(&p) {
                if t.contains("feed.view") || t.contains("api/cell/") && !p.ends_with("shell.js") {
                    hits.push(p.display().to_string());
                }
            }
        }
    }
    let mut hits = vec![];
    walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site"), &mut hits);
    assert!(hits.is_empty(), "{hits:?}");
}

/// The Conversations tab (AI-AS-CELLS slice 2): emerges with ai.session:list while ai.session answers; its views are
/// this site's (conv.list and conv.thread, laid out by v/conversations.js) and its data relay is read only.
#[test]
fn the_conversations_tab_needs_ai_session_list_and_goes_when_ai_session_stops() {
    let now = 1_000_000;
    let mut live = all_live(now);
    let routes = |a: &[&str], live: &BTreeMap<String, u64>| tabs_for(&s(a), live, now).into_iter().map(|t| t.route).collect::<Vec<_>>();
    assert_eq!(routes(&["ai.session:list"], &live), ["/conversations/"]);
    assert_eq!(routes(&["ai.session:list", "ai.session:read"], &live), ["/conversations/"]);
    assert!(routes(&["ai.session:read", "ai.session:health", "ai.session:open"], &live).is_empty(), "only :list opens the tab");
    assert!(routes(ROOT, &live).contains(&"/conversations/".to_string()));
    live.insert("ai.session".into(), now - LIVE_MS - 1);
    assert!(routes(&["ai.session:list"], &live).is_empty(), "ai.session silent: no tab");
    assert_eq!(view_of("/conversations/"), Some("v/conversations.js"));
    assert_eq!((view_needs("v/conv-list.js"), view_needs("v/conv-thread.js")), (Some("ai.session:list"), Some("ai.session:read")));
    // the shell may mint the viewer's own ai.session rights and ask ai.session's health, nothing more of it
    let mine: Vec<&&str> = GRANT.iter().filter(|g| g.contains("ai.session")).collect();
    assert_eq!(mine, [&"wire.ws-door:mint-for:ai.session", &"ai.session:health"]);
    assert_eq!(viewer_grant(&s(&["ai.session:list", "ai.session:read", "ai.session:*", "feed.view:read"])), s(&["ai.session:list", "ai.session:read", "feed.view:read"]));
    assert_eq!(for_cell(&s(&["ai.session:list", "feed.view:read"]), "ai.session", "ai.session:read"), None, "Read needs ai.session:read");
    for f in ["conversations.js", "conv-list.js", "conv-thread.js"] {
        let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/v").join(f)).unwrap();
        for bad in ["fetch(", "localStorage", "document.cookie", "window.open", "location.href", "innerHTML", "http://", "https://"] {
            assert!(!src.contains(bad), "{f} holds {bad}");
        }
    }
}

/// The Architecture view under Core: open only to a viewer holding `arch.map:see`; its tab opens it there until the gate
/// routes /architecture/, and
/// the shell's own grant gains exactly the arch.map mint and its health.
#[test]
fn the_architecture_view_opens_under_core_with_arch_map_see() {
    assert_eq!(extra_views(&s(&["arch.map:see"]), "/core/"), ["architecture"]);
    assert_eq!(extra_views(&s(&["arch.map:see", "arch.map:internal"]), "/core/"), ["architecture"]);
    assert!(extra_views(&s(&["arch.map:internal"]), "/core/").is_empty(), "internal alone does not open the view");
    assert!(extra_views(&s(&["site.cellmap:read"]), "/core/").is_empty());
    assert!(extra_views(&s(&["arch.map:see"]), "/search/").is_empty(), "only under Core");
    let arch: Vec<&&str> = GRANT.iter().filter(|g| g.contains(":arch.") || g.starts_with("arch.")).collect();
    assert_eq!(arch, [&"wire.ws-door:mint-for:arch.map", &"wire.ws-door:mint-for:arch.notes", &"arch.map:health", &"arch.notes:health"]);
    let g = viewer_grant(&s(&["arch.map:see", "arch.map:internal", "arch.map:*", "site.cellmap:read"]));
    assert_eq!(g, s(&["arch.map:internal", "arch.map:see", "site.cellmap:read"]));
    // its tab (ce-net-net ARCHITECTURE) is the only one a viewer holding only the view's rights gets, and it opens the
    // view under Core while the gate has no /architecture/ route
    let now = 1_000_000;
    let t = tabs_for(&s(&["arch.map:see", "arch.map:internal"]), &all_live(now), now);
    assert_eq!(t.iter().map(|t| t.route.as_str()).collect::<Vec<_>>(), ["/architecture/"]);
    assert_eq!(href_of("/architecture/"), Some("/core/?view=architecture"));
    assert_eq!(view_of("/architecture/"), Some("v/architecture.js"));
    assert!(href_of("/core/").is_none());
    let site = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/v/architecture.js");
    assert!(!site.exists(), "the Architecture module is arch.map's, served from its Ui, not from site/");
}

#[test]
fn no_file_of_the_site_names_the_arch_map_cell() {
    fn walk(d: &std::path::Path, hits: &mut Vec<String>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, hits);
            } else if let Ok(t) = std::fs::read_to_string(&p) {
                if t.contains("arch.map") {
                    hits.push(p.display().to_string());
                }
            }
        }
    }
    let mut hits = vec![];
    walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site"), &mut hits);
    assert!(hits.is_empty(), "{hits:?}");
}

/// The notes strip: mounted (tabs.json `notes`) only for a viewer holding arch.notes:write; the relay's ops and rights.
#[test]
fn the_notes_strip_is_for_writers_only() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<String>>();
    assert_eq!(notes_cell(&s(&["arch.map:see", "arch.notes:write", "arch.notes:read"])), Some("arch.notes"));
    assert_eq!(notes_cell(&s(&["arch.map:see", "arch.notes:read"])), None);
    assert_eq!(notes_cell(&s(&["arch.map:see"])), None);
    assert!(for_cell(&s(&["arch.map:see"]), "arch.notes", "arch.notes:write").is_none(), "a see-only viewer is refused before any cell is asked");
    let g = viewer_grant(&s(&["arch.notes:write", "arch.notes:read", "arch.map:see"]));
    assert_eq!(g, s(&["arch.map:see", "arch.notes:read", "arch.notes:write"]));
    assert!(ARCH_FACES.contains(&"/core/") && ARCH_FACES.contains(&"/architecture/"));
}

/// A tmux row opens live: conv.term is served only to a viewer holding `term.session:read`; the relay passes five ops
/// (typing and Interrupt need `term.session:write`); a viewer's session limits ride in their grant to the cell; the
/// request is checked before any call; the module holds no fetch, storage, cookie, URL or innerHTML.
#[test]
fn a_tmux_row_opens_live_through_term_session_as_the_viewer() {
    assert_eq!(view_needs("v/conv-term.js"), Some("term.session:read"));
    assert_eq!((term_need("Watch"), term_need("Keys"), term_need("Interrupt")), (Some("term.session:read"), Some("term.session:write"), Some("term.session:write")));
    for op in ["Open", "OpenWith", "Close", "Freeze", "Thaw", "Resize", "Expect", "Attach", "Node"] {
        assert_eq!(term_need(op), None, "{op} is not passed");
    }
    assert!(GRANT.contains(&"wire.ws-door:mint-for:term.session") && DATA_CELLS.contains(&"term.session"));
    assert!(!GRANT.iter().any(|g| g.starts_with("term.session:")), "the shell holds no term.session right of its own");
    // read only: Keys is refused before a grant is minted; the session limit is carried, a wildcard never
    let viewer = s(&["ai.session:list", "term.session:read", "term.session:where:session=proof-*"]);
    assert_eq!(for_cell(&viewer, "term.session", "term.session:write"), None);
    assert_eq!(for_cell(&viewer, "term.session", "term.session:read"), Some(s(&["term.session:read", "term.session:where:session=proof-*"])));
    assert_eq!(viewer_grant(&s(&["term.session:read", "term.session:*"])), s(&["term.session:read"]), "a wildcard is never minted");
    let ok = TermReq { node: "foreman-ops-1".into(), pane: "lane-a:0.0".into(), text: "ls -la".into(), keys: s(&["Enter", "C-c"]), ..Default::default() };
    assert!(ok.check("Keys").is_ok());
    for bad in [
        TermReq { node: "".into(), pane: "%1".into(), ..Default::default() },
        TermReq { node: "a b".into(), pane: "%1".into(), ..Default::default() },
        TermReq { node: "n".into(), pane: "-t".into(), ..Default::default() },
        TermReq { node: "n".into(), pane: "".into(), ..Default::default() },
        TermReq { node: "n".into(), pane: "%1".into(), keys: s(&["Enter; rm"]), ..Default::default() },
        TermReq { node: "n".into(), pane: "%1".into(), text: "x".repeat(4097), ..Default::default() },
    ] {
        assert!(bad.check("Keys").is_err(), "{bad:?}");
    }
    assert!(TermReq { node: "n".into(), ..Default::default() }.check("List").is_ok(), "List needs no pane");
    assert!(serde_json::from_str::<TermReq>(r#"{"node":"n","pane":"%1","argv":["sh"]}"#).is_err(), "unknown fields are refused");
    let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/v/conv-term.js")).unwrap();
    for bad in ["fetch(", "localStorage", "document.cookie", "window.open", "location.href", "innerHTML", "http://", "https://"] {
        assert!(!src.contains(bad), "conv-term.js holds {bad}");
    }
}

/// A session limit written `name*` reaches term.session as a prefix limit, never dropped (a dropped limit is every
/// session); any other `*` in a term.session ability refuses.
#[test]
fn a_starred_session_limit_stays_a_limit() {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(term_scope(v(&["term.session:read", "term.session:where:session=lane-*"])), Some(v(&["term.session:read", "term.session:where:session-prefix=lane-"])));
    assert_eq!(term_scope(v(&["term.session:read", "term.session:where:session=lane-a"])), Some(v(&["term.session:read", "term.session:where:session=lane-a"])));
    assert_eq!(term_scope(v(&["term.session:read", "term.session:where:session=*"])), Some(v(&["term.session:read", "term.session:where:session-prefix="])));
    assert_eq!(term_scope(v(&["term.session:read", "term.session:where:node=a*"])), None);
    assert_eq!(term_scope(v(&["term.session:read", "term.session:where:session=a*b*"])), None);
}

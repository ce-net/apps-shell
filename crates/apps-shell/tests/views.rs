//! The ce-net browser's rules over the views this site carries (repo:ce/docs/CE-REF-OPENERS-AND-HOME-2026-11-18.md
//! section 2.4): a view holds ce-refs and calls host.open, never a URL. A view module may not contain `href="http`,
//! `window.open`, `location.href` or `fetch(`, and no `target=_blank` link is left in any of them.
use apps_shell::*;
use std::path::Path;

fn views() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/v");
    let mut v: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "js"))
        .map(|e| (e.file_name().to_string_lossy().to_string(), std::fs::read_to_string(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}

#[test]
fn ui_lint_no_url_in_any_view() {
    let v = views();
    assert!(v.len() >= 16, "{} views", v.len());
    for (name, text) in &v {
        for bad in ["href=\"http", "href='http", "window.open", "location.href", "fetch("] {
            assert!(!text.contains(bad), "site/v/{name} contains {bad}");
        }
        let blank = text.matches("_blank").count();
        assert_eq!(blank, 0, "site/v/{name}: {blank} target=_blank");
    }
}

#[test]
fn every_view_has_the_v2_surface() {
    for (name, text) in views() {
        assert!(text.contains("export function mount(root, host)"), "site/v/{name}: mount(root, host)");
        assert!(text.contains("export const view = {"), "site/v/{name}: view descriptor");
    }
}

#[test]
fn every_view_the_shell_names_exists_and_is_gated() {
    let shell = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/_k/shell.js")).unwrap();
    let names: Vec<String> = views().into_iter().map(|(n, _)| format!("v/{n}")).collect();
    for n in shell.split("'v/").skip(1).map(|s| format!("v/{}", s.split('\'').next().unwrap())) {
        assert!(names.contains(&n), "shell.js names {n}, which site/v lacks");
        assert!(view_needs(&n).is_some() || ["v/home.js", "v/search.js", "v/core.js", "v/approvals.js"].contains(&n.as_str()), "{n} is served to nobody");
    }
}

#[test]
fn home_is_the_first_tab_and_needs_home_shell_open() {
    let t = all_tabs();
    assert_eq!(t[0].1.route, "/home/");
    assert_eq!(t[0].1.needs, "home.shell:open");
    assert_eq!(view_of("/home/"), Some("v/home.js"));
    assert!(GRANT.contains(&"home.shell:health") && GRANT.contains(&"wire.ws-door:mint-for:open.with") && GRANT.contains(&"wire.ws-door:mint-for:home.shell"));
}

#[test]
fn links_rows_must_be_ce_refs() {
    let f = r#"{"links":[{"ref":"assembly:huebot","title":"Huebot assembly"},{"ref":"https://example.com"},{"ref":"web:https://example.com","title":"x"},{"ref":"repo:ce/docs/X.md"}]}"#;
    let l = links(Some(f));
    assert_eq!(l.iter().map(|x| x.r.as_str()).collect::<Vec<_>>(), ["assembly:huebot", "web:https://example.com", "repo:ce/docs/X.md"]);
    assert!(links(Some("{bad")).is_empty() && links(None).is_empty());
    let rows = [r#"{"ref":"map:twinton","title":"Twinton"}"#, r#"{"ref":"http://x"}"#, "not json", r#"{"ref":"person:t-c-daniel-arm"}"#];
    assert_eq!(links_rows(rows.into_iter()).iter().map(|x| x.r.as_str()).collect::<Vec<_>>(), ["map:twinton", "person:t-c-daniel-arm"]);
}

#[test]
fn day_and_due_windows() {
    assert_eq!(day_range(None, None, 86_400 * 10 + 5), (86_400 * 10, 86_400 * 11));
    assert_eq!(day_range(Some(100), Some(100 + 3 * 86_400), 0), (100, 100 + 86_400), "at most two days");
    let now = 1_000_000_000;
    let d = due_soon(&[(0, true), (now - 1, true), (now + 5, true), (now + 10_000_000, true), (now + 2, false)], now, 1_000_000);
    assert_eq!(d, [(1, true), (2, false)]);
}

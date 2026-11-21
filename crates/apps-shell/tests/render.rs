//! Golden render: the five states (normal, 80, 90, capped, not offered) and the stale badge, painted by
//! site/v/conv-usage.js from the typed Strips of tests/strip.rs. Needs `node` (on foreman-ops-1; CI sets
//! CE_REQUIRE_NODE=1). `UPDATE_GOLDEN=1 cargo test` rewrites tests/golden/render-*.html after tests/strip.rs did.
use std::path::PathBuf;
use std::process::Command;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

const CASES: [&str; 6] = ["normal", "warn80", "bad90", "capped", "not-offered", "stale"];

fn painted(write: bool) -> Option<serde_json::Value> {
    let mut c = Command::new("node");
    c.arg(dir().join("render_golden.mjs"));
    if write {
        c.arg("write");
    }
    match c.output() {
        Ok(o) => {
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
            Some(serde_json::from_slice(&o.stdout).unwrap_or(serde_json::Value::Null))
        }
        Err(_) => {
            assert!(std::env::var("CE_REQUIRE_NODE").is_err(), "node is required here");
            eprintln!("node not found: render golden skipped");
            None
        }
    }
}

#[test]
fn the_strip_paints_the_golden_markup() {
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        painted(true);
    }
    let Some(out) = painted(false) else { return };
    for name in CASES {
        let want = std::fs::read_to_string(dir().join(format!("golden/render-{name}.html"))).unwrap_or_else(|_| panic!("missing render-{name}.html: UPDATE_GOLDEN=1 cargo test"));
        let got = out[name].as_str().unwrap();
        assert_eq!(got, want, "render {name} differs (UPDATE_GOLDEN=1 rewrites it)");
        assert!(!got.contains('@'), "{name}: no email in the markup");
        assert!(!got.contains("style="), "{name}: no inline style (the CSP allows none)");
    }
}

#[test]
fn each_state_is_in_the_markup_it_should_be() {
    let Some(out) = painted(false) else { return };
    let g = |n: &str| out[n].as_str().unwrap().to_string();
    assert!(g("normal").contains("data-state=\"off\"") && g("normal").contains("no 5-hour limit on this plan") && g("normal").contains("16%"));
    assert!(g("warn80").contains("data-gauge=\"7d\"") && g("warn80").contains("data-state=\"warn\""));
    assert!(g("bad90").contains("data-state=\"bad\"") && g("bad90").contains("92%"));
    assert!(g("capped").contains("data-lock=\"capped\"") && g("capped").contains("capped, resets Thu 09:00 UTC"));
    assert!(g("not-offered").contains("data-gauge=\"5h\"") && g("not-offered").contains("value=\"0\""));
    assert!(g("stale").contains("data-stale-badge=\"true\"") && g("stale").contains("stale, last sample 30 min ago"));
    assert!(!g("normal").contains("30 min ago"));
    assert!(g("normal").contains("data-lock=\"lapsed\"") && g("normal").contains("payment past due"), "ambience: lock on Lapsed");
}

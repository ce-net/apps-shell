//! The usage strip (conv.usage): golden Strips for the five states (normal, 80, 90, capped, not offered) and the stale
//! badge, built from the real ai.usage Report of 2026-11-21 (tests/fixtures/report-live-20261121.json, from
//! `ce cell call ai.usage Read` on foreman-ops-1) with one window changed per case. `UPDATE_GOLDEN=1 cargo test`
//! rewrites tests/golden/*.json; the explicit assertions below keep a rewritten golden from changing what a state means.
use apps_shell::ai_usage_api::Report;
use apps_shell::strip::{build, cost, parse_time, Strip};
use std::path::PathBuf;

fn dir(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join(p)
}

fn live() -> Report {
    serde_json::from_slice(&std::fs::read(dir("fixtures/report-live-20261121.json")).unwrap()).unwrap()
}

const NOW: &str = "2026-11-21T06:30:00Z";

fn strip_of(r: &Report) -> Strip {
    build(r, parse_time(NOW).unwrap(), Some("foreman-ops-1"))
}

fn golden(name: &str, s: &Strip) {
    let got = serde_json::to_string_pretty(s).unwrap() + "\n";
    let p = dir(&format!("golden/{name}.json"));
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(&p, &got).unwrap();
    }
    let want = std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("missing golden {name}; run UPDATE_GOLDEN=1 cargo test"));
    assert_eq!(got, want, "golden {name} differs (UPDATE_GOLDEN=1 rewrites it)");
    assert!(!got.contains('@'), "no email or at-sign reaches the page");
}

fn acct2(s: &Strip) -> &apps_shell::strip::AccountStrip {
    s.accounts.iter().find(|a| a.label == "acct2").unwrap()
}

fn seven(r: &mut Report, f: impl Fn(&mut apps_shell::ai_usage_api::WindowRow)) {
    let a = r.accounts.iter_mut().find(|a| a.label == "acct2").unwrap();
    f(a.windows.iter_mut().find(|w| w.kind == "seven_day").unwrap());
}

#[test]
fn normal_is_the_live_account_with_no_five_hour_limit() {
    let s = strip_of(&live());
    let a = acct2(&s);
    assert_eq!(a.state, "ok");
    let (five, seven) = (a.five.as_ref().unwrap(), a.seven.as_ref().unwrap());
    assert_eq!((five.state.as_str(), five.note.as_str(), five.value.as_str()), ("off", "no 5-hour limit on this plan", ""));
    assert_eq!((seven.state.as_str(), seven.value.as_str(), seven.note.as_str(), seven.bar), ("normal", "16%", "resets Thu 09:00 UTC", 16));
    assert_eq!(a.chips.len(), 1);
    assert_eq!((a.chips[0].name.as_str(), a.chips[0].value.as_str()), ("Fable", "7%"));
    assert!(a.badge.is_none() && a.lock.is_none(), "fresh sample: no stale badge, no lock");
    let d = s.device.as_ref().unwrap();
    assert_eq!((d.name.as_str(), d.class.as_str(), d.online, d.logins.as_slice()), ("foreman-ops-1", "server", true, &["acct2".to_string()][..]));
    assert_eq!(s.keys[0].key_ref, "opencode-livekit");
    assert_eq!(s.keys[0].month, "$2.78");
    golden("normal", &s);
}

#[test]
fn eighty_is_amber() {
    let mut r = live();
    seven(&mut r, |w| w.pct = Some(83.0));
    let s = strip_of(&r);
    assert_eq!((acct2(&s).seven.as_ref().unwrap().state.as_str(), acct2(&s).state.as_str()), ("warn", "warn"));
    golden("warn80", &s);
}

#[test]
fn ninety_is_red() {
    let mut r = live();
    seven(&mut r, |w| w.pct = Some(92.0));
    let s = strip_of(&r);
    assert_eq!((acct2(&s).seven.as_ref().unwrap().state.as_str(), acct2(&s).state.as_str()), ("bad", "bad"));
    golden("bad90", &s);
}

#[test]
fn a_full_window_is_capped_with_a_lock_and_the_reset() {
    let mut r = live();
    seven(&mut r, |w| w.pct = Some(100.0));
    let s = strip_of(&r);
    let a = acct2(&s);
    assert_eq!(a.state, "capped");
    assert_eq!(a.lock.as_deref(), Some("capped, resets Thu 09:00 UTC"));
    assert_eq!(a.seven.as_ref().unwrap().note, "capped, resets Thu 09:00 UTC");
    golden("capped", &s);
    // ai.account's own Capped state locks too, whatever the bars say
    let mut r = live();
    r.accounts.iter_mut().find(|a| a.label == "acct2").unwrap().state = "capped".into();
    assert_eq!(acct2(&strip_of(&r)).state, "capped");
}

#[test]
fn not_offered_is_grey_and_not_a_number() {
    let s = strip_of(&live());
    let five = acct2(&s).five.as_ref().unwrap();
    assert!(five.state == "off" && five.pct.is_none() && five.bar == 0 && !five.stale);
    golden("not-offered", &s);
}

#[test]
fn an_old_sample_gets_the_stale_badge_and_a_fresh_one_does_not() {
    let mut r = live();
    for w in &mut r.accounts.iter_mut().find(|a| a.label == "acct2").unwrap().windows {
        w.sampled_at = Some("2026-11-21T06:00:00+00:00".into()); // 30 min before NOW
    }
    r.accounts.iter_mut().find(|a| a.label == "acct2").unwrap().last_sample = Some("2026-11-21T06:00:00+00:00".into());
    let s = strip_of(&r);
    assert_eq!(acct2(&s).badge.as_deref(), Some("stale, last sample 30 min ago"));
    assert!(acct2(&s).seven.as_ref().unwrap().stale);
    golden("stale", &s);
    // exactly at the limit (15 min) is still fresh
    let mut r = live();
    for w in &mut r.accounts.iter_mut().find(|a| a.label == "acct2").unwrap().windows {
        w.sampled_at = Some("2026-11-21T06:15:00Z".into());
    }
    r.accounts.iter_mut().find(|a| a.label == "acct2").unwrap().last_sample = Some("2026-11-21T06:15:00Z".into());
    assert!(acct2(&strip_of(&r)).badge.is_none());
}

#[test]
fn lapsed_and_last_answer_and_no_reading_are_each_said_plainly() {
    let s = strip_of(&live());
    let amb = s.accounts.iter().find(|a| a.label == "ambience").unwrap();
    assert_eq!((amb.state.as_str(), amb.lock.as_deref()), ("lapsed", Some("payment past due")));
    let cenet = s.accounts.iter().find(|a| a.label == "cenet").unwrap();
    assert!(cenet.badge.as_deref().unwrap().starts_with("stale, last sample 7 d ago"), "{:?}", cenet.badge);
    // a window with no number is "unreadable: <why>", never blank and never a made-up number
    let mut r = live();
    seven(&mut r, |w| {
        w.pct = None;
        w.locked_reason = Some("usage API refused".into());
    });
    let g = acct2(&strip_of(&r)).seven.clone().unwrap();
    assert_eq!((g.state.as_str(), g.value.as_str(), g.note.as_str()), ("unreadable", "", "unreadable: usage API refused"));
    // an account with no window at all
    let mut r = live();
    r.accounts.iter_mut().find(|a| a.label == "acct2").unwrap().windows.clear();
    assert_eq!(acct2(&strip_of(&r)).unreadable.as_deref(), Some("unreadable: no window sampled for this account"));
}

#[test]
fn an_email_in_a_label_never_reaches_the_strip() {
    let mut r = live();
    r.accounts[0].label = "someone@example.com".into();
    r.devices[0].logins = vec!["other@example.com".into()];
    r.keys[0].key_ref = "k@x".into();
    let j = serde_json::to_string(&strip_of(&r)).unwrap();
    assert!(!j.contains("example.com") && !j.contains("k@x"), "{j}");
}

#[test]
fn conversation_cost_reads_tokens_dollars_time_with_model_account_device_in_the_tip() {
    let c: apps_shell::ai_usage_api::ConvRow = serde_json::from_value(serde_json::json!({
        "tenant": "rydenfalk", "conversation": "c1", "provider": "claude", "account": "acct2", "device": "foreman-ops-1", "model": "claude-opus-5-5",
        "turns": 12, "input": 38000, "output": 3100, "cache_read": 900000, "cache_write": 0, "cost_usd": 0.82, "wall_ms": 720000,
        "first_at": "2026-11-21T05:00:00Z", "last_at": "2026-11-21T05:12:00Z"
    }))
    .unwrap();
    let k = cost("c1", Some(&c));
    assert_eq!(k.text, "41k tok \u{b7} $0.82 \u{b7} 12 min");
    assert_eq!(&k.tip[..3], ["model claude-opus-5-5", "account acct2", "device foreman-ops-1"]);
    let none = cost("c2", None);
    assert!(!none.known && none.text == "no usage recorded");
}

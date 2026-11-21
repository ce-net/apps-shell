//! `conv.usage`: the usage strip of the Conversations tab (repo:ce docs/AI-USAGE-AND-DEVICES-2026-11-18.md section 3).
//!
//! A typed UI cell in V2 format: the Rust types below are the contract between this shell and `site/v/conv-usage.js`
//! (`ce-wire-gen --lang ts-types` writes `gen/ConvUsage.d.ts` from them; a test refuses a drifted copy), and every
//! decision the strip makes (which bar is amber, red, capped, not offered, stale; what each line says) is made here in
//! [`build`], from an `ai.usage` Report, so the page only paints what it is given. The page never computes a number
//! and never shows one the report did not hold: a window with no reading is "unreadable: <why>", never blank.
//!
//! Times are written in UTC with the zone named ("resets Thu 09:00 UTC"), so a golden test is the same on every host.
//! Accounts are labels, API keys are their key-ref, devices their name: anything with an "@" in it is replaced
//! ([`clean`]), so no login email can reach the page even if a collector wrote one.

use crate::ai_usage_api::{AccountLine, ConvRow, DeviceRow, KeyRow, Report, WindowRow};

ce_wire::api! {
    api ConvUsage = "conv.usage", version 1;
    types {
        /// Health contract: these three fields, in this order, in every cell.
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        /// `device`: the device whose chip to show (a name from ai.usage Devices); None: the one serving the page.
        pub struct StripReq { pub device: Option<String> }
        /// One bar. `name`: 5h, 7d or day. `state`: normal, warn (80 and over), bad (90 and over), capped (100 and over),
        /// off (the plan has no such window) or unreadable. `value` is the percent as text ("16%"), empty when there is
        /// no number; `note` the line under or beside it ("resets Thu 09:00 UTC", "no 5-hour limit on this plan",
        /// "capped, resets Thu 09:00 UTC", "unreadable: <why>"). `bar` is 0 to 100, for a 4 px bar.
        pub struct Gauge {
            pub name: String, pub state: String, pub pct: Option<f64>, pub value: String, pub note: String,
            pub bar: u32, pub stale: bool
        }
        /// A model-scoped week as a small chip: `name` Fable, Opus, Sonnet.
        pub struct Chip { pub name: String, pub state: String, pub value: String, pub tip: String }
        /// One account. `state`: ok, warn, bad, capped, lapsed or unreadable. `lock` is the text of the red lock when
        /// capped or lapsed ("capped, resets Thu 09:00 UTC", "payment past due"). `badge` is the stale badge ("stale,
        /// last sample 3 d ago"). `unreadable` is why nothing could be read ("unreadable: no window sampled").
        pub struct AccountStrip {
            pub label: String, pub plan: String, pub state: String, pub lock: Option<String>, pub badge: Option<String>,
            pub unreadable: Option<String>, pub five: Option<Gauge>, pub seven: Option<Gauge>, pub day: Option<Gauge>,
            pub chips: Vec<Chip>
        }
        /// One API key by its keys-cell ref: this month's cost, and the day's when the report held it.
        pub struct KeyStrip {
            pub key_ref: String, pub provider: String, pub today: Option<String>, pub month: String,
            pub tip: String, pub stale: bool
        }
        /// The device the strip is shown for: name, class, online and the account labels logged in on it.
        pub struct DeviceChip { pub name: String, pub class: String, pub online: bool, pub logins: Vec<String>, pub tip: String }
        /// The whole strip. `keys_today` is every key's cost today (the report holds one total, not one per key).
        pub struct Strip {
            pub at: String, pub accounts: Vec<AccountStrip>, pub keys: Vec<KeyStrip>, pub keys_today: Option<String>,
            pub device: Option<DeviceChip>, pub alerts: Vec<String>, pub last_event: Option<String>
        }
        pub struct CostsReq { pub ids: Vec<String> }
        /// One conversation's `tokens . $cost . time`. `known` false: ai.usage holds no row for it yet (`text` then
        /// says so). `tip` is model, account label, device.
        pub struct ConvCost { pub id: String, pub known: bool, pub text: String, pub tip: Vec<String> }
        pub struct Costs { pub rows: Vec<ConvCost> }
        pub struct WaitReq { pub since: Option<String>, pub wait_ms: u32 }
        /// `changed`: an Events item arrived; `at` is its time (pass it back as `since`).
        pub struct Waited { pub changed: bool, pub at: Option<String> }
        pub enum StripError { Invalid(String), Unavailable(String) }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "ai.usage:read";
        2 => StripOp(StripReq) -> Result<Strip, StripError>, needs "ai.usage:read";
        3 => CostsOp(CostsReq) -> Result<Costs, StripError>, needs "ai.usage:read";
        4 => WaitOp(WaitReq) -> Result<Waited, StripError>, needs "ai.usage:read", deadline_ms 40000;
    }
}

/// Older than this a sample is stale (15 minutes).
pub const STALE_S: i64 = 15 * 60;

// ---- time, without a date crate: RFC 3339 in, seconds since 1970 out ----

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Seconds since 1970 of an RFC 3339 time (`2026-11-26T09:00:00.021901+00:00`, `...Z`, any offset), or None.
pub fn parse_time(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let mut rest = &s[19..];
    if let Some(r) = rest.strip_prefix('.') {
        let k = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        rest = &r[k..];
    }
    let off = match rest {
        "" | "Z" | "z" => 0,
        r if (r.starts_with('+') || r.starts_with('-')) && r.len() >= 3 => {
            let sign = if r.starts_with('-') { -1 } else { 1 };
            let hh = r.get(1..3)?.parse::<i64>().ok()?;
            let mm = r.get(3..).map(|t| t.trim_start_matches(':')).filter(|t| !t.is_empty()).map(|t| t.parse::<i64>().ok()).unwrap_or(Some(0))?;
            sign * (hh * 3600 + mm * 60)
        }
        _ => return None,
    };
    Some(days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se - off)
}

const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"]; // 1970-01-01 was a Thursday

/// `09:00` or, with `weekday`, `Thu 09:00`, in UTC.
fn clock(t: i64, weekday: bool) -> String {
    let (day, sod) = (t.div_euclid(86400), t.rem_euclid(86400));
    let hm = format!("{:02}:{:02}", sod / 3600, sod % 3600 / 60);
    if weekday {
        format!("{} {hm}", WEEKDAYS[day.rem_euclid(7) as usize])
    } else {
        hm
    }
}

fn ago(secs: i64) -> String {
    let s = secs.max(0);
    if s < 90 {
        "now".into()
    } else if s < 5400 {
        format!("{} min ago", (s + 30) / 60)
    } else if s < 129_600 {
        format!("{} h ago", (s + 1800) / 3600)
    } else {
        format!("{} d ago", (s + 43_200) / 86_400)
    }
}

/// A label that is safe to show: never an email (anything with an "@"), never empty, at most 48 characters.
pub fn clean(s: &str) -> String {
    let t: String = s.trim().chars().filter(|c| !c.is_control()).take(48).collect();
    if t.is_empty() {
        "(unnamed)".into()
    } else if t.contains('@') {
        "(hidden)".into()
    } else {
        t
    }
}

/// `$2.78`, `$0.04`, `<$0.01` for a cost that is not zero, `$0.00` for none.
pub fn usd(v: f64) -> String {
    if !v.is_finite() || v <= 0.0 {
        "$0.00".into()
    } else if v < 0.005 {
        "<$0.01".into()
    } else if v < 100.0 {
        format!("${v:.2}")
    } else {
        let s = format!("{:.0}", v.round());
        let mut o = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                o.push(',');
            }
            o.push(c);
        }
        format!("${o}")
    }
}

/// `812 tok`, `4.1k tok`, `41k tok`, `1.2M tok`.
pub fn tokens(n: u64) -> String {
    if n < 1000 {
        format!("{n} tok")
    } else if n < 10_000 {
        format!("{:.1}k tok", n as f64 / 1000.0)
    } else if n < 1_000_000 {
        format!("{}k tok", (n as f64 / 1000.0).round() as u64)
    } else if n < 10_000_000 {
        format!("{:.1}M tok", n as f64 / 1e6)
    } else {
        format!("{}M tok", (n as f64 / 1e6).round() as u64)
    }
}

/// `45 s`, `12 min`, `1 h 5 min`.
pub fn wall(ms: u64) -> String {
    let s = (ms + 500) / 1000;
    if s < 60 {
        format!("{s} s")
    } else if s < 3600 {
        format!("{} min", s / 60)
    } else if s % 3600 / 60 == 0 {
        format!("{} h", s / 3600)
    } else {
        format!("{} h {} min", s / 3600, s % 3600 / 60)
    }
}

fn level(pct: f64) -> &'static str {
    if pct >= 100.0 {
        "capped"
    } else if pct >= 90.0 {
        "bad"
    } else if pct >= 80.0 {
        "warn"
    } else {
        "normal"
    }
}

fn pct_text(p: f64) -> String {
    format!("{}%", p.round() as i64)
}

fn stale(w: &WindowRow, now: i64) -> bool {
    w.stale_since.is_some() || w.sampled_at.as_deref().and_then(parse_time).map_or(true, |t| now - t > STALE_S)
}

/// One window as a bar. `five`: the 5-hour window (its `offered=false` says "no 5-hour limit on this plan").
fn gauge(w: &WindowRow, name: &str, now: i64, weekday: bool) -> Gauge {
    let when = w.resets_at.as_deref().and_then(parse_time).map(|t| format!("{} UTC", clock(t, weekday)));
    let st = stale(w, now);
    if w.offered == Some(false) {
        let note = if name == "5h" { "no 5-hour limit on this plan".to_string() } else { "not offered on this plan".to_string() };
        return Gauge { name: name.into(), state: "off".into(), pct: None, value: String::new(), note, bar: 0, stale: false };
    }
    let Some(p) = w.pct.filter(|p| p.is_finite()) else {
        let why = w.locked_reason.as_deref().map(clean).unwrap_or_else(|| "the usage API gave no number".into());
        return Gauge { name: name.into(), state: "unreadable".into(), pct: None, value: String::new(), note: format!("unreadable: {why}"), bar: 0, stale: st };
    };
    let capped = p >= 100.0 || w.severity.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("capped"));
    let state = if capped { "capped" } else { level(p) };
    let note = match (capped, when) {
        (true, Some(t)) if w.resets_at.as_deref().and_then(parse_time).is_some_and(|r| r <= now) => format!("capped, reset passed {t}, reading is old"),
        (true, Some(t)) => format!("capped, resets {t}"),
        (true, None) => "capped".into(),
        (false, Some(t)) if w.resets_at.as_deref().and_then(parse_time).is_some_and(|r| r <= now) => format!("reset passed {t}, reading is old"),
        (false, Some(t)) => format!("resets {t}"),
        (false, None) => "no reset time given".into(),
    };
    Gauge { name: name.into(), state: state.into(), pct: Some(p), value: pct_text(p), note, bar: p.clamp(0.0, 100.0).round() as u32, stale: st }
}

fn model_of(kind: &str) -> Option<String> {
    let m = kind.strip_prefix("seven_day_")?;
    let mut c = m.chars();
    Some(c.next()?.to_uppercase().chain(c).collect())
}

fn sev_of(state: &str) -> usize {
    match state {
        "warn" => 1,
        "bad" => 2,
        "capped" => 3,
        _ => 0,
    }
}

fn account(a: &AccountLine, now: i64) -> AccountStrip {
    let mut s = AccountStrip {
        label: clean(&a.label), plan: if a.plan.is_empty() || a.plan == "unknown" { String::new() } else { clean(&a.plan) }, state: "ok".into(),
        lock: None, badge: None, unreadable: None, five: None, seven: None, day: None, chips: vec![],
    };
    let mut newest: Option<i64> = None;
    let mut reset_of_cap: Option<i64> = None;
    let mut latest_reset: Option<i64> = None;
    let mut all_stale = !a.windows.is_empty();
    let mut sev = 0;
    for w in &a.windows {
        let (name, g) = match w.kind.as_str() {
            "five_hour" => ("5h", gauge(w, "5h", now, false)),
            "seven_day" => ("7d", gauge(w, "7d", now, true)),
            "gateway_day" => ("day", gauge(w, "day", now, false)),
            k => {
                if let Some(m) = model_of(k) {
                    let g = gauge(w, "7d", now, true);
                    let tip = format!("{m} week: {}", if g.value.is_empty() { g.note.clone() } else { format!("{}, {}", g.value, g.note) });
                    s.chips.push(Chip { name: m, state: g.state.clone(), value: g.value.clone(), tip });
                    sev = sev.max(sev_of(&g.state));
                    if let Some(t) = w.sampled_at.as_deref().and_then(parse_time) {
                        newest = newest.max(Some(t));
                    }
                    all_stale &= stale(w, now);
                    continue;
                }
                continue;
            }
        };
        if g.state == "capped" {
            reset_of_cap = reset_of_cap.max(w.resets_at.as_deref().and_then(parse_time));
        }
        latest_reset = latest_reset.max(w.resets_at.as_deref().and_then(parse_time));
        if let Some(t) = w.sampled_at.as_deref().and_then(parse_time) {
            newest = newest.max(Some(t));
        }
        all_stale &= g.stale || g.state == "off";
        sev = sev.max(sev_of(&g.state));
        match name {
            "5h" => s.five = Some(g),
            "7d" => s.seven = Some(g),
            _ => s.day = Some(g),
        }
    }
    s.state = ["ok", "warn", "bad", "capped"][sev].into();
    // the account's own state (ai.account): Capped and Lapsed win over what the bars say
    match a.state.to_ascii_lowercase().as_str() {
        "lapsed" => {
            s.state = "lapsed".into();
            s.lock = Some("payment past due".into());
        }
        "capped" => {
            s.state = "capped".into();
        }
        _ => {}
    }
    if s.state == "capped" && s.lock.is_none() {
        let t = reset_of_cap.or(latest_reset);
        s.lock = Some(match t {
            Some(t) => format!("capped, resets {} UTC", clock(t, true)),
            None => "capped".into(),
        });
    }
    if a.payment.eq_ignore_ascii_case("pastdue") && s.lock.is_none() {
        s.state = "lapsed".into();
        s.lock = Some("payment past due".into());
    }
    if !a.windows.is_empty() && a.windows.iter().all(|w| w.offered == Some(false)) && s.state != "lapsed" && s.state != "capped" {
        s.unreadable = Some("unreadable: no usable window sampled".into());
        s.state = "unreadable".into();
    } else if s.five.is_none() && s.seven.is_none() && s.day.is_none() && s.chips.is_empty() && s.state != "lapsed" && s.state != "capped" {
        s.unreadable = Some("unreadable: no window sampled for this account".into());
        s.state = "unreadable".into();
    }
    // stale badge: every window is old (or the usage API refused and these are its last answers)
    let last = a.last_sample.as_deref().and_then(parse_time).or(newest);
    if all_stale || (a.windows.is_empty() && last.map_or(false, |t| now - t > STALE_S)) {
        s.badge = Some(match last {
            Some(t) => format!("stale, last sample {}", ago(now - t)),
            None => "stale, never sampled".into(),
        });
    }
    s
}

fn key(k: &KeyRow, now: i64) -> KeyStrip {
    let st = parse_time(&k.sampled_at).map_or(true, |t| now - t > STALE_S);
    let mut tip = format!("{} {}, {} in, {} out", clean(&k.provider), k.month, tokens(k.tokens_in), tokens(k.tokens_out));
    tip.push_str(&format!(", {}", if k.source == "provider" { "from the provider" } else { "counted here" }));
    if let Some(w) = &k.why {
        tip.push_str(&format!(". {}", clean(w)));
    }
    KeyStrip { key_ref: clean(&k.key_ref), provider: clean(&k.provider), today: None, month: usd(k.month_spend.unwrap_or(k.cost_usd)), tip, stale: st }
}

fn device(d: &DeviceRow, accounts: &[AccountLine]) -> DeviceChip {
    let mut logins: Vec<String> = accounts.iter().filter(|a| a.logged_in_on.iter().any(|n| n == &d.name)).map(|a| clean(&a.label)).collect();
    for l in &d.logins {
        let l = clean(l);
        // "default" is an unlabelled login: named only when no account label says whose it is
        if !logins.contains(&l) && !(l == "default" && !logins.is_empty()) {
            logins.push(l);
        }
    }
    let clis: Vec<String> = d.clis.iter().map(|c| clean(&c.name)).collect();
    let mut tip = format!("{} {} {}, {}", clean(&d.os), clean(&d.arch), clean(&d.class), if d.online { "online" } else { "offline" });
    if !clis.is_empty() {
        tip.push_str(&format!(". CLIs: {}", clis.join(", ")));
    }
    if let Some(b) = d.battery.as_ref().and_then(|b| b.pct) {
        tip.push_str(&format!(". Battery {}%", b.round() as i64));
    }
    DeviceChip { name: clean(&d.name), class: clean(&d.class), online: d.online, logins, tip }
}

/// The strip from an ai.usage Report. `now` is seconds since 1970; `here` is the device name to show (the one serving
/// the page when the caller names none): the report's device of that name, else the only online one, else the first.
pub fn build(r: &Report, now: i64, here: Option<&str>) -> Strip {
    let mut accounts: Vec<AccountStrip> = r.accounts.iter().map(|a| account(a, now)).collect();
    // working accounts first (they are what you can use), then the rest; each group by label
    let rank = |s: &AccountStrip| match s.state.as_str() {
        "lapsed" => 3,
        "unreadable" => 2,
        _ if s.badge.is_some() => 2,
        _ => 0,
    };
    accounts.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.label.cmp(&b.label)));
    let mut keys: Vec<KeyStrip> = r.keys.iter().map(|k| key(k, now)).collect();
    keys.sort_by(|a, b| a.key_ref.cmp(&b.key_ref));
    let dev = here
        .and_then(|h| r.devices.iter().find(|d| d.name == h))
        .or_else(|| if r.devices.iter().filter(|d| d.online).count() == 1 { r.devices.iter().find(|d| d.online) } else { None })
        .or_else(|| r.devices.first())
        .map(|d| device(d, &r.accounts));
    let keys_today = (!r.keys.is_empty()).then(|| usd(r.today.keys_usd));
    Strip {
        at: r.at.clone(),
        accounts,
        keys,
        keys_today,
        device: dev,
        alerts: r.alerts.iter().take(3).map(|a| a.text.chars().take(160).collect()).collect(),
        last_event: r.alerts.iter().map(|a| a.at.clone()).max(),
    }
}

fn or_unknown(s: &str) -> String {
    if s.trim().is_empty() { "not recorded".into() } else { clean(s) }
}

/// `tokens . $cost . time` of one conversation (ai.usage ConvRow), with the model, account and device in the tooltip.
pub fn cost(id: &str, row: Option<&ConvRow>) -> ConvCost {
    match row {
        None => ConvCost { id: id.into(), known: false, text: "no usage recorded".into(), tip: vec![] },
        Some(c) => ConvCost {
            id: id.into(),
            known: true,
            text: format!("{} \u{b7} {} \u{b7} {}", tokens(c.input + c.output), usd(c.cost_usd), wall(c.wall_ms)),
            tip: vec![
                format!("model {}", or_unknown(&c.model)),
                format!("account {}", or_unknown(&c.account)),
                format!("device {}", or_unknown(&c.device)),
                format!("{} in, {} out, {} turns", tokens(c.input), tokens(c.output), c.turns),
            ],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_parse_and_print_in_utc() {
        assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_time("2026-11-26T09:00:00.021901+00:00"), parse_time("2026-11-26T10:00:00+01:00"));
        let t = parse_time("2026-11-26T09:00:00+00:00").unwrap();
        assert_eq!(clock(t, true), "Thu 09:00"); // 26 Nov 2026 is a Thursday
        assert_eq!(clock(t, false), "09:00");
        assert_eq!(parse_time("garbage"), None);
    }

    #[test]
    fn numbers_read_the_way_the_strip_shows_them() {
        assert_eq!((usd(0.0), usd(0.003), usd(2.775282), usd(1234.4)), ("$0.00".into(), "<$0.01".into(), "$2.78".into(), "$1,234".into()));
        assert_eq!((tokens(812), tokens(4100), tokens(41_000), tokens(1_200_000)), ("812 tok".into(), "4.1k tok".into(), "41k tok".into(), "1.2M tok".into()));
        assert_eq!((wall(45_000), wall(720_000), wall(3_900_000)), ("45 s".into(), "12 min".into(), "1 h 5 min".into()));
        assert_eq!(clean("me@example.com"), "(hidden)");
        assert_eq!(clean("acct2"), "acct2");
    }
}

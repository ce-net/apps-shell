# The usage strip (`conv.usage`, top of apps.ce-net.com/conversations/)

Design: repo:ce `docs/AI-USAGE-AND-DEVICES-2026-11-18.md` section 3; app context repo:ce `docs/AI-AS-CELLS-2026-11-18.md`
section 6. Built 2026-11-21 on foreman-ops-1 (lane foreman/win-2c3d). Additive: nothing that exists switches.

## What it shows

Per account (label, never an email): `5h` bar, percent and "resets HH:MM UTC"; `7d` bar and "resets Thu 09:00 UTC";
model weeks (Fable, Opus, Sonnet) as chips; per API key (key ref) its cost this month, with every key's cost today in the
header (the Report holds one today-total, not one per key, so no per-key "today" is invented); one device chip (name,
class, online, the account labels logged in on it); one dot for the connection (idle, finding, live, stale,
reconnecting, refused, failed). Under it `conv.list` rows and the `conv.thread` header show `tokens . $cost . time`
(tooltip: model, account label, device) from `ai.usage Conversation{id}`.

| State | When | Shown |
|---|---|---|
| normal | window under 80 % | green 4 px bar, percent right-aligned, reset time |
| amber | 80 % and over | amber bar and percent |
| red | 90 % and over | red bar and percent |
| capped | 100 % and over, severity `capped` or account `Capped` | red lock "capped, resets Thu 09:00 UTC" |
| lapsed | account `Lapsed` or payment `pastdue` | red lock "payment past due" |
| not offered | `offered == false` | grey "no 5-hour limit on this plan" (no number, no bar) |
| stale | `stale_since` set or `sampled_at` older than 15 min | badge "stale, last sample 7 d ago" |
| unreadable | no number | "unreadable: <why>", never blank, never a made-up number |

## How it is built (V2: typed Rust, generated TypeScript, one slot)

- `crates/apps-shell/src/strip.rs`: the `conv.usage` api! block (types `Strip`, `AccountStrip`, `Gauge`, `Chip`, `KeyStrip`,
  `DeviceChip`, `ConvCost`; ops StripOp, CostsOp, WaitOp) and `build(Report, now, device) -> Strip`, which makes every
  decision above. `gen/ConvUsage.d.ts` is generated from it (`ce-wire-gen --lang ts-types src/strip.rs -o gen --api
  ConvUsage`, the one in ~/.local/bin); `tests/ts_drift.rs` fails when the committed file differs or its schema hash is
  not the compiled one.
- `site/v/conv-usage.js` only paints a `Strip`. `conversations.js` mounts it above the two columns through a dynamic
  import (the module is served only to a viewer holding `ai.usage:read`; others get the tab without the strip) and
  `shell.js` hands the layout `host.other['conv.usage']` for a view that names it in `view.also`.
- Relay `POST /api/cell/conv.usage/<StripOp|CostsOp|WaitOp>` on the Conversations face only, with a grant wire.ws-door
  mints for the viewer's own `ai.usage:read`. The request names no tenant: ai.usage answers its home tenant, so a
  viewer cannot ask for another. StripOp is `ai.usage Read{scope:"all"}`; WaitOp holds the `Events` stream open up to
  25 s and says "read again" (the page also reads every 60 s, because a percent moves without an event); CostsOp is
  `Conversation{id}` for up to 120 ids, asked only for the rows in view.
- `ai.usage@1` is copied verbatim into `src/ai_usage_api.rs` (schema `0x4a104f9ecd492dcd`, pinned by a test).
- Grant: `wire.ws-door:mint-for:ai.usage`. ROOT and Leif's people entry gain `ai.usage:read` (and Leif's entry
  `ai.session:list`, `ai.session:read`, which it lacked, so the tab was not his).

## Tests

`cargo test` here: `tests/strip.rs` (golden Strips for normal, 80, 90, capped, not offered and stale, built from the real
Report of 2026-11-21 in `tests/fixtures/` with one window changed; plus lapsed, unreadable, no email), `tests/render.rs`
(the same six painted by `conv-usage.js` under node against a minimal DOM, golden markup in `tests/golden/render-*.html`;
no inline style, no email), `tests/ts_drift.rs`. `UPDATE_GOLDEN=1 cargo test` rewrites goldens. CI sets
`CE_REQUIRE_WIRE_GEN=1 CE_WIRE_GEN=~/.local/bin/ce-wire-gen` and `CE_REQUIRE_NODE=1`.

## Not in this slice

- Per-conversation cost lines appear only where ai.usage holds a row under the same id. Today ai.usage.turns keys lanes as
  `lane-<name>` and ai.session names conversations `i<hex>`; the lines light up when ai.session records its turns through
  ai.usage.turns with its own ids (the providers workflow). The relay and views are ready.
- Per-key "today" (ai.usage.key writes months). Times are UTC, not the viewer's zone.

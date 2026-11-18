# The Feed tab (apps.ce-net.com/feed/)

Package B of slice 1 of repo:ce `docs/FOREMAN-AS-CELLS-V2-2026-11-18.md` (sections 5 and 6.4), built 2026-11-18 by
foreman/win-2c3d. A shadow: the old feed in /foreman/ stays live and nothing Leif sees switches. The Feed tab is the
cell `feed.view` major 2 drawn by a view module that the cell itself ships.

## Who does what

| piece | where | what |
|---|---|---|
| the view | repo:feed-view `ui/feed.js` | one ES module, `view = { cell: "feed.view", name: "Feed", speaks: [2], kit: 1 }`, `mount(root, host)`; embedded in the feed-view binary and handed out by op 7 `Ui{view: "Feed"}` as `Bundle{sha256, speaks, kit, bytes}` |
| the tab | repo:ce-net-net `FEED` (90554dd) | `cell feed.view@2 replicas 2; tab "Feed" route "/feed/" order 11 needs "feed.view:read" shell feed.view@2` |
| the host | this repo | `GET /v/feed.js`, `GET /_c/<sha256>.js`, `POST /api/cell/feed.view/<Op>`, `_k/shell.js` gives the view its `host` |
| the route | repo:ce-fleet `door/gate.py` | `/feed/` to 127.0.0.1:8884, a cell face (hops to ops-1 like /search/) |

net! allows one tab a system and wants the tab's shell in that system, and FOREMAN already holds its tab, so FEED is
its own system (needs FOREMAN) and holds `feed.view@2`; `feed.view@1` stays in TALK (the one-system test now keys a
cell by name and major).

## How a viewer reaches it

- `/tabs.json` lists the Feed tab for a person whose abilities cover `feed.view:read` while `feed.view` answers
  `HealthOp` by name (apps.shell's own grant holds `feed.view:health`). `view` is `v/feed.js`.
- `GET /v/feed.js` calls `Ui{view: "Feed"}` as the viewer (a grant minted by wire.ws-door), checks the answer against
  its own sha256 and the majors (contract 2, kit 1), and serves the bytes. A viewer without the tab gets the same 404 as
  for a file that does not exist. The bytes are kept at `/_c/<sha256>.js` (immutable, only for a viewer with the tab).
- `POST /api/cell/feed.view/<Op>` passes Page, Topic, Changes, Mark and Close only (JSON in and out,
  `Content-Type: application/json`, 64 KB). It is called with a grant wire.ws-door minted for the viewer:
  their own `feed.view:*` abilities and the row caveat `feed.view:where:person=<id>`. The cell reads the person from
  that caveat (v2::viewer; `ability:person:<id>` works too), so read marks are per person and survive a new grant.
  apps.shell's own grant gained exactly `wire.ws-door:mint-for:feed.view` and `feed.view:health`: the right to call
  is the viewer's, and feed.view's own gate decides (a reader gets 403 on Mark and Close from the cell).
- `_k/shell.js` builds the host for a module that exports `view`: `host.h`, `host.call(op, req)`,
  `host.watch(op, req, onDelta)` (a long poll; `req` may be a function; `onDelta(null, error)` then a retry with
  backoff, 1 s to 30 s) and `host.open(target)` (https in a new tab; a `repo:` ref in the reference viewer). `call` and
  `watch` reach only `view.cell`. The shell script and every other file of this site never name the feed cell (a test
  greps `site/`).

## The view

Filters For you (default), Alarms, Everything; a switch Unread; the unread count; one card per topic (lane, time,
title, summary, link chips through `host.open`, "n lines", "Read all" which opens the thread in place with each folded
or hidden line's rule id, and every line's rule under Everything); a quiet row under Everything with the hidden counts
(agent notes and receipts open); the alarm card ("rang n times, from - to", Join = `Close{Held}` then open the link,
Dismiss = `Close{Dismissed}`, closed alarms collapsed under Alarms); read marks when a card was fully on screen for one
second or its thread is opened, "Mark all read" for the filter (every unread card of it, loaded or not); live through
`Changes` (put and gone applied to the cards, the list is not redrawn). Styles are scoped rules in the module (a
constructable stylesheet: CSP allows no inline style), on the kit's tokens, 390 px first, light and dark by the shell's
button. The module has no `fetch(`, `document.cookie`, `localStorage`, URL or other cell (feed-view test).

## Proof (2026-11-18, foreman-ops-1, test persons only; screenshots and result files in `docs/feed-proof/`)

Run against a feed.view started on a synthetic fixture (`FEED_FIXTURE=<rows.jsonl>`: major 2 only, marks in memory,
rows re-read on every call), through a test copy of the gate (door/gate.py with the /feed/ route, port 18896) and a
headless Chromium driven over the DevTools protocol. The live gate was not restarted (talk-messages gate said WAIT).

- `t-feed-owner` (read, mark, close), 390 px and 1280 px, 14 checks each, all pass: Feed tab in the bar; no horizontal
  scroll; the alarm is one card "rang 28 times"; Join closes it in 113 ms and it leaves For you; closed alarms collapsed
  under Alarms; a new fixture row appears in 0.8 to 0.9 s with no reload; Mark all read sets the count to 0 and it
  stays 0 after reload; Everything shows "1 agent note, 3 machine lines, 1 receipt, 28 repeats" and a hidden line
  with its rule id; no console error, exception or CSP violation. `owner-390-*.png`, `owner-1280-*.png`.
- `t-feed-stranger` (`search.hybrid:find` only): no Feed tab; `/v/feed.js` 404; POST Page, Mark and Ui 403; `/_c/<hash>`
  404; none of the 11 responses the browser received contains "feed.view" or the bundle hash.
- `t-feed-reader` (`feed.view:read` only): Mark and Close refused by the cell (403), the view says "You can read this
  feed. It does not keep what you have read for you." and "You can read alarms here, not close them." and keeps working.
- Bundle sha256 `b0de6ac7938cbff1506903330506de4281da2f9b51fa4cc85c931124558d425e` (22,7 KB), served identical to the
  repo file.
- Tests: `cargo test --workspace` in apps-shell (contract hash `0xf226abb16f72c818` pinned, tab rules, grant, no site
  file names the cell), feed-view (Ui bundle, no fetch/cookie/storage), ce-net-net (9).

## Installed and how to swap back

apps.shell on foreman-ops-1 runs this build (`~/.local/bin/apps-shell`, previous binary
`~/.local/bin/apps-shell.bak-20261118-win2c3d`). Its grant (`~/.local/share/ce/cell-keys/apps.shell/grant.hex`) was
reissued with `ce-iam grant` for the old list plus `wire.ws-door:mint-for:feed.view` and `feed.view:health`; the old
one is `grant.hex.bak-20261118-win2c3d`. Swap back:

```
cp ~/.local/bin/apps-shell.bak-20261118-win2c3d ~/.local/bin/apps-shell && ce cell restart apps.shell
cp ~/.local/share/ce/cell-keys/apps.shell/grant.hex.bak-20261118-win2c3d ~/.local/share/ce/cell-keys/apps.shell/grant.hex   # optional
```

The ports are 8881 to 8884 (`APPS_SHELL_HTTP`); the old binary serves the first three and ignores a Feed tab it does
not know.

## Not done

- feed.view@2 is not installed in the live door (package A's install lost v1; its fix 2972849 is not installed), so
  no one has the Feed tab yet and `/feed/` through the public gate waits for: the install, a gate snapshot with the
  /feed/ route on ops-1 (window OPEN, proxy-groups check) and then Debian and leif-desk (ask the lead).
- The tab badge (unread count in the bar) is not wired: it would call Page on every 3 s tab poll.
- `host.open` of a ce-ref other than `repo:` searches for it until REFS `open.with` exists.

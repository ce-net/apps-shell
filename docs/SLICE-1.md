# Sites on cells, slice 1: as built and proved (2026-11-16)

Slice 1 of repo:ce/docs/SITES-ON-CELLS.md section 11, on foreman-ops-1 and leif-desk. What runs, how it was proved
through the public https://apps.ce-net.com with scoped test persons (ended after use), and what is not done.

## What runs where

| Cell / piece | Repo @ commit | foreman-ops-1 | leif-desk |
|---|---|---|---|
| `apps.shell@1` (tabs, gateway face 8881 `/approvals/`, 8882 `/core/`) | repo:apps-shell @ 9584048 | cell.install Repo, running | not installable (card `gb-b3a1c4df346e3d0e` item 2) |
| `approve.inbox@1` (APPROVALS system) | repo:approve-inbox @ 6d931c8 | running | same |
| `site.cellmap@1` (Core tab) | repo:site-cellmap @ db24688 | running | same |
| `foreman.shell@1` (FOREMAN system's shell) | repo:foreman-shell @ 9afec16 | running | same |
| cell systems `CORE`, `APPROVALS`, `CONTROL_ROOM`, `FOREMAN`, `CE_NET` | repo:ce-net-net @ 1e39b62 | compiled into the cells | - |
| `net!` `include`, `needs [SYSTEM]`, `tab` | repo:cell-stack @ 046f906 | - | - |
| gate snapshot `ddb233e1ddbd-3f2031adaee8` (`81c7453a038896f0`): PORT_PATHS `approvals`, `core`, `cell_face_hop` | repo:ce-fleet @ ddb233e1 | live | live; hops `/approvals/`, `/core/` to ops-1's gate (CE_GATE_WORK_HOP) |

Keys: each cell's own key and node-minted grant in `<ce data dir>/cell-keys/<api>/` (apps.shell: ws-door mint-for
approve.inbox and site.cellmap, and the four tab shells' health, no data right; approve.inbox: grant.bundle:read,
needs.list:read; site.cellmap: cell.install:list, plus `peers/<node>.hex` grants leif-desk and the Mac issued,
attenuated to exactly `cell.install:list`; the Mac's ends after 24 h, its parent's limit). People:
`~/.config/apps-shell/people.json` on ops-1 (now only `leif`).

## Proofs (public https://apps.ce-net.com, headless Chromium, scoped test persons)

1. **Tabs by capability.** `t-sites-none` (no rights): tab bar empty, `api/inbox` 403, `v/core.js` 404; every byte
   the browser received searched: no system, tab, view or cell name, only the URL it asked for (`proof/none.png`).
   `t-sites-appr` (`approve.inbox:read` + `approve.inbox:where:requester=t-sites-appr`): one tab, and of a dry-run
   bundle with an item for it and one for `t-sites-op`, only its own (`proof/appr-1440.png`, `appr-390.png`,
   `appr-card.png`). `t-sites-op` (+ `cr.shell:open`, `site.cellmap:read`): Approvals, Control room, Core
   (`proof/op-core.png`). The row cut is made by approve.inbox's own gate on a viewer grant wire.ws-door minted.
2. **Grant and end on an open page.** `foreman.shell:open` added to `t-sites-op` with the Core page open: Foreman
   tab after 0.98 s; removed: gone after 3.3 s (`proof/grant.log`).
3. **Liveness.** `cell.install Stop approve.inbox` with the Core page open: badge gone after 3.5 s, Approvals tab gone
   after 8.4 s, APPROVALS red (3 / 8) in the Core view (`proof/live-21298.png`); `Start`: tab back after 1.1 s, badge
   after 4.4 s (`proof/live.log`).
4. **Either gate.** A session made through ops-1's front was accepted by leif-desk's gate on the first try (1.1 s,
   including the ssh connect); ending it on ops-1 was refused by leif-desk after 2.2 s. Through leif-desk's gate
   `/approvals/tabs.json`, `/approvals/api/inbox`, `/core/tabs.json` answer 200 (0.2 to 0.4 s, hopped to ops-1).
5. **Releases.** `cell.install Restart apps.shell` (new process verified before the old stops): 82 of 82 public
   requests 200 during it. Before SO_REUSEPORT the new process could not bind; Restart kept the old one serving by
   itself (the failing build never replaced the running one). `Upgrade` (stop, then start): 1 of 166 requests 502.
6. **Break-proof.** A tab needing a wildcard right, or a system cell needing a cell no included system places, fails
   `cargo build` (repo:cell-stack trybuild `net_tab_wildcard`, `net_system_needs_unplaced`).

Tests: cell-stack cell-net 3 + 2 trybuild, ce-net-net 2, approve-inbox 4, site-cellmap 2, apps-shell 6, ce-fleet
`test_gate_failover` 31 (5 new).

## Not done, and why

- **Second replica on leif-desk** of all four cells, and so a real node-loss drill (ops-1's gateway and gate down,
  leif-desk serving with its own replicas): leif-desk's `cell.install` takes nothing from foreman/lead until Leif taps
  card `gb-b3a1c4df346e3d0e` item 2. Today the `/approvals/` and `/core/` tabs on leif-desk's front go to ops-1's gate,
  so losing ops-1 loses them. Public DNS points at ops-1 only (Cloudflare token, item 1).
- Site root key and node certificates (section 4.2), `account.session` read-through and revoke fan-out,
  `gate.session-sync`: not built; the gates' existing session flood already gave 1.1 s / 2.2 s here.
- `feed.approve Act` (typed approve with a fresh WebAuthn assertion) and state.store records for approvals: signing
  stays on feed.approve's `/grant` card, linked from the Approvals card. `approve.ledger`, `approve.keys`: not built.
- Views are plain modules carried by apps.shell, polled every 3 to 5 s; not yet `views!` contracts with ui-gen types
  (so a renamed field does not fail `tsc`), not one ws-door socket, no `ManifestDelta` push.
- Foreman and Control room tabs link to their existing pages; those pages do not draw the computed tab bar yet.
- Viewer abilities come from the node's people file, not yet from `person.grant` and the session's chain.
- work-show as one artifact through `cell.build`/`artifact.index`/`orch` (step 7), and `ce site plan/apply`: not done.

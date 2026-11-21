# The Architecture tab (apps.ce-net.com, `/core/?view=architecture`)

ce-net drawn from what its own cells report, with notes on any node. Design: repo:ce/docs/ARCHITECTURE-ATLAS-APP-2026-11-18.md
(slice 1 discover and draw, slice 2 the tab and the notes). The picture is arch.map's own view module; this site only
mounts it and relays as the viewer.

**The gate route `/architecture/` is not added yet.** The gate on foreman-ops-1 (repo:ce-fleet snapshot
`4bfabd85e82a-16c2dd95730f`) does not route it and is not restarted for this slice (quiet window after the
proxy-groups/work-show drop-in check, `repo:ce-fleet/docs/GATE-DEPLOY.md`). Until then the view is reachable at
**https://apps.ce-net.com/core/?view=architecture**, and the Architecture tab in the bar links there.

## Groups

| who | needs | sees |
|---|---|---|
| tab and view | `arch.map:see` | the Architecture tab (ce-net-net `ARCHITECTURE`, order 16), `GET /v/architecture.js` (arch.map `Ui` as the viewer) |
| full map | `arch.map:internal` | GraphFull (Layers, Systems, Machines, Caps), DetailFull, Sources, Changes; a `see`-only viewer gets 403 from arch.map's own gate |
| notes strip | `arch.notes:write` (+ `arch.notes:read` for the list) | the Notes strip in the detail pane: open notes on that node, a box, Save (arch.notes `Put`), Close on each (its author's) |
| nothing | | no tab, `v/architecture.js` 404, every relay 403 |

## Wiring

- Faces: `$APPS_SHELL_HTTP` default gains `127.0.0.1:8887=/architecture/` (8881 approvals, 8882 core, 8883 search,
  8884 feed, 8885 home, 8886 conversations; the design named 8886, taken by Conversations the same morning). Core (`/core/?view=architecture`, `EXTRA_VIEWS`) and the tab's own face serve the same module
  and relays (`ARCH_FACES`); Core's `?view=` stays at least 14 days after the gate route is live.
- Tab: ce-net-net 1fde9d1 (`ARCHITECTURE`: arch.map@1 and arch.notes@1, both replicas 2 on foreman-ops-1 and leif-desk;
  `tab "Architecture" route "/architecture/" order 16 needs "arch.map:see" shell arch.map@1`). It emerges by itself:
  the liveness loop asks arch.map `HealthOp` by name (contract copied, `arch.map:health`). While the gate lacks the
  route, `tabs.json` gives the tab `href: "/core/?view=architecture"` (`NOT_ROUTED_YET`); the bar links there and marks it
  current there.
- `POST /api/cell/arch.map/<Op>`: `ARCH_OPS` = GraphFull, DetailFull, Sources, Graph, Detail, Story, Progress, Changes
  (Changes holds up to 30 s: the client deadline is 35 s for it). Graph, Detail, Story, Progress answer
  `Invalid("not built")` (400) until slice 3.
- `POST /api/cell/arch.notes/<Op>`: Put, List, Close only (`NOTES_OPS`, each with the viewer's right checked here first),
  with a grant wire.ws-door mints for the viewer: their own `arch.notes:*` rights plus `arch.notes:where:author=<them>`,
  so arch.notes writes a note only as its author (a Put naming anyone else: 403 "this caller may not write as ...").
- `tabs.json` gains `notes` (`"arch.notes"` only for a viewer holding `arch.notes:write` on an Architecture face); the
  shell hands the view `host.notes.call(op, req)` only then (the module asks with `view.notes: true`), so a `see`-only
  viewer gets no strip and no notes call at all. No site file names arch.map (test); the module names no cell.
- apps.shell's grant (`~/.local/share/ce/cell-keys/apps.shell/grant.hex`, reissued with `ce-iam grant` for `GRANT`;
  previous `grant.hex.bak-20261121-win2c3d`) gains `wire.ws-door:mint-for:arch.notes` and `arch.notes:health`.
- Contracts copied verbatim and pinned: arch.map `0x934e4e8a827ab7ab` (frozen), arch.notes `0xa4dc01ad901bfcda` (frozen
  whole at repo:arch-notes 534c49f). apps.shell's own schema is unchanged.
- People (`~/.config/apps-shell/people.json` on foreman-ops-1; previous `people.json.bak-20261121-win2c3d`): leif gains
  `arch.notes:write`, `arch.notes:read`; `t-arch-owner` gains both; `t-arch-see` none.
- Shell fixes found while proving: a long tab title wraps in the rail ("Architecture" was cut); the Bar no longer
  pushes the page to 438 px at 390 (Copy is hidden on a phone).

## Proof (2026-11-21, foreman-ops-1, through https://apps.ce-net.com, test persons with test sessions, ended after)

`docs/arch-proof/`: `proof.log` and screenshots 1440x900 and 390x844 of the real page.

- `t-arch-owner` (see, internal, notes write+read): opened site.cellmap's detail, closed the open note there (author),
  typed a note and pressed Save: "Saved.", and the node showed `1` (its open notes) **4.3 s** after Save (arch.map's
  notes source reads arch.notes every 5 s; the page reads the map again on Save and whenever arch.map's seq rises,
  through the Changes long-poll). `ce cell call arch.notes List
  '{"target":"cell:site.cellmap@1","open_only":true,"since":null,"max":10}'` shows the row (author `t-arch-owner`);
  GraphFull Systems/CORE has `cell:site.cellmap@1` notes 1 (`owner-1440-3-saved-notes-1.png`, `owner-390-2-notes-strip.png`).
  Caps lens: one card per CE_NET cell with the rights its ops need (`owner-1440-4-caps.png`).
- `t-arch-see`: the tab and the module (200), no strip (`notes: null`), "The full map is not open to you" from the
  cell, `arch.notes/Put` 403, `arch.notes/List` 403, GraphFull 403, Changes 403 (`see-1440-no-strip.png`).
- `t-arch-stranger`: no tab, `v/architecture.js` 404, every relay 403 (`stranger-1440.png`).
- No horizontal scroll at 390 (scrollWidth 390).

Tests: `cargo test --release` 36 (apps-shell, with the Conversations tab of f99f235), 28 (arch-map), 9 (arch-notes), 19 (ce-net-net).

## Installed, and how to swap back

`~/.local/bin/apps-shell` runs this build; the previous binary is `~/.local/bin/apps-shell.bak-20261121-win2c3d`
(restart verified: `ce cell restart apps.shell`, new process answered before the old one stopped). Swap back:

```
cp ~/.local/bin/apps-shell.bak-20261121-win2c3d ~/.local/bin/apps-shell && ce cell restart apps.shell
```

## Not done

- The gate route `/architecture/` (quiet window, `repo:ce-fleet` `PORT_PATHS["architecture"] = 8887` and a
  `CELL_FACES` entry, deployed as GATE-DEPLOY.md says). The tab links to `/core/?view=architecture` until then.
- The leif-desk replicas of arch.map and arch.notes (ce-net-net wants 2; one runs, on foreman-ops-1). leif-desk's
  cell.install refuses our chain (design 9.1), and arch.notes' collection cap is rooted at foreman-ops-1's key.
- Graph, Detail, Story, Progress (the demo projection) are relayed but answer "not built" until slice 3.

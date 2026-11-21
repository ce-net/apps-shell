# The Conversations tab (apps.ce-net.com/conversations/)

Slice 2 of repo:ce `docs/AI-AS-CELLS-2026-11-18.md` (sections 6 and 8): every AI conversation of the viewer's realm
in one list, read only. Built 2026-11-21 on foreman-ops-1. Additive: nothing that exists switches.

## Who does what

| piece | where | what |
|---|---|---|
| the data | repo:ai-session (`ai.session@1`) | List, Get, Read; rows in state.store app `ai.store`, cut by the caller's tenant; Read of an imported Claude Code session reads the transcript in place on its node through `ai.provider#claude.terminal.<node>` |
| the tab | repo:ce-net-net `CONVERSATIONS` | `cell ai.session@1 replicas 2; tab "Conversations" route "/conversations/" order 12 needs "ai.session:list" shell ai.session@1` |
| the views | this repo `site/v/` | `conversations.js` lays out two UI cells: `conv-list.js` (conv.list, needs `ai.session:list`) and `conv-thread.js` (conv.thread, needs `ai.session:read`) |
| the relay | this repo | `POST /api/cell/ai.session/<List|Get|Read>` on the /conversations/ face only, with a grant wire.ws-door mints for the viewer (their own `ai.session:*` rights), so ai.session's gate decides |
| the face | this repo | `127.0.0.1:8886=/conversations/` (APPS_SHELL_HTTP default) |
| the route | repo:ce-fleet `door/gate.py` | `conversations` -> 127.0.0.1:8886, in CELL_FACES (a gate without its own replica hops it to ops-1) |

The tab emerges: `/tabs.json` lists it for a person whose abilities cover `ai.session:list` while ai.session answers
`HealthOp` by name. A viewer without it gets no tab, 403 from the relay and 404 for the views. `ROOT` (Leif's
abilities when the people file does not name him) gains `ai.session:list` and `ai.session:read`.

## The view

Left (conv.list): the count, a search box (titles, folders, ids; ai.session `List{q}`), kind chips (All, Live, Lanes,
Workers, Sessions, Chats, Terminals; Live = state not Closed or Lost over the newest 1000), node chips, the cell's own
totals per source and per node, rows grouped Live, Today, Yesterday, Earlier (title, kind, node, messages, model, age).
Centre (conv.thread): the row's provider, account, node, kind, state, folder, ids; turns in pages of 50 (you,
assistant by model, tool), tool calls and results folded to one line each, thinking left out. On a phone one column
at a time with Back. Styles are a constructable stylesheet on the kit's tokens (CSP allows no inline style); the
modules hold no fetch, storage, cookie, URL or window.open (test).

## Proof (2026-11-21, foreman-ops-1; proof copies, test persons only, stopped after)

ai.session and claude.terminal (capped user units), apps-shell with `APPS_SHELL_FACES_ONLY=1` on 18886 with a proof
cell key (2 h grant of GRANT) and a test people file, and a stand-in for the sign-in gate signing `X-Ce-Person` for
`t-conv-owner` (`ai.session:list`, `ai.session:read`) and `t-conv-stranger` (`search.hybrid:find`). Headless
Chromium at 390 and 1280 px. Keys and grant destroyed after.

- owner: the Conversations tab; 7,329 conversations (ai.session 3, claude-code 7,284, lean-worker 25, tmux 17;
  debian 7,301, foreman-ops-1 28); foreman-ops-1 28 = 17 Claude Code session files + 11 tmux panes on that node;
  Live 29; this session (c8f41a49, 3,285 messages) opened and read in place on ops-1 in 0.4 s; no horizontal scroll
  at 1280.
- stranger: no Conversations tab; relay List 403; `/v/conversations.js` 404.
- A Debian session answers 502 "the transcript is on debian" until claude.terminal runs at Debian's door.
- Tests: `cargo test` here (tabs 19 incl. the Conversations rules, contract hash `0xb35024aa61e4992c` pinned, the
  relay passes List, Get and Read only), ce-net-net 10.

Not in this slice: talking (Send, Fork, Interrupt from the UI, conv.compose and conv.new are slice 3), live Events in
the view (it refreshes the list every 30 s), the Details column of the sketch.

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

Not in this slice: talking to headless sessions (Send, Fork, Interrupt from the UI, conv.compose and conv.new are slice 3), live Events in
the view (it refreshes the list every 30 s), the Details column of the sketch.

## tmux lanes live (2026-11-21, Leif ~09:00: "i must always be able to directly interact with all ai especially when its tmux")

A tmux row no longer shows "metadata only". It opens live through `term.session` (repo:term-session, schema
`7c4d50fa566fd6e8`) on the node that holds the pane (`term.session#<node>`), and the cell's answer to `Node` is
checked, so a dial that falls back to another node's cell is refused.
- conv.term (site/v/conv-term.js, served only to a viewer who holds `term.session:read`) shows the screen as it
  changes. `Watch` long-polls the cell's Attach stream and returns the first screen whose hash differs from the one
  the page shows. You can type a line (Keys: text, then Enter) and use the key row (Enter, Up, Down, Tab, Esc,
  Backspace, C-d) and Interrupt (op 12, Ctrl-C).
- conv.list (the entrypoint work's addition, kept): every pane that term.session List returns on foreman-ops-1,
  leif-desk and every node a row names is listed under "Live terminals". A pane opened after the last import shows up
  there too.
- Relay `POST /api/cell/term.session/<List|Read|Watch|Keys|Interrupt>` on /conversations/ only, as the viewer. The
  grant is minted by wire.ws-door from the viewer's own `term.session:*` rights (`read` to watch, `write` to type and
  interrupt). A `term.session:where:session=<name>` (or `name*`) ability rides along, and term.session itself filters
  List and answers 404 for any pane outside it. The body is typed (`TermReq`, unknown fields refused, at most 4096
  characters of text and 16 tmux key names). Open, Close, Freeze, Resize and OpenWith are never relayed.

Proof (foreman-ops-1, proof copies `c89c-*` units, test persons only, keys and grant destroyed after;
docs/term-proof/): throwaway pane `c89c-proof`.
- t-term-owner (read+write, sessions `c89c-proof*`), 1280 and 390 px: the pane opens, a loop typed in the tab prints
  `tick-N` lines that appear live (the screen differs 2 s apart), Interrupt stops it (`^C`, the screen stays still
  after). No horizontal scroll.
- t-term-watcher (read only): sees the pane live. Typing is refused (403) and the input row says why. Nothing reaches
  the pane.
- the owner opening the leif-cenet row (outside their sessions): "not there any more, or not open to you", no screen.
- t-term-rowonly (ai.session only): rows, no panes, /v/conv-term.js 404. t-term-stranger: no tab, module 404.
- curl: debian and leif-desk answer 503 "term.session is not running on <node>" (it is not installed there yet).
  Before the Node check, they showed ops-1's panes.

## Slice 3: talking from the tab (2026-11-21, foreman/win-c89c; repo:ce docs/AI-AS-CELLS-2026-11-18.md section 12)

Leif 2026-11-21: "i must always be able to directly interact with all ai".

- Relay `POST /api/cell/ai.session/<Op>` passes `AI_OPS` only (lib.rs), each under the cell's own right: List, Get,
  Read, Open, Send, Continue (op 17), Fork, Interrupt, Close, Ask (op 18). `Config*`, `Profile*`, `ImportRun` and
  `Rehome` are never passed from a page (404). `ai_check` runs before the call: Open takes the Bare harness only with
  no tool grants (a Code harness is a shell on the node), Send never waits (the answer comes over Events), parts
  are Text (at most 32 KB) or Attachment.
- `GET /api/cell/ai.session/Events?conversation=<id>`: server-sent events over one ce-wire stream opened as the viewer.
  Get first, so a row the viewer may not read is 403/404 before a stream opens; a comment every 15 s; ends on
  Closed, at stream end, or after 30 min. The kit's `host.events(op, req, on)` (site/_k/shell.js) is the module's
  one way to it (EventSource on the same origin; `connect-src 'self'` already allows it).
- UI: "New chat" (Open, Bare, claude.terminal) at the top of the list; under a conversation the message box (Send;
  Ctrl/Cmd+Enter), the answer written live (Delta), Interrupt while it answers, Fork (at the last turn: native), Close.
  An imported Claude Code row shows "Continue here" (Continue adopts it in place), then the same box.
- The copied ai.session contract is ai-session ce6eef1's whole block (schema `711b33ac59622d2b`, pinned by a test).
- The tab is now one screen high on desktop: the list and the turns scroll inside it, so the message box stays in
  view (it was below 300 rows). On a phone the message box sticks to the bottom.

Proof (foreman-ops-1, capped user units `c89c-*`: ai.session#c89c, ai.provider#claude.terminal.c89c, ai.account#c89c,
apps-shell faces-only on 18886 behind a stand-in gate; test persons only, stopped and their key destroyed after;
docs/slice3-proof/):
- t-c89c-chat (list, read, open, send, close, ask): New chat answered a poem ending kiwi-42, streamed live
  (02-live-delta); a second message in it answered kiwi-42 (04); a count to 2000 was interrupted mid-answer: the
  note says "Interrupted.", the row is Idle (05, 06); Fork at the last turn (native, `--fork-session`) answered
  kiwi-42 from the parent's context (07); the imported throwaway transcript a2a3e512 was continued in place and
  answered kiwi-77 from its own history (09). 390 px: no horizontal scroll (10).
- t-c89c-reader (list, read): New chat "not open to you", Send 403 (11).
- curl: Send{wait: true} from a page answered at once (wait forced false); Open with a Code harness 400; ConfigPut 404.

## Section 13: every node in the tab (2026-11-21, foreman/win-c89c; repo:ce docs/AI-AS-CELLS-2026-11-18.md 13.8, design by Fable 5.2)

- **Nodes block** (replaces the totals line for a viewer holding `ai.session:coverage`): `ai.session Coverage` as the
  viewer, at most once a minute. Summary "Nodes · k need a look" and "N sessions"; opened, one line per node: name,
  "listed of real" over the sources the node's importer counted, and the age of the last good import. Quiet when the
  numbers match and the import is fresh; otherwise words and the warn or bad colour (never colour alone): a gap
  ("7,200 of 7,298"), "rows, not counted, last import 3 d ago", "not answering", "importer refused", "no importer".
  Each node's sources are chips ("Claude Code 84 of 84", "tmux 10 of 10 live", "DeepSeek lanes 18 of 18"); a node
  line or a source chip filters the list on the server (`List{node, source}`).
- A viewer without `ai.session:coverage` (403) sees the old totals line (List with limit 1 per source and node).
- **Live panes**: `term.session List` on every node Coverage names that runs an importer, plus foreman-ops-1 and
  leif-desk, not a fixed pair.
- **Paging**: the footer always says "N of total"; "Load 300 older" calls `ListBefore{list: <same filter>, before_ms:
  <oldest row shown>}` and appends.
- Relay: `AI_OPS` adds `Coverage` (`ai.session:coverage`, takes no request: null or {} only) and `ListBefore`
  (`ai.session:list`). The contract is copied from repo:ai-session e923cd3 (schema 0xaf962657245182c4).
- Deploy: give Leif `ai.session:coverage` in `~/.config/apps-shell/people.json` (with list and read).
- Not built: the "Imported from opencode on <node>. Read only." line in the thread, and "no access to terminals" on a
  node line whose term.session refuses the viewer.

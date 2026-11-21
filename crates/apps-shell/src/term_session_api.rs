//! `term.session@1`'s contract, copied verbatim from repo:term-session `crates/term-session-api` (Node, the instance check), like
//! ai_usage_api: a different schema hash and the cell refuses every call as drift, so a test pins it. The
//! Conversations tab opens a tmux row live through it: List, Read, Attach (the screen as it changes), Keys and
//! Interrupt, on the node that holds the pane (`term.session#<node>`), as the viewer.
#![allow(dead_code)]

ce_wire::api! {
    api TermSession = "term.session", version 1;
    types {
        // Pane: id is tmux's pane id (`%3`), the handle every other op takes.
        pub struct Pane { pub id: String, pub session: String, pub window: u32, pub index: u32, pub cwd: String, pub cmd: String, pub width: u32, pub height: u32, pub active: bool }
        // OpenReq: a new detached session `name` in `cwd` running `argv` (empty argv = the login shell).
        pub struct OpenReq { pub name: String, pub cwd: String, pub argv: Vec<String> }
        // SendReq: literal = the text typed as-is; otherwise `keys` are tmux key names (Enter, C-c, Up ...). enter adds Enter.
        pub struct SendReq { pub pane: String, pub keys: Vec<String>, pub literal: bool, pub enter: bool }
        // ReadReq: the last `lines` lines of the pane, including scrollback (0 = the visible screen).
        pub struct ReadReq { pub pane: String, pub lines: u32 }
        pub struct Screen { pub pane: String, pub width: u32, pub height: u32, pub text: String }
        pub struct WatchReq { pub pane: String }
        // Row: line n (0 = top of the visible screen) now reads `text`.
        pub struct Row { pub n: u32, pub text: String }
        // Diff: the rows that changed since the last Diff; the first one carries every row. `rows` is the screen height.
        pub struct Diff { pub seq: u64, pub rows: u32, pub changed: Vec<Row> }
        // OpenSpec: Open plus an environment. `env` keys must be on the cell's allow-list (CLAUDE_CONFIG_DIR, LANG,
        // TERM_SESSION_ID); anything else is refused. `cols`/`rows` 0 = tmux default.
        pub struct OpenSpec { pub name: String, pub cwd: String, pub argv: Vec<String>, pub env: Vec<EnvVar>, pub cols: u32, pub rows: u32 }
        pub struct EnvVar { pub key: String, pub value: String }
        pub struct ResizeReq { pub pane: String, pub cols: u32, pub rows: u32 }
        // CloseReq: C-c, then kill the session after grace_ms if it is still there.
        pub struct CloseReq { pub pane: String, pub grace_ms: u32 }
        pub struct Closed { pub pane: String, pub existed: bool }
        // FreezeReq: `move` labels the freeze (the session.move id); Thaw needs the same label.
        pub struct FreezeReq { pub pane: String, pub move_id: String }
        // Frozen: idle = the screen was unchanged for 1.5 s and shows no running turn; screen_hash = sha256 of it.
        pub struct Frozen { pub pane: String, pub move_id: String, pub idle: bool, pub screen_hash: String }
        // ExpectReq: wait until the visible screen contains one of `any_of` (plain substrings).
        pub struct ExpectReq { pub pane: String, pub any_of: Vec<String>, pub timeout_ms: u32 }
        pub struct Matched { pub index: u32, pub screen: String }
        // AttachReq: a device viewing a pane: the screen each time it changes (Type goes through Keys).
        pub struct AttachReq { pub pane: String }
        pub struct Frame { pub seq: u64, pub width: u32, pub height: u32, pub text: String }
        // PaneReq: one pane (Interrupt).
        pub struct PaneReq { pub pane: String }
    }
    ops {
        1 => List(()) -> Vec<Pane>, needs "term.session:read";
        2 => Open(OpenReq) -> Pane, needs "term.session:write";
        3 => Keys(SendReq) -> (), needs "term.session:write";
        4 => Read(ReadReq) -> Screen, needs "term.session:read";
        6 => Resize(ResizeReq) -> (), needs "term.session:write";
        7 => Close(CloseReq) -> Closed, needs "term.session:close";
        8 => Freeze(FreezeReq) -> Frozen, needs "term.session:freeze";
        9 => Thaw(FreezeReq) -> (), needs "term.session:freeze";
        10 => Expect(ExpectReq) -> Matched, needs "term.session:read";
        11 => OpenWith(OpenSpec) -> Pane, needs "term.session:write";
        // Interrupt: Ctrl-C to the pane (what a viewer's Interrupt button sends); refused while the pane is frozen.
        12 => Interrupt(PaneReq) -> (), needs "term.session:write";
        // Node: the name of the node this cell runs on (its instance name). A caller that asked for
        // term.session#<node> checks it: a dial with no such instance falls back to any process of the bare api.
        13 => Node(()) -> String, needs "term.session:read";
    }
    streams {
        5 => Watch(WatchReq) -> stream Diff, needs "term.session:read";
        20 => Attach(AttachReq) -> stream Frame, needs "term.session:read";
    }
}

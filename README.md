# apps-shell

`apps.shell@1`, the root view of apps.ce-net.com (repo:ce/docs/SITES-ON-CELLS.md section 3). There is no tab list:
the tabs come from the cell systems of repo:ce-net-net `CE_NET` (each system's one `tab` line), and for each viewer a
tab is kept only when **allowed** (their abilities cover its one `needs` right) and **live** (its shell cell answered
HealthOp by name, local replica or another node, within 8 s; asked every 2 s). The page polls `tabs.json` every 3 s,
so a grant, an end, or a system's last replica going down changes the bar on an open page without a reload. A viewer
never receives a tab, view module or data route that is not theirs: the page and kit name no system, `v/<tab>.js` is a
404 to anyone whose tabs lack it, and the `api/*` routes answer only for a tab the viewer has.

Approvals is its own cell system (APPROVALS: grant.bundle, needs.list, feed.approve, approve.inbox), shown as the
`/approvals/` tab and as the Pending strip of the Core tab; Foreman is one tab among the others.

The gateway face (`$APPS_SHELL_HTTP`, default `127.0.0.1:8881=/approvals/,127.0.0.1:8882=/core/`) sits behind the
sign-in gate (`X-Ce-Person` signed with the gate's per-host key). Data calls are made with a **viewer grant** that
wire.ws-door mints for exactly that viewer's abilities in the data cells (bounded Mint: `GRANT` holds
`wire.ws-door:mint-for:approve.inbox|site.cellmap` and health rights only, no data right), so the data cell's own
gate and its row caveats (`approve.inbox:where:requester=..`) decide for the viewer. Who holds what: the people file
`~/.config/apps-shell/people.json` on the node (read per request; no file: leif holds `ROOT`).

`cargo test`: 6 (tabs per chain, a stopped shell takes its tab away, the people file fails closed, a call carries only
the viewer's own rights, every tab shell's health is granted). `apps-shell live` prints each tab shell's health.
The site (`site/`, ce-design tokens and components) is carried inside the binary. Apache-2.0.

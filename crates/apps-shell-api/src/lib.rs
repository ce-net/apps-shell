//! `apps.shell@1`: the root view of apps.ce-net.com (repo:ce/docs/SITES-ON-CELLS.md section 3). `Tabs` answers the
//! tabs the caller's chain may open whose shell cell answers now; the same rule its gateway face applies per request.
#![forbid(unsafe_code)]

ce_wire::api! {
    api AppsShell = "apps.shell", version 1;
    types {
        /// Health contract: these three fields, in this order, in every cell.
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        /// One tab: the system offering it, its title, public route and order. live_ms: when its shell last answered.
        pub struct TabRow { pub system: String, pub title: String, pub route: String, pub order: u32, pub live_ms: u64 }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "apps.shell:health";
        2 => Tabs(()) -> Vec<TabRow>, needs "apps.shell:tabs";
    }
}

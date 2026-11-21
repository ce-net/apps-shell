//! `ai.usage@1`'s contract, copied verbatim from repo:ai-usage `crates/ai-usage/crates/ai-usage-api` (major 1), like
//! ai_session_api in main.rs: a different schema hash and every call is refused by the cell as drift, so the test in
//! this crate pins `0x4a104f9ecd492dcd`. The conv.usage strip calls Read, Devices and Conversation, and Events.
//! The device chip and the strip read it as the viewer; rows hold account labels and key refs, never emails or keys.

ce_wire::api! {
    api AiUsage = "ai.usage", version 1;
    types {
        /// Health contract: these three fields, in this order, in every cell.
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        /// Where a shown value came from (ce-ref): kind msg, goal, commit, file, url, api or cell.
        pub struct Ref { pub label: String, pub kind: String, pub to: String }
        /// One subscription window of one account. `kind`: five_hour, seven_day, seven_day_<model> (fable, opus,
        /// sonnet), or gateway_day (sub.gateway asks per day). `offered` false: the plan has no such window.
        /// `stale_since`: the usage API refused and this is the last answer it gave.
        pub struct WindowRow {
            pub tenant: String, pub account: String, pub kind: String, pub pct: Option<f64>, pub resets_at: Option<String>,
            pub offered: Option<bool>, pub severity: Option<String>, pub locked_reason: Option<String>,
            pub sampled_at: Option<String>, pub stale_since: Option<String>, pub source: String
        }
        /// API usage and cost of one key in one month. `source`: provider (the provider's own usage/cost answer) or
        /// counted (our own turn counts times config/prices.toml). `key_ref` names the key in the keys cell.
        pub struct KeyRow {
            pub tenant: String, pub key_ref: String, pub provider: String, pub month: String,
            pub tokens_in: u64, pub tokens_out: u64, pub cache_read: u64, pub cache_write: u64,
            pub cost_usd: f64, pub month_spend: Option<f64>, pub limit: Option<f64>, pub source: String,
            pub sampled_at: String, pub why: Option<String>
        }
        pub struct Battery { pub pct: Option<f64>, pub charging: Option<bool> }
        /// An AI CLI on a device: claude, codex, gemini, opencode; `version` from `--version`.
        pub struct Cli { pub name: String, pub version: Option<String> }
        /// What one device can do now (device.registry). `logins`: account labels whose credentials are present
        /// (presence only). `cells_installed`: the usage-relevant cells, `cells_total` all of them.
        pub struct DeviceRow {
            pub device_id: String, pub name: String, pub class: String, pub os: String, pub arch: String,
            pub online: bool, pub battery: Option<Battery>, pub clis: Vec<Cli>, pub logins: Vec<String>,
            pub cells_installed: Vec<String>, pub cells_total: u32, pub last_seen: String, pub tenant: String
        }
        /// One turn's usage (ai.session Turn{usage}, or a lane's transcript counts via usage.budget LaneTokens).
        pub struct TurnUsage {
            pub tenant: String, pub conversation: String, pub n: u32, pub provider: String, pub account: String,
            pub device: String, pub model: String, pub input: u64, pub output: u64, pub cache_read: u64,
            pub cache_write: u64, pub cost_usd: Option<f64>, pub wall_ms: u64, pub at: String, pub source: String
        }
        /// A conversation's totals.
        pub struct ConvRow {
            pub tenant: String, pub conversation: String, pub provider: String, pub account: String, pub device: String,
            pub model: String, pub turns: u32, pub input: u64, pub output: u64, pub cache_read: u64, pub cache_write: u64,
            pub cost_usd: f64, pub wall_ms: u64, pub first_at: String, pub last_at: String
        }
        /// One device's totals for one day.
        pub struct DeviceDay {
            pub tenant: String, pub device: String, pub day: String, pub turns: u32, pub input: u64, pub output: u64,
            pub cache_read: u64, pub cache_write: u64, pub cost_usd: f64
        }
        /// A usage event, raised once (ai.usage.sub): a window crossed 80, 90 or 100 %, a window reset, an account
        /// lapsed (payment past due).
        pub enum Event {
            Threshold { account: String, kind: String, pct: u32, resets_at: Option<String> },
            Reset { account: String, kind: String },
            Lapsed { account: String, why: String }
        }
        pub struct EventRow { pub at: String, pub tenant: String, pub event: Event, pub text: String }
        /// One account as the front door shows it: the ai.account row plus its windows.
        pub struct AccountLine {
            pub label: String, pub provider: String, pub plan: String, pub plan_price_month: Option<f64>,
            pub payment: String, pub state: String, pub windows: Vec<WindowRow>, pub logged_in_on: Vec<String>,
            pub last_sample: Option<String>
        }
        pub struct Totals {
            pub turns: u64, pub conversations: u64, pub input: u64, pub output: u64, pub cache_read: u64,
            pub cache_write: u64, pub counted_usd: f64, pub keys_usd: f64, pub subscription_usd: f64, pub total_usd: f64
        }
        /// `scope`: all, accounts, keys, devices.
        pub struct ReadReq { pub tenant: String, pub scope: String }
        pub struct Report {
            pub tenant: String, pub at: String, pub accounts: Vec<AccountLine>, pub keys: Vec<KeyRow>,
            pub devices: Vec<DeviceRow>, pub today: Totals, pub month: Totals, pub alerts: Vec<EventRow>,
            pub refs: Vec<Ref>
        }
        pub struct WindowsReq { pub account: Option<String> }
        pub struct TenantReq { pub tenant: String }
        pub struct ConvReq { pub id: String }
        /// `period`: today, month (this month), or yyyymm.
        pub struct RollupReq { pub tenant: String, pub period: String }
        pub struct Rollup {
            pub tenant: String, pub period: String, pub totals: Totals, pub accounts: u32, pub keys: u32,
            pub devices: u32, pub by_provider: Vec<ProviderSpend>, pub refs: Vec<Ref>
        }
        pub struct ProviderSpend { pub provider: String, pub usd: f64, pub tokens: u64, pub source: String }
        /// Events after `since` (an EventRow.at; None: the last 20), then every new one as it is written.
        pub struct EventsReq { pub tenant: String, pub since: Option<String> }
        pub enum UsageError { Invalid(String), Store(String) }
        pub struct SpawnReq { pub node: String, pub n: u32 }
        pub struct SpawnResp { pub ids: Vec<String> }
        pub struct InstancesReq { pub node: String }
        pub struct InstancesResp { pub ids: Vec<String> }
        pub struct StopIdReq { pub id: String }
        pub struct StopIdResp { pub stopped: bool }
        pub struct DeployReq { pub node: String, pub version: String }
        pub struct DeployResp { pub ok: bool, pub version: String }
        pub struct BenchReq { pub op: String, pub n: u32 }
        pub struct BenchResp { pub p50_ms: f64, pub p95_ms: f64, pub errors: u32 }
        pub enum CellOpsError { Invalid(String), Failed(String) }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "ai.usage:health";
        2 => Read(ReadReq) -> Result<Report, UsageError>, needs "ai.usage:read";
        3 => Windows(WindowsReq) -> Result<Vec<WindowRow>, UsageError>, needs "ai.usage:read";
        4 => Keys(TenantReq) -> Result<Vec<KeyRow>, UsageError>, needs "ai.usage:read";
        5 => Devices(TenantReq) -> Result<Vec<DeviceRow>, UsageError>, needs "ai.usage:read";
        6 => Conversation(ConvReq) -> Result<Option<ConvRow>, UsageError>, needs "ai.usage:read";
        7 => RollupOp(RollupReq) -> Result<Rollup, UsageError>, needs "ai.usage:read";
        900 => CellSpawn(SpawnReq) -> Result<SpawnResp, CellOpsError>, needs "ai.usage:spawn";
        901 => CellInstances(InstancesReq) -> Result<InstancesResp, CellOpsError>, needs "ai.usage:instances";
        902 => CellStop(StopIdReq) -> Result<StopIdResp, CellOpsError>, needs "ai.usage:stop";
        903 => CellDeploy(DeployReq) -> Result<DeployResp, CellOpsError>, needs "ai.usage:deploy";
        904 => CellBench(BenchReq) -> Result<BenchResp, CellOpsError>, needs "ai.usage:bench";
    }
    streams {
        10 => Events(EventsReq) -> stream EventRow, needs "ai.usage:read";
    }
}

/// The state.store app every ai.usage cell keeps its rows in.
pub const APP: &str = "ai-usage";

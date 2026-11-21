//! The generated TypeScript of the usage strip cannot drift from its Rust types (gen/ConvUsage.d.ts, written by
//! `ce-wire-gen --lang ts-types src/strip.rs -o gen --api ConvUsage`, the one in ~/.local/bin):
//! 1. its header names the schema hash of the api! block as compiled here (no tool needed);
//! 2. when `ce-wire-gen` is on PATH (always on foreman-ops-1, set CE_REQUIRE_WIRE_GEN=1 in CI) a fresh generation is
//!    byte for byte the committed file.
//! Also pins ai.usage@1's copied contract (a changed hash is refused by the cell as drift).
use ce_wire::Api;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn generated_types_carry_the_compiled_schema_hash() {
    let ts = std::fs::read_to_string(root().join("gen/ConvUsage.d.ts")).unwrap();
    let want = format!("schema 0x{:016x}", <apps_shell::strip::ConvUsage as Api>::SCHEMA);
    assert!(ts.lines().next().unwrap().contains(&want), "gen/ConvUsage.d.ts is stale: regenerate it ({want})");
}

#[test]
fn a_fresh_generation_is_the_committed_file() {
    // CE_WIRE_GEN names the tool (cargo puts ~/.cargo/bin first on PATH, where an older copy may be)
    let bin = std::env::var("CE_WIRE_GEN").unwrap_or_else(|_| "ce-wire-gen".into());
    let tool = std::process::Command::new(&bin).arg("--list").arg(root().join("src/strip.rs")).output();
    let Ok(out) = tool.as_ref() else {
        assert!(std::env::var("CE_REQUIRE_WIRE_GEN").is_err(), "ce-wire-gen is required here and is not on PATH");
        eprintln!("ce-wire-gen not on PATH: only the hash check ran");
        return;
    };
    assert!(out.status.success(), "ce-wire-gen --list failed");
    let dir = std::env::temp_dir().join(format!("conv-usage-gen-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let r = std::process::Command::new(&bin)
        .args(["--lang", "ts-types"]).arg(root().join("src/strip.rs")).arg("-o").arg(&dir).args(["--api", "ConvUsage"])
        .output().unwrap();
    if !r.status.success() && String::from_utf8_lossy(&r.stderr).contains("expected py, ts, ts-web, swift or json") {
        assert!(std::env::var("CE_REQUIRE_WIRE_GEN").is_err(), "this ce-wire-gen has no ts-types: set CE_WIRE_GEN to a newer one");
        eprintln!("this ce-wire-gen is too old for ts-types: only the hash check ran");
        return;
    }
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let fresh = std::fs::read_to_string(dir.join("ConvUsage.d.ts")).unwrap();
    let kept = std::fs::read_to_string(root().join("gen/ConvUsage.d.ts")).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(fresh, kept, "gen/ConvUsage.d.ts drifted from src/strip.rs: ce-wire-gen --lang ts-types src/strip.rs -o gen --api ConvUsage");
}

#[test]
fn the_copied_ai_usage_contract_is_the_cells() {
    assert_eq!(<apps_shell::ai_usage_api::AiUsage as Api>::SCHEMA, 0x4a104f9ecd492dcd);
    let ops: Vec<&str> = apps_shell::ai_usage_api::AiUsage::OPS.iter().map(|o| o.name).collect();
    for op in ["Read", "Conversation", "Devices"] {
        assert!(ops.contains(&op), "{op}");
    }
}

//! The Approvals tab's cards: decision.card@1 copied verbatim (schema pinned), a live board (foreman-ops-1 and Debian,
//! 2026-11-21, addresses replaced) reads into the typed Board, the shell may mint for decision.card and holds no data
//! right of it, and the view module asks only its own ops.
use apps_shell::decision_card_api::{Board, DecisionCard};
use apps_shell::*;
use ce_wire::Api;

#[test]
fn contract_is_decision_card_v1() {
    assert_eq!(<DecisionCard as Api>::SCHEMA, 0xd059ebf54034fc22);
    let ops: Vec<&str> = DecisionCard::OPS.iter().map(|o| o.name).collect();
    assert_eq!(ops, ["HealthOp", "BoardOp", "Receipts", "Sources"]);
}

#[test]
fn a_live_board_reads() {
    let b: Board = serde_json::from_str(include_str!("fixtures/decision-board.json")).unwrap();
    assert_eq!(b.cards.len(), 5);
    assert_eq!(b.cards[0].kind, "policy");
    assert!(b.cards.iter().all(|c| (2..=3).contains(&c.options.len()) && (c.recommended as usize) < c.options.len()));
    assert!(b.closed >= 33);
}

#[test]
fn the_shell_mints_for_decision_card_and_reads_nothing_itself() {
    assert!(GRANT.contains(&"wire.ws-door:mint-for:decision.card") && DATA_CELLS.contains(&"decision.card"));
    assert!(!GRANT.iter().any(|g| g.starts_with("decision.card:") && *g != "decision.card:health"));
    assert!(ROOT.contains(&"decision.card:read"));
    let js = include_str!("../../../site/v/approvals.js");
    assert!(js.contains("host.call('decisions')") && !js.contains("fetch("));
}

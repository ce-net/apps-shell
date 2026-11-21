//! `decision.card@1`'s contract, copied verbatim from repo:decision-card `crates/decision-card-api` at c89cf86 (major
//! 1), like ai_usage_api: a different schema hash and every call is refused by the cell as drift, so the test in this
//! crate pins `0xd059ebf54034fc22`. The Approvals tab reads `BoardOp` as the viewer: the cards the owner must answer
//! (repo:ce/docs/CRITICAL-DECISIONS-2026-11-21.md section 8), in place of the inbox's row list.

ce_wire::api! {
    api DecisionCard = "decision.card", version 1;
    types {
        /// Health contract: these three fields, in this order, in every cell.
        pub struct Health { pub ok: bool, pub version: String, pub uptime_s: u64 }
        /// An ask's stable id: the source cell and the source's own id.
        pub struct AskId { pub source: String, pub id: String }
        /// Money leaves: amount in minor units, ISO currency, who is paid, what for (as the source record states it).
        pub struct Spend { pub minor: u64, pub currency: String, pub payee: String, pub what: String }
        pub enum Act { Promise, Contract, Signature, Tax, Filing }
        /// A binding act with a party; `what` the term as the source record states it.
        pub struct Bind { pub act: Act, pub party: String, pub what: String }
        /// A message to a person. `outside`: a real person outside the system (not a lane, cell or test person).
        pub struct Tell { pub person: String, pub channel: String, pub outside: bool }
        /// Something is lost. `recover_s` None: it cannot be undone.
        pub struct Destroy { pub what: String, pub recover_s: Option<u64>, pub bytes: u64 }
        /// A capability grant. `until` 0: no end stated.
        pub struct Grant { pub owner: String, pub scope: Vec<String>, pub to: String, pub node: String, pub until: u64, pub revocable: bool }
        /// The typed facts of an ask: the only input to its class.
        pub struct Facts { pub spend: Option<Spend>, pub binds: Option<Bind>, pub tells: Option<Tell>, pub destroys: Option<Destroy>, pub grants: Option<Grant> }
        /// One candidate ask. `title` and `text` are the asker's prose: shown, never signed, never move class or rank.
        /// `fingerprint`: hash of what it would do (owner cell, kind, scope, receiver, node); `answered` an ask with
        /// that fingerprint the owner already signed (found by the collector), so this one is superseded.
        pub struct Ask {
            pub id: AskId, pub asker: String, pub title: String, pub text: String, pub facts: Facts,
            pub deadline: Option<u64>, pub blocks: Vec<String>, pub fingerprint: String, pub dry_run: bool,
            pub at: u64, pub src: String, pub sign_at: String, pub answered: Option<AskId>
        }
        pub enum Class { Money, Law, Message, Delete, Reversible, Unclassified }
        pub enum DropWhy { DryRun, Duplicate(AskId), Superseded(AskId), Expired, Returned(String) }
        /// Card: the owner decides. Wait: a reversible ask that a standing rule (`policy`) covers once signed; until
        /// then it is not applied. Auto: a signed rule decided it. Drop: closed with a receipt.
        pub enum Outcome { Card(Class), Wait { policy: String }, Auto { policy: String, allow: bool }, Drop(DropWhy) }
        /// One triage result, hash chained (`prev` is the previous receipt's `hash`).
        pub struct Receipt { pub ask: AskId, pub outcome: Outcome, pub rule: String, pub at: u64, pub prev: String, pub hash: String }
        /// score = impact x urgency x (1 + blocked); every rank carries its factors.
        pub struct Rank { pub impact: u32, pub urgency: u32, pub blocked: u32, pub score: u32 }
        /// One option. `signs`: the exact text a passkey signs, generated from the typed facts.
        pub struct Opt { pub label: String, pub signs: String, pub standing: bool }
        /// What happens if there is no answer by `by`: always a non-action.
        pub struct NoAnswer { pub by: u64, pub then: String, pub cost: String }
        /// One card. `oldest`: when its oldest ask was made. `kind` is "decision" (one of the four classes) or "policy" (sign a standing rule once).
        pub struct Card {
            pub id: String, pub kind: String, pub class: Class, pub question: String, pub options: Vec<Opt>,
            pub recommended: u32, pub because: String, pub no_answer: NoAnswer, pub rank: Rank,
            pub asks: Vec<AskId>, pub lines: Vec<String>, pub src: Vec<String>, pub sign_at: String, pub oldest: u64
        }
        pub enum SourceState { Live, Stale { age_s: u64 }, Down { why: String } }
        pub struct Source { pub cell: String, pub node: String, pub state: SourceState, pub ms: u32, pub asks: u32 }
        /// The board: the top `shown` cards, how many more, what closed, and where every ask came from.
        pub struct Board {
            pub cards: Vec<Card>, pub more: u32, pub asks: u32, pub closed: u32, pub waiting_on_rule: u32,
            pub returned: u32, pub sources: Vec<Source>, pub not_connected: Vec<String>, pub as_of_ms: u64
        }
        pub struct ReceiptsReq { pub limit: u32 }
    }
    ops {
        1 => HealthOp(()) -> Health, needs "decision.card:health";
        2 => BoardOp(()) -> Board, needs "decision.card:read", deadline_ms 15000;
        3 => Receipts(ReceiptsReq) -> Vec<Receipt>, needs "decision.card:read";
        4 => Sources(()) -> Vec<Source>, needs "decision.card:read";
    }
}

# Decisions in the Approvals tab (apps.ce-net.com/approvals/)

Slice 1 of repo:ce/docs/CRITICAL-DECISIONS-2026-11-21.md section 8. The Approvals tab now opens on decision.card's
board (repo:decision-card) instead of approve.inbox's row list: at most 5 cards, ranked impact x urgency x (1 +
blocked), each with its options, the recommended one, why, and what happens without an answer (always a hold); one
line for what closed with a receipt, one for the sources, one for what is not collected yet. "Every waiting row"
(`?all=1`) keeps the old list and `?b=<bundle>` the old card. The badge counts the cards, not the rows.

- `GET /api/decisions`: decision.card `BoardOp` as the viewer (a grant wire.ws-door mints, `decision.card:read` and
  their `decision.card:where:owner=` caveat), so decision.card's gate decides. No `decision.card:read`: 403 and the
  view falls back to the old list; the badge to the row count.
- The contract is copied verbatim (src/decision_card_api.rs, schema 0xd059ebf54034fc22 pinned in tests/decisions.rs).
- GRANT gains `wire.ws-door:mint-for:decision.card`, `decision.card:health` (reissue with ce-iam grant); ROOT and the
  owner's people-file row gain `decision.card:read`.
- Signing is not here: each card links to where its asks are signed today (the grant card, the Foreman approvals
  page). One passkey tap on the card itself is decision.answer, after a security review.

Proof: docs/decisions-proof/decisions-1440.png and -390.png, the live board of 2026-11-21 (foreman-ops-1 and Debian,
addresses replaced) painted by site/v/approvals.js in Chromium through docs/decisions-proof/harness.html.

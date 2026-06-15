# Phase 4: Conditional live promotion

## Status

Not started.

## Goal

Promote AI 1.1 to the live lobby default only if Phase 3 replay evidence shows that AI 1.1 outperforms AI 1.0 and the handoff explicitly recommends promotion.

## Scope

- Read the Phase 3 handoff and replay result before making any code changes.
- If Phase 3 does not recommend promotion, do not change live defaults. Mark this phase done as a no-promotion documentation phase after recording why AI 1.0 remains live.
- If Phase 3 recommends promotion, update `server/crates/ai/src/live.rs` so new live AI controllers use AI 1.1 by default.
- If promotion proceeds, prefer a singular live pool containing AI 1.1 unless the user explicitly asks for mixed live AIs.
- If promotion proceeds, update profile fallback behavior so unknown live profile ids resolve to the promoted AI 1.1 default.
- If promotion proceeds, update lobby-facing labels that hard-code "AI 1.0".
- If promotion proceeds, update developer aliases/help text only where appropriate. Keep explicit AI 1.0 aliases working.
- Update `docs/design/ai.md` either way: promoted AI 1.1 if evidence supports it, or AI 1.1 retained as a developer-only comparison profile if it does not.

## Behavioral requirements

- Do not add a lobby protocol field or UI selector for AI profile choice.
- Existing host `addAi` behavior must remain unchanged from the client perspective.
- Developer tooling must still allow `ai_1_0_tech` vs `ai_1_1_tank_mg` comparisons.
- Do not promote on hope, intuition, or short-run behavior. Promotion requires the Phase 3 30,000 tick release replay evidence.
- If lobby text changes to "AI 1.1", make sure tests or snapshots that assert the old label are updated intentionally.

## Expected touch points

Promotion path:

- `server/crates/ai/src/live.rs`
- `server/crates/ai/src/selfplay/replay.rs`
- `server/crates/ai/src/tools/matchup.rs`
- `client/src/lobby_view.js`
- `tests/ai_integration.mjs` or focused client/lobby tests if they assert the label
- `docs/design/ai.md`

No-promotion path:

- `plans/ai1.1/phase-4.md`
- `docs/design/ai.md` only if Phase 3 changed the documented status

## Verification

Promotion path:

- `cargo test -p rts-ai live_profile`
- `cargo run --bin ai-matchup -- --list-profiles`
- A focused lobby/client test if an existing one covers AI seat labels.

No-promotion path:

- Docs-only review of the Phase 3 evidence and this phase document.

## Manual testing focus

Promotion path:

- Start a local server, add an AI from the lobby, and confirm the seat label matches the promoted AI 1.1 wording.
- Start a match with one human and one AI and confirm no lobby/profile-selection workflow changed.
- Confirm `ai-matchup` can still run both `ai_1_1_tank_mg ai_1_0_tech` and `ai_1_0_tech ai_1_1_tank_mg`.

No-promotion path:

- Confirm AI 1.0 remains the live label/default and AI 1.1 remains available only through developer matchup tooling.

## Handoff

After implementation, mark this phase done and summarize whether promotion happened. If it happened, include the live default id, fallback/alias behavior, tests run, and user-visible label changes. If it did not happen, include the Phase 3 evidence that blocked promotion and the next behavior changes that should be considered before retesting.

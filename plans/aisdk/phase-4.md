# Phase 4 - Prove the Authoring Path with a Real Strategy

## Phase Status

- [x] Done.

## Objective

Use the public Phase 3 SDK to write one small but real deterministic strategy, then fix only the
first-use problems that strategy demonstrates. The result must be useful as executable documentation
for an AI author: it should run through the canonical live runtime, make ordinary economic and
combat decisions, and use no private AI modules, raw protocol DTOs, or `SimCommand` construction.

This replaces the former observational task-feedback phase. We do not yet have a sound meaning for
"this command completed": owner-visible state can show a postcondition without proving which command
caused it, and missing evidence cannot reliably distinguish rejection, delay, destruction, fog, or
supersession. Stable intent IDs and statuses would therefore harden an attractive but misleading API.

## Work

- Add a compact reference strategy in an outside-crate integration-test/example location using only
  public `rts_ai::sdk` imports and the supported custom-strategy constructor.
- Give it enough policy to exercise the real lifecycle: inspect owned units and economy, issue at
  least one economic action and one tactical action, keep small cross-tick memory, and tolerate
  unavailable candidates without panicking.
- Run it through the same canonical controller driver and ordinary `Game::enqueue` validation used
  by Jeff and AI 2.1. Do not build a special example-only host.
- Add a short author guide showing the strategy lifecycle, frame knowledge boundaries, deterministic
  ordering, action emission, and how to run the example/test.
- Fix SDK naming, visibility, ergonomics, or missing small accessors only when the reference strategy
  provides a concrete call site. Record larger missing capabilities for Phases 5 and 6 instead of
  inventing them here.
- Keep the example intentionally ordinary. It is a usability specimen, not a new competitive AI,
  a framework, or a benchmark claim.

## Expected Touch Points

- `server/crates/ai/tests/` and/or `server/crates/ai/examples/`.
- Small focused changes under `server/crates/ai/src/sdk/` if first-use friction proves them necessary.
- `docs/design/ai.md` and a concise AI-author guide linked from it.

Do not touch `rts-sim`, wire protocol mirrors, client code, balance, lobby scheduling, Jeff policy,
or AI 2.1 policy.

## Implementation Checklist

- [x] Add a nontrivial public-SDK-only reference strategy.
- [x] Run it through the canonical production runtime and ordinary simulation validation.
- [x] Cover lifecycle, cross-tick memory, economic action, and tactical action.
- [x] Add a concise runnable author guide.
- [x] Fix only demonstrated Phase 3 API friction.
- [x] List concrete rule/query and action-helper gaps for Phases 5 and 6.
- [x] Pass the unchanged Phase 1 Jeff transcript.
- [x] Mark this phase done in the implementation commit.

## Verification

- Compile the reference consumer outside private `rts_ai` module visibility.
- Prove initialization occurs once and steps occur only on canonical decision ticks.
- Run a deterministic reference-strategy matchup twice and compare ordered command logs.
- Verify every command reaches normal simulation validation and replay logging.
- Run focused `rts-ai` tests, strict clippy, crate/simulation architecture checks, docs health, and
  the exact Phase 1 normal/full transcript without fixture regeneration.

## Non-Goals

- No command acceptance, completion, rejection, or causal task status.
- No intent IDs, task ledger, timeouts, idempotent goals, scheduler, behavior tree, or GOAP.
- No rulebook extraction, placement/path query suite, or public budget/reservation planner; concrete
  needs for those surfaces are inputs to Phases 5 and 6.
- No new player-selectable AI unless separately justified by the user.
- No policy-strength claim or broad SDK stability promise.

## Handoff Expectations

Report the public imports used, the complete authoring flow, commands exercised, first-use SDK fixes,
determinism/parity evidence, and a prioritized list of specific missing rule/query/action operations.
Phase 5 must use that list to bound its surface rather than implementing a catalog speculatively.

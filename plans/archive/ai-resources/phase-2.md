# Phase 2 - Availability-Driven Economy Intent

Status: Done.

## Goal

Move AI economy intent onto the resource availability model so worker targets are based on
currently mineable resources and impossible oil targets do not block available steel work.

## Scope

- Update `EconomyPlan` construction in `server/crates/ai/src/ai_core/decision/resources.rs` to
  consume the Phase 1 availability helpers.
- Compute steel saturation targets from mineable, remaining steel nodes rather than raw known steel
  lists.
- Make desired oil workers depend on free mineable oil availability:
  - normal profile oil demand should be zero when no free mineable oil exists
  - panic support oil demand should also avoid impossible oil unless a phase explicitly documents a
    different emergency policy
  - when oil becomes mineable after a City Centre completes, existing profile oil targets should
    resume without extra tuning
- Keep post-expansion local-assignment bounds, but apply them to mineable candidate nodes rather
  than using them as a substitute for mineability.
- Preserve expansion planning's use of known non-main resources in
  `server/crates/ai/src/ai_core/decision/expansion.rs`. Do not accidentally filter expansion
  candidates down to currently mineable nodes.
- Add compact economy plan fields or trace labels if needed so tests and debugging can distinguish
  "wants no oil because none is mineable" from "wants no oil because steel floor is unmet."

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/resources.rs`
- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/decision/trace.rs` if economy blockers/readouts change
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/ai_core/facts.rs` or the Phase 1 availability module

## Behavioral Requirements

- In a pre-expansion observation with free mineable steel and no free mineable oil, the economy plan
  should target steel work and report zero desired oil workers.
- In a post-expansion observation with completed City Centre coverage over oil, the economy plan
  should allow profile oil targets again.
- If current steel workers are below the profile's steel floor, desired oil workers should remain
  zero for the existing reason.
- If expansion policy says oil-before-steel after the target City Centre count is complete, that
  priority should apply only to mineable oil.
- The change should not alter production, combat, or expansion build decisions except through
  resource availability changing economy worker intent.

## Verification

- Add focused decision tests for:
  - pre-expansion free steel plus non-mineable oil yields steel intent and no oil intent
  - completed expansion oil yields oil intent when profile thresholds are met
  - incomplete expansion City Centre does not make oil mineable
  - expansion candidate discovery still finds future non-main resource clusters
- Run targeted tests such as:

```bash
cd server
cargo test -p rts-ai economy
cargo test -p rts-ai expansion_candidate
```

## Manual Testing Focus

In a local self-play or replay inspection around the 20-30 supply window, confirm newly trained AI
workers are sent to home steel when no mineable oil exists. After the expansion City Centre
completes, confirm workers can be assigned to oil if the oil is in range.

## Handoff

After implementation, mark this phase done and summarize which economy targets now use
availability, which trace labels or plan fields explain oil suppression, and the tests run. Tell
Phase 3 whether any assignment path still accepts raw `observation.resources` without an
availability filter.

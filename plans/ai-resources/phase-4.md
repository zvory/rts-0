# Phase 4 - Regression Scenarios and Documentation

Status: Done.

## Goal

Prove the new resource availability architecture fixes the idle-worker class and document the
resource contract for future AI work.

## Scope

- Add or extend deterministic AI scenario coverage for the reproduced bug class:
  - one completed starting City Centre
  - supply in the low-to-mid 20s
  - free mineable steel nodes
  - oil nodes known but not currently mineable
  - idle worker near the City Centre
  - expected result: worker receives valid steel gather intent, not invalid oil
- Add a complementary expansion scenario where oil becomes mineable only after the expansion City
  Centre completes and workers can then be assigned to oil.
- Add matchup or scenario scorecard/readout fields only if they help preserve this regression
  signal without bloating normal output.
- Update `docs/design/ai.md` to describe the resource availability model, the known-vs-mineable
  distinction, and the guarantee that economy assignment only targets currently mineable nodes.
- Update `docs/context/server-sim.md` only if the design section list or AI pointers need to change.
- Optionally add a short developer note under this plan directory with the exact replay/probe
  command used to inspect the original class, if a stable command exists after implementation.

## Expected Touch Points

- `server/crates/ai/src/selfplay/scenarios.rs`
- `server/crates/ai/src/selfplay/tests.rs`
- `server/crates/ai/src/selfplay/milestones.rs` or scorecard code if new metrics are needed
- `server/crates/ai/src/tools/matchup.rs` only if a developer-facing scenario selector/readout is
  added
- `docs/design/ai.md`
- `docs/context/server-sim.md` if section pointers shift
- This phase document, marked done with results

## Behavioral Requirements

- The regression scenario must fail on the old class of behavior: assigning an idle worker to
  known but non-mineable oil while free mineable steel exists.
- The post-expansion scenario must prove the architecture does not permanently suppress oil.
- Profile-backed AI 1.0/AI 1.1 progression should still reach the normal tech economy in a short
  or bounded run appropriate for development.
- Do not promote, retune, or relabel AI profiles in this phase.

## Verification

- Run focused AI scenario tests added in this phase.
- Run a short profile-backed matchup or scenario command that covers ordinary economy progression,
  for example:

```bash
cd server
cargo run -p rts-ai --bin ai-matchup -- ai_1_0_tech ai_1_0_tech --ticks 6000 --seed 7 --json
```

- If the phase changes profile-backed harness behavior or scenario scorecards, consider:

```bash
cd server
RTS_FULL_AI_TESTS=1 cargo test -p rts-ai
```

Use the full AI gate only when the phase's actual changes justify the longer run.

## Manual Testing Focus

Open or inspect a local replay/self-play run around the original failure window. Confirm workers
spawned near the City Centre move to available steel before expansion, and later oil assignment
begins only after a completed City Centre covers oil.

## Handoff

After implementation, mark this phase done and summarize the regression scenario, the replay or
matchup evidence, docs updated, and any remaining economy risks to watch in playtests. Include
factual gameplay patch notes describing the worker-idle fix and any observed AI economy timing
changes.

## Completed Handoff

- Added profile-backed self-play regression coverage for the low-to-mid supply pre-expansion case:
  while free steel is mineable and known oil is not covered by a completed City Centre, player 1's
  gather command targets steel and any oil gather command must target a currently mineable node.
- Added a complementary profile-backed self-play regression where AI 1.0 completes an expansion
  City Centre, oil becomes mineable, and a later worker assignment targets oil.
- Updated `docs/design/ai.md` with the AI-owned resource availability contract, the
  known-vs-mineable distinction, and the action-layer assignable-node guard.
- Targeted verification: `cargo test -p rts-ai profile_backed_ai_` passed.
- Profile-backed checkup: `cargo run -p rts-ai --bin ai-matchup -- ai_1_0_tech ai_1_0_tech --ticks 6000 --seed 7 --json` passed after implementation.
- Gameplay patch note: AI workers now have regression coverage proving they prefer currently
  mineable resources and do not spend the pre-expansion worker opportunity on known but
  non-mineable oil; post-expansion oil assignment still occurs after City Centre coverage exists.

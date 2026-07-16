# Phase 3 - Playtest and Live Cutover

## Phase Status

- [ ] Not started.

## Depends On

- Phase 2 merged with crude coverage for existing gameplay-significant world and feedback records.
- The user completed the Phase 2 Babylon live-player playtest, reported any blockers, and explicitly
  approved Phase 3 blocker fixes and default cutover.

## Objective

Use the user's live-play evidence to repair only observed play blockers, prove the default route,
and make Babylon the live-player renderer with a simple rollback path.

## Work

- Record the user's Phase 2 live-player results covering camera/navigation, mixed-force
  identification, selection/marquee, economy, construction, production, rallying, mining,
  movement, attack, one targeted ability or support weapon, combat across terrain/fog, and
  leave/re-enter. Fix only reported issues that prevent or materially confuse those flows;
  primitive visuals and lower fidelity are not blockers.
- Add a required browser canary that opens a no-selector normal live route, proves Babylon was
  selected, and fails on dependency, page, frame, render, interaction, or teardown errors. Add
  focused selector checks proving explicit `rtsRenderer=pixi` and replay/spectator routes choose
  Pixi without requesting Babylon.
- After the playtest passes, resolve and load the default backend late enough that normal
  live-player and Lab routes choose Babylon while replay/spectator do not load or depend on it.
  Keep an explicit Pixi selector as rollback.
- Update the parity ledger and durable renderer docs to record the actual cutover and remaining
  fallback routes; mark this phase done so the normal PR workflow archives the plan.

## Expected Touch Points

- renderer selection and live/Lab route defaults
- blocker-specific Babylon presentation code only
- Babylon live browser canary and existing focused contracts
- renderer selection/rendering design docs, parity ledger, and this phase status

## Acceptance

- The user-approved live-match playtest is playable without consulting Pixi and produced no
  unresolved page, frame, renderer, or teardown error.
- Babylon is the normal live-player/Lab renderer; explicit Pixi rollback and Pixi
  replay/spectator fallback remain functional without loading Babylon.
- No visual-polish or speculative infrastructure work is pulled into the cutover.

## Verification and Manual Test

Run the required no-selector Babylon live browser canary, selector/fallback contracts, existing
two-recipient secrecy and kernel contracts, the client architecture check, and the focused checks
for any blocker fixes. Repeat the real live-player flow after the default changes and verify
explicit Pixi rollback, replay/spectator without Babylon loading, and one leave/re-enter cycle.

## Handoff

Report the playtest, blockers fixed, default/rollback behavior, checks, and player-facing visual
limitations. State that future renderer work should improve Babylon directly from observed needs;
do not propose another parity or polish phase automatically.

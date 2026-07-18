# Architecture Stewardship Cleanup

## Purpose

Apply six evidence-backed stewardship improvements after the July 2026 review. The work fixes
specific CI bypasses, duplicate server contracts, reconstruction failure behavior, documentation
automation, and client composition seams without starting a general refactoring program. Complete
the first four phases, reassess the remaining client work against current `origin/main`, then finish
only the two already-approved client outcomes and stop.

## Overall Constraints

- Optimize for easier future changes and fewer silent contract drifts, not architectural purity.
- Keep each phase to one job or one tightly coherent ownership surface. An implementation agent may
  omit listed touch points that prove unnecessary, but must not absorb deferred work merely because
  nearby files are open.
- Preserve server authority, fog/privacy projection, wire compatibility, balance, command budgets,
  client pixels/timing, and ordinary live/Lab/replay behavior except where Phase 3 explicitly aligns
  oversized command handling on whole-command rejection.
- Do not introduce a framework, DI container, generalized registry platform, service tier, broad
  compatibility layer, or mass file-splitting campaign.
- Keep deployment-required Rust/server/client mirrors. Name one domain owner, document intentional
  wire or UI mirrors, and prove their parity rather than pretending dependency direction can remove
  every repeated representation.
- Treat metrics as evidence, not goals. Do not add a ratchet unless it blocks a concrete bypass that
  current checks permit.
- Implement phases serially from current `origin/main`. Each phase gets its own clean worktree and
  `zvorygin/` branch, is committed separately, and is pushed as an owned PR with auto-merge armed.
- After opening each phase PR, wait for a definite merge and verify its head is reachable from
  `origin/main` before reporting completion or starting the next phase. Mark the phase document done
  in that phase's implementation commit.
- After every phase, provide a handoff describing what changed, what the next agent should do, and
  the core features that should be manually tested; do not turn the handoff into an exhaustive test
  matrix.

## Phase Summaries

### [Phase 1 - Make Client Guardrails Truthful](phase-1.md)

Extract the mixed player-palette mirror, then select cross-language CI only for config modules that
actually mirror server-owned rules, factions, timing, palette, or other contracts. Add CSS to
source-size inventory and mechanically refresh stale exceptions while keeping beneficial shrinkage
advisory. This phase changes one module boundary plus guardrail selection and baselines only.

### [Phase 2 - Make Documentation Stewardship Recoverable](phase-2.md)

Give the scheduled documentation sweep one deterministic branch lifecycle for active, merged,
closed, stale, dirty, and conflicted runs. Preserve generated commits and use a fresh run branch
after terminal PR state instead of trying to fast-forward a permanently divergent fixed branch.
This phase fixes the observed scheduled failure without broadening into route-map maintenance.

### [Phase 3 - Consolidate Server Contract Ownership](phase-3.md)

Make `rts-rules` the typed domain owner for ability and upgrade identities while retaining and
checking the dependency-required protocol wire mirror. Give ordinary and Lab-bypass command unit
caps one shared owner consumed by runtime admission, Lab artifact validation, and checkpoint
validation. Align raw lists above either cap on whole-command rejection while preserving the current
numeric limits and command-budget behavior.

### [Phase 4 - Make Reconstruction Commit on Success](phase-4.md)

Make ordinary replay seeking rebuild candidate state before mutating the active session. Apply one
small panic-to-error boundary to replay seeking and the already-candidate-based Lab reconstruction
paths without creating a reconstruction framework. Preserve the prior authoritative session after
any normal error or panic and commit only on success.

### [Phase 5 - Make Client Command Interaction Explicit](phase-5.md)

Centralize the duplicated issue-and-record command path used by Input, HUD, and Minimap. Replace
hidden command-policy discovery through `GameState` with one narrow read-only policy projection,
while keeping command authority in the explicit interaction. Preserve ordinary, spectator, Lab,
prediction, queued-command, visual, and audio behavior.

### [Phase 6 - Report Net Subscriber Failures Safely](phase-6.md)

Keep `Net` subscribers isolated while making the first occurrence of each stable subscriber-error
signature visible through bounded always-on logging. Optionally mirror the report into debug
diagnostics, never capture payloads, and continue later subscribers even if reporting throws. This
phase is a deliberately small observability fix, not a logging framework.

## Phase Index

1. [Phase 1 - Make Client Guardrails Truthful](phase-1.md)
2. [Phase 2 - Make Documentation Stewardship Recoverable](phase-2.md)
3. [Phase 3 - Consolidate Server Contract Ownership](phase-3.md)
4. [Phase 4 - Make Reconstruction Commit on Success](phase-4.md)
5. [Phase 5 - Make Client Command Interaction Explicit](phase-5.md)
6. [Phase 6 - Report Net Subscriber Failures Safely](phase-6.md)

## Checkpoint After Phase 4

Stop after Phase 4 and run a bounded current-main consumer audit for the exact `state.controlPolicy`,
duplicated issue-and-record, and silent `Net._emit` paths named by Phases 5 and 6. The only permitted
outcomes are to execute a phase unchanged, narrow consumers that no longer exist, or cancel a phase
whose defect has already disappeared. Do not rerun broad hotspot scoring or add new stewardship
scope at this checkpoint.

Land the checkpoint result as a small owned plan-update PR before invoking another phase runner. If
a defect has disappeared entirely, set that phase to `Status: Done.`, add a `Cancellation Evidence`
section naming the current-main proof and stating that no implementation was performed, and remove
its executor command below in the same PR. Otherwise record only the narrowed touch points; do not
mark incomplete work done.

### Checkpoint Result (2026-07-17)

Audited current `origin/main` at `b7c4a0e6` after Phase 4 merged. Both approved defects remain and
neither phase is canceled: execute Phases 5 and 6 unchanged.

- Phase 5: `Match` still publishes `labControlPolicy` through `state.controlPolicy`; the hidden
  consumers remain in command budget, HUD, Input and its selection/control-group helpers, Minimap,
  renderer ownership/feedback, combat audio, LabPanel, ReplayControls, and Match shell visibility.
  Input, HUD, and Minimap still each own the same issue-selected-snapshot-record wrapper and local
  `issueGameplayCommand` compatibility helper. These are the exact approved ownership seams; do not
  broaden the phase beyond them.
- Phase 6: `client/src/net.js` `Net._emit` still catches each subscriber exception and discards it
  silently before continuing. The bounded, payload-independent first-signature reporting work
  remains necessary and its existing touch points are unchanged.

## Final Checkpoint After Phase 6

Rerun the focused architecture and contract checks and review whether hidden policy access and
silent subscriber failures are gone. Archive the plan and stop; create another plan only when
current development or playtesting demonstrates a concrete remaining cost.

## Deferred Backlog

- Add further no-growth guards for the public `Game` seam, prototype-grafted methods, or client
  fan-out only after a concrete bypass proves current exact-name, area, and export checks inadequate.
- Make partial `Match` construction fully transactional. First require an observed startup failure
  or a separately approved design that distinguishes replay-restart recovery from a live room whose
  server-authoritative match has already begun.
- Add App-level reconnect or explicit live-session recovery if startup failures occur in practice;
  client cleanup alone must not claim to return an authoritative in-game room to the lobby.
- Narrow the broad public `Game`/entity surface or decompose `RoomTask`, `server/src/main.rs`, Match,
  App, Input, Renderer, or the stylesheet along observed responsibility seams.
- Consolidate AI profile descriptors and share pure placement geometry where drift is demonstrated.
- Add representative Rust-serializer-to-JavaScript-decoder protocol fixtures.
- Replace raw `window.__rts` test access with a narrow test bridge.
- Continue retiring the Pixi compatibility adapter as presentation-frame fields naturally move.

These are candidates, not approved implementation phases. Do not execute them from this plan.

## Suggested Executor Commands

After the plan is approved and each preceding phase has merged:

```bash
scripts/phase-runner.sh --plan stewardship phase-1 --pr --wait
scripts/phase-runner.sh --plan stewardship phase-2 --pr --wait
scripts/phase-runner.sh --plan stewardship phase-3 --pr --wait
scripts/phase-runner.sh --plan stewardship phase-4 --pr --wait
# Stop here and merge the required Phase 4 checkpoint plan update.
scripts/phase-runner.sh --plan stewardship phase-5 --pr --wait
scripts/phase-runner.sh --plan stewardship phase-6 --pr --wait
```

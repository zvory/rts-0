# Architecture Stewardship Cleanup

## Purpose

Apply a bounded series of high-return architectural cleanups after the July 2026 review. The
codebase does not need a rewrite: its authority, projection, crate, and lifecycle foundations are
sound, while several checks, duplicated contracts, and composition seams need maintenance. Complete
these ten deliberately narrow phases, then stop and reassess from current evidence rather than
turning stewardship into a permanent refactoring program.

## Overall Constraints

- Optimize for easier future changes and fewer silent contract drifts, not architectural purity.
- Keep each phase to one job or one tightly coherent ownership surface. Do not absorb an adjacent
  phase merely because its files are already open.
- Preserve server authority, fog/privacy projection, wire compatibility, current balance and command
  semantics, client pixels/timing, and ordinary live/Lab/replay behavior unless a phase explicitly
  says otherwise.
- Give each phase to one trusted implementation agent. The phase documents define outcomes and
  boundaries; the agent should choose the smallest coherent implementation and may avoid listed
  touch points that prove unnecessary.
- Do not introduce a framework, DI container, generalized registry platform, new service tier, broad
  compatibility layer, or mass file-splitting campaign.
- Keep mirrors that are required by the Rust/server and plain-JavaScript deployment model; improve
  their ownership and parity checks instead of trying to eliminate every repeated representation.
- Treat metrics as evidence, not goals. A split or new abstraction must reduce real ownership,
  coupling, failure, or review risk rather than merely lower a line count.
- Implement phases serially from current `origin/main`. Each phase gets its own clean worktree and
  `zvorygin/` branch, is committed separately, and is pushed as an owned PR with auto-merge armed.
- After opening each phase PR, wait for a definite merge and verify its head is reachable from
  `origin/main` before reporting completion or starting the next phase. Mark the phase document done
  in that phase's implementation commit.
- After every phase, provide a handoff describing what changed, what the next agent should do, and
  the core features that should be manually tested; do not turn the handoff into an exhaustive test
  matrix.

## Phase Summaries

### [Phase 1 - Close the Client Config CI Gap](phase-1.md)

Make every internal client rules/config mirror file select the same balance and faction parity
coverage as the public `config.js` facade. Add selector cases that fail when a new mirror file can
bypass those checks. This phase changes test selection and parity-checker discovery only.

### [Phase 2 - Make Documentation Stewardship Recoverable](phase-2.md)

Make the scheduled documentation sweep recover cleanly from stale runner state while preserving any
generated work on the sweep branch. Refresh the documentation route map for the known split source
families and reject routes that match no tracked files. This phase stays inside documentation
automation, route health, and their focused tests.

### [Phase 3 - Refresh Source-Size Guardrails](phase-3.md)

Refresh genuinely obsolete source-size exceptions and include CSS in the tracked inventory. Keep
above-cap shrinkage advisory so ordinary cleanup does not create CI bookkeeping. This phase changes
only size inventory policy and its checked-in baseline.

### [Phase 4 - Freeze Reviewed Architecture Seams](phase-4.md)

Add narrow no-growth checks for the external `Game` seam and the reviewed client prototype-graft
and fan-out boundaries. Capture the current intentional shape without refactoring runtime code to
improve the numbers. This phase does not baseline the temporary `GameState.controlPolicy` path that
Phase 8 removes.

### [Phase 5 - Give Rule Identities One Typed Owner](phase-5.md)

Make `rts-rules` the typed authority for upgrade and ability identities, stable ids, and ability
target modes. Keep simulation-specific ability effects in `rts-sim` and retain thin re-exports when
they materially reduce churn. This phase changes ownership, not gameplay, balance, or wire shapes.

### [Phase 6 - Share Command Unit-List Limits](phase-6.md)

Move the ordinary and Lab-bypass command unit-list caps to one dependency-safe owner. Consume the
same constants from runtime command admission and Lab replay validation. This phase preserves the
existing limits and whole-command rejection behavior.

### [Phase 7 - Make Reconstruction Commit on Success](phase-7.md)

Make ordinary replay seeking rebuild candidate state before mutating the active session. Add a
small shared panic-to-error wrapper around replay seeking, Lab timeline seeking, and Lab replay
import without forcing them through a reconstruction framework. This phase preserves the current
room after either an ordinary error or a panic.

### [Phase 8 - Make Client Command Interaction Explicit](phase-8.md)

Give Input, HUD, and Minimap one command interaction that issues a command and records its local
planned feedback exactly once. Inject separate read-only control and feedback policy into consumers
that cannot issue commands, then remove hidden discovery through `GameState`. This phase preserves
ordinary, spectator, Lab, prediction, queued-command, visual, and audio behavior.

### [Phase 9 - Roll Back Failed Match Startup](phase-9.md)

Make `Match` clean up its own partial construction and make `App` own rollback of the full session
assembly. Cover failures both inside Match and after Match succeeds while App is creating the Lab
shell. This phase leaves the lobby usable and permits a successful subsequent match start.

### [Phase 10 - Report Net Subscriber Failures Safely](phase-10.md)

Keep network subscribers isolated while making thrown subscriber errors observable. Bound or
deduplicate reporting so a hot snapshot path cannot flood diagnostics, and include the message type
without exposing protocol or payload data unnecessarily. This phase changes diagnostics only and
must never interrupt later subscribers.

## Phase Index

1. [Phase 1 - Close the Client Config CI Gap](phase-1.md)
2. [Phase 2 - Make Documentation Stewardship Recoverable](phase-2.md)
3. [Phase 3 - Refresh Source-Size Guardrails](phase-3.md)
4. [Phase 4 - Freeze Reviewed Architecture Seams](phase-4.md)
5. [Phase 5 - Give Rule Identities One Typed Owner](phase-5.md)
6. [Phase 6 - Share Command Unit-List Limits](phase-6.md)
7. [Phase 7 - Make Reconstruction Commit on Success](phase-7.md)
8. [Phase 8 - Make Client Command Interaction Explicit](phase-8.md)
9. [Phase 9 - Roll Back Failed Match Startup](phase-9.md)
10. [Phase 10 - Report Net Subscriber Failures Safely](phase-10.md)

## Checkpoint After Phase 10

Stop after Phase 10 and rerun the architecture checks and current hotspot analysis. Review whether
the changed boundaries actually reduced bypass risk, duplicate ownership, and failure ambiguity; do
not judge success by file size alone. Create another small plan only if current development or
playtesting demonstrates that a deferred item is still costly.

## Deferred Backlog

- Narrow the broad public `Game`/entity surface and simplify its constructor family beyond the
  no-growth guard added in Phase 4.
- Decompose `RoomTask`, `server/src/main.rs`, Match, App, Input, Renderer, or the stylesheet along
  observed responsibility seams.
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
scripts/phase-runner.sh --plan stewardship phase-5 --pr --wait
scripts/phase-runner.sh --plan stewardship phase-6 --pr --wait
scripts/phase-runner.sh --plan stewardship phase-7 --pr --wait
scripts/phase-runner.sh --plan stewardship phase-8 --pr --wait
scripts/phase-runner.sh --plan stewardship phase-9 --pr --wait
scripts/phase-runner.sh --plan stewardship phase-10 --pr --wait
```

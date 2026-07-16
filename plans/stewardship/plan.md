# Architecture Stewardship Cleanup

## Purpose

Apply a short series of high-return architectural cleanups after the July 2026 review. The codebase
does not need a rewrite: its authority, projection, crate, and lifecycle foundations are sound, while
several checks, duplicated contracts, and composition seams now need maintenance. Complete these
three phases, then stop and reassess from current evidence rather than turning stewardship into a
permanent refactoring program.

## Overall Constraints

- Optimize for easier future changes and fewer silent contract drifts, not architectural purity.
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

### [Phase 1 - Make Safeguards Truthful](phase-1.md)

Fix the known paths where CI selection, documentation drift, and architecture ratchets currently
pass without covering the intended surface. Refresh stale baselines and add narrow no-growth guards
for CSS, the external `Game` seam, client prototype grafts/fan-out, and hidden command-policy state.
This phase changes enforcement and documentation only; it does not refactor runtime code to improve
the resulting metrics.

### [Phase 2 - Consolidate Server Invariants](phase-2.md)

Give upgrade and ability identities one typed rules owner, share command-list limits between runtime
and replay validation, and remove panic-on-registry-drift behavior. Put replay and Lab reconstruction
behind one panic-contained, commit-on-success boundary while preserving gameplay and wire behavior.
Stop at these high-risk invariants rather than broadening the work into `Game`, `RoomTask`, tick, or
AI restructuring.

### [Phase 3 - Tighten Client Command and Session Seams](phase-3.md)

Replace hidden `GameState` command-policy discovery and repeated issue-and-record helpers with one
small explicitly injected command interaction. Make partially constructed matches unwind cleanly and
make subscriber failures observable without sacrificing event-handler isolation. Preserve current
command, Lab, prediction, visual, audio, and rematch behavior and avoid a general client rewrite.

## Phase Index

1. [Phase 1 - Make Safeguards Truthful](phase-1.md)
2. [Phase 2 - Consolidate Server Invariants](phase-2.md)
3. [Phase 3 - Tighten Client Command and Session Seams](phase-3.md)

## Checkpoint After Phase 3

Stop after Phase 3 and rerun the architecture checks and current hotspot analysis. Review whether
the changed boundaries actually reduced bypass risk, duplicate ownership, and failure ambiguity; do
not judge success by file size alone. Create another small plan only if current development or
playtesting demonstrates that a deferred item is still costly.

## Deferred Backlog

- Narrow the broad public `Game`/entity surface and simplify its constructor family beyond the
  no-growth guard added in Phase 1.
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
```

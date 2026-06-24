# Room Policy Capability Naming Plan

## Purpose

Finish the room-policy naming cleanup started by PR #393. The goal is to make shared room
affordances read as capabilities the platform offers, not as the first product path that happened to
use them. This is mostly synchronized renaming and small composition cleanup; it should not become a
hardening pass, behavior rewrite, or broad room architecture refactor.

## Overall Constraints

- Keep this to two implementation phases unless a phase discovers a real behavior or compatibility
  blocker.
- Preserve gameplay behavior, room lifecycle behavior, fog/privacy behavior, replay seek behavior,
  lab timeline behavior, branch behavior, and match-history behavior.
- Do not rename true product/source concepts just because they contain product words. `RoomMode`,
  `SessionMode`, replay artifact metadata, lab metadata, lab operations, replay-session internals,
  and replay-branch staging can keep product names where they represent actual setup/source data.
- Rename policy and capability names when they describe what the room allows: observer-analysis
  audience, visibility/projection shape, room-time UI controls, branch-from-tick affordances, and
  selectable vision/perspective controls.
- Wire changes in Phase 2 should be direct synchronized protocol changes for the current pre-alpha
  client/server. Do not add long-lived compatibility shims unless implementation finds an immediate
  stale-client or deploy-order blocker.
- For every protocol field or tag rename, update `server/crates/protocol/src/lib.rs`,
  `server/crates/contract/src/lib.rs`, `client/src/protocol*.js`, `docs/design/protocol.md`, and
  focused protocol parity/client contract coverage in the same commit.
- For every client room-time rename, keep the visible controls and storage behavior stable unless a
  name is user-facing and intentionally changes from replay/dev wording to room-time wording.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is
  reachable from `origin/main`.
- After implementing each phase, the implementing agent must mark that phase document done and
  provide a handoff message describing what the next agent should do and what should be manually
  tested. Manual testing notes should cover core features, not an exhaustive matrix.

## Phase Summaries

### [Phase 1 - Internal And Client Names](phase-1.md)

Rename capability-like internal policy variants and client room-time control surfaces without
changing the wire protocol. This phase should clean up server names such as replay/dev/lab-specific
observer-analysis and visibility-policy labels where they actually mean spectator-only analysis,
all-recipient analysis, selected perspective, or full-world projection. It should also move the
shared room-time client markup/selectors/helpers away from `replaySpeed`, `dev-pause`, and
`replay-timeline` naming while keeping the same controls visible for replay, AI-only live,
dev-watch, and lab rooms.

### [Phase 2 - Protocol Capability Names](phase-2.md)

Rename the public protocol capability and message names that still expose replay-specific wording
for shared room affordances. This phase should update observer analysis, selectable vision, and
branch-from-tick/fork-from-tick names across Rust DTOs, JS mirrors, docs, and focused tests in one
synchronized change. It should be a direct current-protocol rename rather than a compatibility
framework unless implementation finds a concrete deploy-order issue.

## Phase Index

1. [Phase 1 - Internal And Client Names](phase-1.md)
2. [Phase 2 - Protocol Capability Names](phase-2.md)

## Non-Goals

- Do not redesign room lifecycle, room actor ownership, replay seek, lab timeline rebuilds, branch
  launch, snapshot projection, or match-history persistence.
- Do not add new hardening limits, validation paths, protocol negotiation, stale-client support, or
  replay artifact migrations.
- Do not rename every occurrence of "replay", "lab", "dev", or "branch"; only rename terms that
  are standing in for a reusable room capability.
- Do not split large files or introduce new abstractions unless the rename becomes confusing
  without a tiny local helper.

## Suggested Execution

Run one phase at a time and wait for each PR to merge before starting the next phase:

```bash
scripts/phase-runner.sh --plan roompolicy phase-1 phase-2 --pr --wait
```

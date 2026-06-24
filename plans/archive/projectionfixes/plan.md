# Projection Fixes Plan

## Purpose

Fix the projection-model bugs where lab, replay, dev, and spectator views diverge from normal live
play because the client and server disagree about which owner or vision policy owns a command,
event, overlay, or remembered world fact.

The highest priority is lab control: a lab operator must be able to select a side and get matching
command affordances, command dispatch, order feedback, range overlays, rally overlays, selection
colors, and audio categories for that controlled owner. Server projection fixes should make
transient events attach through the same projection model as snapshots, while preserving intentional
gameplay rules such as globally visible artillery firing markers. Lower-priority spectator/replay
work should clarify team-union memory and all-player resource visibility without over-engineering
cases that are mostly observer polish.

## Current Decisions

- `ArtilleryFiring` is intentionally global gameplay information. Do not remove global delivery;
  only document or test that behavior if the relevant gameplay/protocol docs are unclear.
- Keep `issueCommandAs` as the privileged lab operation. Normal gameplay commands are authenticated
  by the WebSocket sender's active player seat; lab starts are spectator-shaped and need an explicit,
  auditable operator-only issuer override instead of pretending the operator connection is each
  player.
- A lab command should still resolve its issuer from the currently selected controllable owner. If
  selected entities span multiple owners, the command should stay blocked with a clear reason.
- A spectator switching from all vision to one player's replay vision should see only that player's
  current vision and memory. Team/union vision may include multiple players' memories, even when
  those memories contradict, but contradictions must be represented deliberately rather than
  accidentally flattened through viewer id `0`.
- Full-world views should receive full-world events. This is separate from the already-known lab
  full-world P2 bucket bug and should be fixed through the shared projection seam.

## Phase Summaries

Phase 1 fixes lab-controlled visual feedback. It routes renderer feedback, selection coloring,
control groups, command feedback ownership, range overlays, setup wedges, rally lines, and relevant
audio categories through a control-owner/read-model seam instead of raw `state.playerId`. The result
is that selected P2 units in lab look and sound controllable after commands are accepted.

Phase 2 fixes lab command affordance and dispatch semantics. It makes command-card resources,
faction/tech requirements, train/research/cancel target selection, right-click enemy detection, and
ability hover origins resolve against the selected issue-as owner. The result is that P2 lab
commands do not silently no-op or misclassify P1/P2 targets.

Phase 3 unifies full-world event projection with full-world snapshot projection. It introduces a
shared server-side event projection path so dev full-world and lab full-world can receive events
from all relevant player buckets without duplicating privacy-sensitive logic. The result is that
full-world views show the durable world state and the transient effects that explain it.

Phase 4 cleans up spectator, replay, and lab team/union projection semantics. It gives union views a
deliberate rule for event unions, remembered buildings, and resource visibility, using the same
vision selection that builds the snapshot. The result is fewer spectator-only private notices and
correct per-player or team memory when switching replay/lab vision modes.

Phase 5 adds regression coverage and docs for the projection contract. It documents globally visible
artillery firing, `issueCommandAs`, full-world events, team-union memory, and resource visibility,
then adds focused tests for the cross-mode cases. The result is a maintained projection contract
that future lab/replay/spectator work can verify cheaply.

## Phase Index

1. [Phase 1 - Lab Control Feedback](phase-1.md)
2. [Phase 2 - Lab Command Ownership](phase-2.md)
3. [Phase 3 - Full-World Event Projection](phase-3.md)
4. [Phase 4 - Union Vision Semantics](phase-4.md)
5. [Phase 5 - Projection Contract Hardening](phase-5.md)

## Overall Constraints

- Preserve authoritative fog. Do not send normal active players entity ids, positions, death
  effects, target ids, or private owner fields that their projection should not include.
- Preserve global artillery firing markers unless the user explicitly changes the gameplay rule.
- Keep protocol mirrors synchronized if a wire field changes:
  `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
  `docs/design/protocol.md`.
- Prefer projection-policy helpers over mode-name checks. Replay, lab, dev, and spectator behavior
  should be selected by start-payload capabilities, lab control policy, replay vision, and server
  projection policy.
- Keep lab UI app-owned. `Match` may receive injected lab metadata/control policy; ordinary client
  modules should not import `LabClient` or lab panel code.
- Avoid changing unrelated balance or gameplay behavior. This plan is about projection and control
  ownership, not unit stats.
- Use focused tests per phase. Let the PR `./tests/run-all.sh` gate provide the broad final suite.
- Each phase should be implemented and committed on its own `zvorygin/` branch, pushed as an owned
  PR with auto-merge armed, and waited on until merged before starting the next phase.

## Implementation Process

Implement one phase at a time. Mark the phase document as done in the implementation commit for
that phase. After each phase, the implementing agent must provide a handoff message describing what
the next agent should do, important constraints or discoveries, and the core manual test flows.

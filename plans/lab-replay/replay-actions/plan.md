# General Replay Actions and Lab Save Plan

> [!WARNING]
> **POTENTIALLY STALE SUBDIVISION - DO NOT IMPLEMENT YET.**
> This lab-replay subdivision depends on assumptions that may change when
> `plans/archive/game-state/plan.md` lands. Re-evaluate this subplan and its phase files before
> implementation.

## Purpose

Generalize replay timelines so they can express every authoritative action needed for match replay
and lab capture. A replay starts from a checkpoint and applies typed actions with explicit timing.
The practical product outcome is a lab "Save replay so far" action that opens through the shared
replay viewer.

## Phase Summaries

### [Phase 1 - ReplayAction Contract and Tick Semantics](phase-1.md)

Define the typed `ReplayAction` contract and one timing convention for all actions. The contract
should cover actor identity, sequence ordering, tick application, validation bounds, and future
schema extension. This phase should prevent one-tick drift between match commands and lab actions.

### [Phase 2 - Player Command Timeline Executor](phase-2.md)

Move existing match command replay onto the new action contract. Playback behavior should remain
equivalent for newly captured match replays. This phase proves the checkpoint-backed artifact and
shared executor before lab actions are added.

### [Phase 3 - Lab Operator Action Timeline](phase-3.md)

Add typed replay actions for authoritative lab operations and `issueCommandAs`. Actions should
apply through public game or lab APIs, not private simulation mutation. This phase should include
entity-id validation, operator policy, and current-branch ordering rules.

### [Phase 4 - Lab Save Replay So Far](phase-4.md)

Add the server-side lab operation that serializes the active baseline checkpoint plus the retained
current-branch replay action timeline. The save path should handle blank labs, catalog labs,
imports, rewinds, edits, and cap-reset baselines intentionally. It should return a local
`/?replayArtifact=<name>` URL for dev use without writing to match history. Because this is the
phase that introduces a browser-triggered server file write, it must also introduce the write
hardening: generated safe artifact names, a fixed target directory under `target/`, no
client-supplied paths, payload/action byte caps, capability gating, and clear validation errors.

### [Phase 5 - Lab UI and Product Hardening](phase-5.md)

Add the minimal capability-gated LabPanel affordance for saving and opening the replay. Harden
artifact naming, size limits, validation errors, branch controls, and replay-viewer source policy.
Document remaining production sharing decisions separately from match history.

## Overall Constraints

- Replay actions must be authoritative state changes, not client-only render events.
- Timing semantics must be explicit and shared by match and lab actions.
- Lab replay capture uses the current baseline checkpoint plus retained current-branch actions.
- Lab save writes must be hardened in the same phase that introduces the write operation. Do not
  accept client-supplied paths or filenames, and do not write to match history.
- If a lab timeline reset captures a new checkpoint baseline, the replay may start there; if not,
  save must fail clearly when history is insufficient.
- Branch-from-replay should be source-gated and should not appear for lab captures unless
  intentionally enabled.
- The replay viewer should remain shared.

## Handoff Requirements

Every phase handoff must name the action types covered, unsupported action types, focused replay
checks that passed, and the manual replay or lab save flow to test next.

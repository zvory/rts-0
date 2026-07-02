# Lab Replay Finish Plan

## Status

Draft follow-up plan for the last checkpoint cleanup layer after `plans/checkpoint/` landed. This
plan supersedes the stale active-looking `plans/lab-replay/` directory; keep that older directory as
historical reference only. The core remaining product is portable lab replay: a checkpoint-backed
lab start plus a serializable lab operation log.

## Purpose

Finish the migration from setup-specific lab scenarios toward checkpoint-backed lab artifacts. A lab
replay should restore an exact lab checkpoint, then consume an ordered stream of lab operations and
issued commands so a lab session can be saved, shared, and replayed without relying on old
`LabScenarioV1` setup DTOs. Compatibility readers should stay until the new path has proven itself,
then the old lab scenario and replay schema 2 surfaces can be removed deliberately.

## Phase Summaries

### [Phase 1 - Lab Replay Artifact Contract](phase-1.md)

Define a versioned lab replay artifact that starts from a checkpoint-backed lab setup and appends a
stable lab operation stream. This phase should specify the container shape, size and count limits,
map/checkpoint binding rules, schema/version policy, and which current lab operations are in or out
of scope. It should add validation and serde tests without changing the live lab UI or removing old
compatibility readers.

### [Phase 2 - Lab Operation Recording And Playback](phase-2.md)

Make the room-local lab timeline produce and consume the stable lab replay operation stream. This
phase should load a lab replay by restoring the initial checkpoint and replaying lab operations,
including lab mutations, `issueCommandAs`, seek/tick advancement policy, and scenario import
boundaries. It should add focused server tests plus a live Node or browser smoke proving an exported
lab replay can be reopened and reaches the same observable lab state.

### [Phase 3 - Replace Lab Scenario Product Surface](phase-3.md)

Move user-facing lab setup/export/import wording and flows away from "lab scenarios" as the primary
concept. Setup-only artifacts should be treated as lab checkpoint setups, while evolving sessions
should be lab replays; old `LabScenarioV1` remains only as a compatibility input during this phase.
This phase should update client copy, catalog/submission naming, docs, tests, and fixtures so new
work no longer extends the legacy scenario concept.

### [Phase 4 - Retire Compatibility And Final Cleanup](phase-4.md)

After a bake-in gate, remove `LabScenarioV1` compatibility loading and replay artifact schema 2
loading. This phase should delete old adapters, fixtures, docs, protocol parity expectations, and
tests only after committed assets and known dev/self-play/crash/match-history surfaces have tested
replacement paths or intentional rejection messages. It should finish with a release audit covering
rollback, old artifact behavior, lab replay portability, and any remaining cleanup debt.

## Overall Constraints

- No balance or gameplay changes.
- Build on `GameCheckpointV1`, `LabCheckpointScenarioV1`, and schema 3 `ReplayArtifactV1`; do not
  introduce another authoritative state format.
- Treat lab replay import as untrusted JSON. Validate schema, kind, map binding, checkpoint payload,
  lab op count, op payload size, entity references, player references, command shapes, coordinates,
  and byte caps before mutating a live lab game.
- A lab replay start must be checkpoint-backed. Do not reconstruct start state from final game state
  or from old scenario setup instructions.
- Lab operation replay should use the same public `Game` lab API seam used by live lab operations.
  Do not reach into `GameState` internals from lobby/server replay code.
- Preserve id remapping semantics for setup imports. Any operation that imports a checkpoint setup
  must make later operation references unambiguous.
- Keep old replay schema 2 and `LabScenarioV1` loading until the new lab replay path has passed CI
  and manual use on representative artifacts. Removal belongs in Phase 4, not in the first
  implementation phase.
- Existing in-memory lab/replay keyframes are acceptable for seek performance. Replacing them with
  checkpoint keyframes is not required for this plan unless profiling or a product requirement shows
  that persisted/cross-process seek points are needed.
- Any protocol-visible shape change must update Rust protocol DTOs, client mirrors, protocol docs,
  and parity/client contract tests in the same phase.
- Each phase must land through its own `zvorygin/` branch, owned PR, auto-merge, and
  `scripts/wait-pr.sh <pr>` confirmation before the next phase starts.

## Bake-In Gate Before Deletion

Do not remove compatibility readers until all of these are true:

- New lab replay artifacts are written and loaded through automated tests.
- At least one live lab workflow can export, import/open, and replay a checkpoint-backed lab replay.
- Current bundled lab setups no longer require `LabScenarioV1`.
- Dev replay, self-play, crash replay, and match-history replay surfaces have schema 3 coverage or
  intentional rejection tests for old artifacts.
- Manual testing has covered an old schema 2 replay, a new schema 3 replay, an old lab scenario JSON
  import, a new lab checkpoint setup import, and a new lab replay import.

## Non-Goals

- No generic live-match "upload arbitrary checkpoint and replace the room" command.
- No requirement to replace in-memory replay or lab keyframes with checkpoint keyframes.
- No database migration for historical match rows unless Phase 4 explicitly scopes one.
- No cross-version checkpoint migration guarantee beyond the compatibility policy documented and
  tested by the implementation phases.

## Completion Definition

- Labs can save/open a portable artifact consisting of an initial checkpoint-backed lab setup plus a
  deterministic lab op stream.
- New lab setup and lab replay UX/docs no longer present `LabScenarioV1` as a primary concept.
- Legacy `LabScenarioV1` and replay schema 2 loading are either removed after the bake-in gate or
  left with a clearly documented reason and an owner for removal.
- The final audit documents what remains intentionally compatible, what was deleted, and how old
  artifacts fail or load.

## Handoff Expectations

After each phase, provide a handoff that names:

- What changed.
- Any compatibility behavior preserved or removed.
- Focused verification that passed.
- The core manual test to run next.
- Any old artifact or rollback concern that remains.

## Relationship To Checkpoint Plan

`plans/checkpoint/` made checkpoint-backed starts, replay schema 3, and checkpoint-backed lab setup
containers real. This plan uses that foundation to make lab sessions portable and to remove the
remaining compatibility scaffolding once the new path has enough evidence.

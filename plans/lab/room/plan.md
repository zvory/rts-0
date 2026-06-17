# Room Composable Refactor Plan

## Purpose

Refactor the existing room/lobby systems into small composable primitives that can support normal
matches, replays, dev scenarios, replay branches, and future labs without adding another pile of
mode-specific branches. Phase 0 intentionally removes the obsolete live `/dev/selfplay` watch path;
after that deletion, phases 1-7 are behavior-preserving and should keep current wire messages,
gameplay rules, replay semantics, lobby behavior, and visible client behavior unchanged. The goal
is to make policy choices explicit and reusable, not to add the lab, new protocol, or new room
features.

This plan follows the direction in `plans/lab/room/requirements.md` and
`plans/lab/architecture.md`: rooms stay the shared authority and lifecycle boundary, while product
modes choose explicit policies for state source, clock, authority, vision, mutation, persistence,
and UI/start affordances.

## Overall Constraints

- Phase 0 is the only intentional behavior removal in this plan. It should retire live
  `/dev/selfplay` because a normal lobby with AI players and spectator clients covers the same
  product need with less special room machinery.
- Phases 1-7 must make no behavioral changes. If a phase finds that preserving current behavior is
  ambiguous, stop and document the ambiguity instead of choosing new behavior silently.
- Do not add lab product behavior, lab protocol messages, privileged sim mutation APIs, or client
  lab panels in this plan.
- Preserve the AI self-play harness and replay artifacts as test/debug assets. If browser
  inspection of saved self-play artifacts is still useful, migrate it to a neutral replay-artifact
  entry point rather than keeping `RoomMode::DevSelfPlay`.
- Preserve the `Game` API boundary. Lobby code may continue to own one `Game`, but it must not
  reach deeper into sim internals as part of these extractions.
- Preserve the mirrored wire protocol. If a phase accidentally needs a protocol shape change, split
  that into a separate approved plan instead.
- Preserve server-authoritative fog. Any projection helper must keep player, spectator, replay,
  branch, and dev visibility rules explicit and tested.
- Keep `RoomTask::run` as the single event and tick owner for a room. Do not add locks around
  `Game`, a shared room registry mutation path, or cross-room state.
- Keep normal matches, spectators, post-match replay, persisted replay rooms, replay branch
  staging, replay branch live play, dev scenarios, empty-room reset, drain, and match-history
  decisions first-class in every phase after Phase 0.
- Leave `/dev/scenario` and `watchScenario` alone until the shared primitives exist. Dev scenarios
  are not conceptually hard, but they include scripted setup, full-world projection, pause/step
  controls, and tri-state harness usage, so they should be migrated late instead of broadening
  Phase 0.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is
  reachable from `origin/main`.
- A filtered test command only counts as phase verification when it runs the intended tests. If a
  filter matches zero tests, add a narrowly named test or use the exact existing test name before
  claiming verification.
- After each phase, the implementing agent must provide a handoff message naming verification
  results, behavior-preservation evidence, remaining risks, what the next agent should do, and the
  core features that should be manually tested.
- Manual testing notes should cover core flows, not an exhaustive matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Target Primitives

The final shape does not need a plugin framework or dynamic capability registry. It should leave
the lobby with named, boring internal units that can be recomposed later:

- a session policy descriptor for the current room mode and phase;
- participant and seat mapping helpers for connected users, spectators, active seats, host, and
  branch seat aliases;
- an authority helper for ordinary commands, read-only viewers, branch seats, and dev/lab-like
  issue-as policies later;
- a clock/tick-control helper for realtime, speed-controlled, paused, stepped, and non-ticking
  states;
- a projection policy for per-recipient snapshots and observer/dev/replay visibility;
- a launch/start-payload helper for mode-specific start payloads around the same `Game` start data;
- documentation and guardrails that make the boundaries understandable to future implementers.

## Phase Summaries

### [Phase 0 - Retire Dev Self-Play Watch](phase-0.md)

Remove the live `/dev/selfplay` watch path and its hidden `DevSelfPlay` room mode before the
composable refactor starts. Preserve the AI self-play test harness and migrate any still-useful
saved artifact inspection to a neutral replay-artifact path instead of another self-play mode.
Leave `/dev/scenario` unchanged for now because its pause/step and scripted setup behavior is a
better fit for the later shared clock, projection, and launch primitives.

### [Phase 1 - Baseline Mode Matrix](phase-1.md)

Record the current room modes, phase transitions, and policy choices before moving production code.
Add or tighten focused characterization tests around behavior that later phases will route through
shared helpers. This phase should produce a mode matrix and testing baseline, not a new abstraction.

### [Phase 2 - Session Policy Descriptor](phase-2.md)

Introduce a neutral internal descriptor that names the current mode's state source, lifecycle,
clock, authority, vision, mutation, and persistence choices. Replace scattered mode checks only
where the descriptor can express today's exact behavior. The descriptor should make future room
modes easier to add without pretending every policy combination is supported.

### [Phase 3 - Participants And Authority](phase-3.md)

Extract connected-player, host, spectator, active-seat, AI-seat, and branch-seat mapping logic into
a lobby-owned participant helper. Route ordinary command issuer resolution through that helper while
preserving current command acceptance and rejection behavior. This creates a reusable authority
boundary without changing protocol, command validation, or player ids.

### [Phase 4 - Clock And Tick Control](phase-4.md)

Extract the clock decisions that currently live across replay speed, dev pause, branch staging, and
live ticking into a neutral tick-control helper. The room task should still own the Tokio interval
and call the same live, replay, and dev tick paths. This phase should make pause, speed, step, and
non-ticking states explicit while preserving exact current timing behavior.

### [Phase 5 - Projection And Fanout Policy](phase-5.md)

Extract viewer projection decisions into a shared policy used by live fanout, replay fanout, branch
live fanout, and dev snapshots. The policy must keep per-player fog, spectator union vision,
replay per-viewer vision, branch seat aliases, and dev full-world behavior distinct. The result
should be one room-owned snapshot delivery path with explicit projection choices rather than
duplicated mode assumptions.

### [Phase 6 - Launch Plans And Start Payloads](phase-6.md)

Extract common launch bookkeeping and per-recipient `StartPayload` composition from normal live
matches, replay branch live matches, and dev sessions. Game creation and mode-specific launch rules
should remain local, but shared work such as match records, start metadata, prediction flags, and
connection payload stamping should become a small reusable unit. This prepares future lab launch
code to reuse the normal match screen without adding lab behavior in this plan.

### [Phase 7 - Cleanup, Docs, And Guardrails](phase-7.md)

Remove temporary compatibility helpers and update the server room architecture documentation after
the shared primitives exist. Add lightweight guardrails only for boundaries that have become clear
and repeatable during the earlier phases. This phase should be cleanup and documentation, not a
feature or behavior phase.

## Non-Goals

- Do not implement `RoomMode::Lab`, lab join/create routes, lab operations, scenario storage, or
  lab client UI.
- Do not redesign lobby host controls, replay controls, replay branch product behavior, dev
  scenario URLs, or match-history policy.
- Do not remove or migrate `/dev/scenario` in Phase 0. Re-evaluate it in Phase 7 after the room
  primitives exist.
- Do not move transport, connection ownership, the room registry, database writes, or AI
  controller ownership into `rts-sim`.
- Do not add a generic plugin framework, trait object registry, or dynamic capability negotiation
  unless a later plan proves it is needed.
- Do not rename user-facing messages or protocol tags as part of internal neutralization.
- Do not broaden archcheck baselines to hide boundary violations.

## Handoff Rules

Each phase handoff must include:

- exact verification commands and results;
- which normal, replay, branch, and dev paths were touched;
- whether wire shape, `Game` API shape, and gameplay semantics stayed unchanged;
- behavior still protected only by manual testing;
- the core manual test focus for the next agent;
- whether the next phase can proceed or should pause for review.

Once this plan is approved, run Phase 0 explicitly, then run the behavior-preserving range. Avoid a
range that starts at zero so the runner does not skip or mis-handle the explicit phase-zero file:

```bash
scripts/phase-runner.sh --plan lab/room phase-0 --pr --wait
scripts/phase-runner.sh --plan lab/room --from 1 --to 7 --pr --wait
```

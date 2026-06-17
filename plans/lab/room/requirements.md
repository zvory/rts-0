# Lab Room Architecture Requirements

This is a requirements artifact for the lab effort, not an implementation phase list. It describes
the architectural goal for room evolution without prescribing concrete modules, traits, protocol
shapes, or extraction order.

## Goal

Normal games, replays, dev scenarios, and labs should all be understood as room-hosted sessions
with different policies, not as unrelated special cases.

The room should remain the common session boundary for participants, lifecycle, transport, ticking,
snapshot delivery, and server-owned authority. Product modes should then choose explicit policies
for time, state source, command authority, vision, mutation, persistence, and UI affordances. The
lab can then combine the real live-game engine with replay-like time controls and lab-only
authority without duplicating replay or normal-match systems.

The target is recomposition, not a fully generic plugin framework. Named product modes should
expose only supported policy combinations, while shared room primitives avoid replay-specific or
lab-specific assumptions when the behavior is actually common.

## Definitions

- A room is the server-owned session container for connected clients and the current activity.
- A session mode is a named product workflow such as a normal match, replay viewing, dev scenario,
  branch staging, or lab.
- A policy is a bounded rule set that changes how a session uses the room shell, for example who may
  issue commands, which vision projection a viewer receives, or whether the clock can pause.
- A capability is a behavior a session may expose to users, such as speed control, step, issue-as,
  all-world vision, or scenario export.

These terms are meant to keep requirements clear. They do not require a specific type hierarchy or
runtime plugin mechanism.

## Requirements

### Shared Room Model

- Room-hosted workflows must share one conceptual lifecycle for membership, connection ownership,
  event routing, tick scheduling, snapshot fanout, teardown, and empty-room handling.
- Normal matches, replays, dev scenarios, and labs may have different policies, but those
  differences should be explicit and local to the session mode.
- Shared capabilities must use neutral language. If pause, speed, step, viewer vision, or snapshot
  delivery can apply outside replays, their architectural contract should not be replay-only.
- A future agent should be able to explain each mode as a combination of policies over the same
  room foundation, rather than as a separate room-like subsystem.

### Mode Identity

- A replay is a room-hosted read-only playback session. Its special constraints come from its state
  source and authority policy, not from being outside the room model.
- A normal match is a room-hosted live simulation session. Its default policies are realtime
  progression, player-owned command authority, normal fog, normal validation, and normal match
  outcome.
- A lab is a room-hosted privileged live simulation session. It should use the real game engine and
  normal renderer, while adding explicit lab policies for operator authority, controllable time,
  scenario setup, vision selection, and privileged mutations.
- Dev scenarios should be treated as development workflows that may share room primitives, not as
  the conceptual parent of the lab.

### Policy Axes

Each room-hosted session should make these choices explicit:

- State source: whether visible state comes from a live authoritative game, a replay artifact, a
  staged scenario, or a branch.
- Clock: whether time is fixed realtime, speed-controlled, paused, stepped, seekable, or otherwise
  externally controlled.
- Authority: which connected users may issue gameplay commands, privileged commands, playback
  controls, or no commands at all.
- Vision: which fog or full-world projection each viewer receives, and whether the viewer's choice
  is per-room, per-viewer, or role-based.
- Mutation: whether the session accepts only normal gameplay commands, rejects mutations entirely,
  or accepts explicit privileged lab operations.
- Persistence: whether the session records match history, replay artifacts, branch metadata,
  scenario JSON, operation logs, or no durable output.
- UI shell: which controls surround the normal match view, and whether those controls are mode
  controls, gameplay controls, or lab-only tools.

The architectural goal is to make these axes visible enough that features can move from one mode to
another when appropriate without copying whole modes.

### Invariants

- The authoritative simulation remains behind the public game API seam. Room-level code should not
  mutate sim internals directly just because a mode is privileged.
- Clients remain untrusted in every mode. Lab and replay controls still need bounded, validated
  server-side handling.
- Fog and snapshot projection remain server-authoritative. Lab vision can be privileged, but it must
  be an explicit room policy rather than an accidental leak from another mode.
- Normal match behavior must remain stable while shared primitives are introduced.
- Replay determinism and read-only playback semantics must remain stable while reusable replay
  primitives are generalized.
- Lab privileges must be room-local. A lab operator can affect that lab session, not other rooms,
  global server state, arbitrary files, or normal matches.
- Policy names and product behavior should stay understandable to future implementers and testers.
  Avoid abstractions that hide which mode is allowed to do what.

### Recomposition Expectations

- Time controls should be reusable where a mode supports them, while unsupported controls remain
  unavailable by policy.
- Vision controls should be reusable across replay, spectator, dev, and lab workflows without
  implying that every mode has omniscient vision.
- Command routing should distinguish normal player authority, read-only viewing, and lab issue-as
  authority without pretending they are the same permission.
- Snapshot fanout should remain one room responsibility with different projection policies, not a
  set of duplicated replay, live, and lab broadcasters.
- UI controls should be layered around the normal game view where possible. Lab should add panels
  and control policies around the real match experience instead of becoming a second renderer.

## Non-Goals

- Do not prescribe a concrete Rust or JavaScript implementation shape here.
- Do not require a plugin framework, dynamic capability registry, or inheritance model.
- Do not require every policy combination to become user-facing or test-supported.
- Do not turn replay into a mutable lab session unless a later plan explicitly designs that product
  behavior.
- Do not treat dev scenario machinery as the lab architecture just because it already has some
  similar controls.
- Do not include protocol message sketches, API signatures, module names, or phase sequencing in
  this requirements document.

## Success Criteria

- A future lab plan can state which room policies it needs before choosing implementation details.
- Existing replay and dev controls can be evaluated as candidates for shared room primitives without
  assuming their current names or wiring are permanent.
- The lab can reuse normal match rendering, replay-like time control, and explicit lab authority
  without copying an entire replay session or live match path.
- New code reviews can ask whether a proposed lab feature belongs to the shared room shell, a
  reusable policy, or a lab-only capability.
- The architecture leaves room for later timeline, branch, and scenario workflows without making the
  first lab implementation solve all of them.

## Open Questions

- Which currently replay-named controls should become neutral room capabilities before the lab MVP?
- Which policy axes must be formalized first to avoid lab-specific duplication, and which can remain
  mode-local until a second consumer appears?
- How much of dev scenario behavior should migrate to shared room primitives once lab exists?
- What product language should distinguish a room, a session mode, a policy, and a capability in
  user-facing or developer-facing documentation?

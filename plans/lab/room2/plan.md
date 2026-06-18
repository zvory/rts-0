# Room Policy Completion Plan

This is an architectural planning artifact, not a runner-ready multi-phase plan. It describes the
target shape for completing the room-policy refactor after `plans/lab/room/`; a later agent should
convert this into phase files after the scope and ordering are reviewed.

## Plain-Language Explanation

The first room refactor gave us useful shared helpers: session policy, participants, tick control,
projection, and launch payload code. It did not fully finish the architecture goal from
`plans/lab/room/requirements.md`, because some behavior is still attached to product-mode names like
replay, dev watch, or Debug mode. That means a future feature can still end up asking "am I a replay
or a dev scenario?" when the real question is narrower, such as "does this room run realtime or does
the room control time?" or "does this viewer get normal actor vision or room-controlled vision?"

This follow-up should make those reusable behaviors explicit capabilities. Normal matches, replays,
dev scenarios, replay branches, and labs can stay named product workflows, but downstream code should
mostly consume neutral policy choices: clock ownership, command authority, snapshot visibility,
diagnostic affordances, and persistence. The end state should be easy to explain: a room is the
common runtime shell, a session mode picks a supported bundle of policies, and lower-level code does
not infer behavior from mode names unless the behavior is genuinely unique to that product mode.

## Why This Exists

`plans/lab/room/` was deliberately behavior-preserving. That was the right first step, but it left
compatibility-shaped names and policy holes that are now visible:

- `SessionPolicy` centralizes high-level axes, but it does not yet describe whether diagnostic
  overlays are available for a room.
- `tick_control.rs` centralizes timing decisions, but reusable time behavior still appears through
  replay/dev-watch names in several places.
- `Game` decides whether entity snapshots include debug path data from `debug_path_overlays`, which
  is tied to Debug starting loadout and dev scenario construction rather than a room projection
  capability.
- The client enables debug path overlay UI by combining `debugMode` with `devWatch.kind ===
  "scenario"`, so the UI also infers a capability from product mode.
- Protocol and docs still expose compatibility names such as `setReplaySpeed`, `stepDevTick`, and
  `replayState` for behavior that is already shared beyond replay.

The problem is not that these names exist. Some product-mode names are valid at the boundary. The
problem is that shared behavior can still leak through those names into lower-level architecture.

## Architectural Goal

Complete the refactor so product modes choose policies and capabilities, while downstream helpers use
those choices directly.

The target is still a boring, explicit internal architecture. Do not build a plugin framework,
dynamic capability registry, or generic trait-object room runtime unless a later design proves it is
needed. Prefer small typed enums, structs, and helper methods that make today's supported behavior
clear and keep unsupported combinations unavailable.

## Target Concepts

Keep these concepts separate:

- `SessionMode`: the named product workflow, such as normal match, replay viewer, dev scenario,
  replay branch, or lab.
- `SessionPhase`: where the room is in its lifecycle, such as lobby, live game, replay viewer, or
  branch staging.
- `SessionPolicy`: the complete policy bundle selected by mode plus phase.
- `Capability`: a specific coarse behavior a policy may expose, such as room-controlled time,
  room-controlled vision, diagnostics, issue-as commands, scenario export, or match history
  recording. Fine-grained operations such as pause, speed, step, seek, selected-player vision, and
  movement paths live under those capabilities; they are not separate policy axes by default.
- `Affordance`: the UI or protocol surface that lets a user operate a capability.

A product mode may still have bespoke setup. Dev scenario construction, replay artifact loading,
branch staging, and lab scenario setup are not identical. The refactor is successful when their
shared runtime decisions no longer require downstream product-mode checks.

## Policy Axes To Complete

The later phase plan should evaluate and finish these axes.

### State Source

Name where authoritative state comes from: lobby state, live game, replay artifact playback,
replay branch seed, dev scenario game, or lab game. This axis can remain product-aware because state
source is often genuinely mode-specific.

### Lifecycle And Joining

Name who may join, what prompt or start payload they receive, and how empty-room reset behaves.
Product-specific join flows are acceptable at the edge, but shared teardown, reset, drain, and
connection ownership should continue moving through room-owned helpers.

### Clock Capability

Represent active simulation time with two coarse choices:

- fixed realtime ticking;
- room-controlled time.

Room-controlled time means the room owns the allowed operations for that session, such as pause,
speed, step, or seek when they make sense for the state source. Those operations should not become
separate top-level policies unless a later design proves that distinction is useful. Countdown and
branch staging are lifecycle/start states around the clock, not clock capabilities themselves.

Downstream tick code should not need to ask whether a room is a replay or dev watch merely to decide
whether the room owns time. Product-specific action names can remain at the protocol edge until a
separate protocol migration is worth the compatibility cost.

### Authority Capability

Represent what each connected user can do:

- lobby host controls;
- live owner commands;
- read-only viewing;
- replay playback controls;
- branch seat alias commands;
- dev scenario watch controls;
- lab operator privileged operations;
- lab issue-as gameplay commands.

Command routing should consume authority decisions, not mode identity. The server must keep
validating ownership, seat aliases, spectator status, defeated status, stale sequence ids, and lab
operator privileges.

### Snapshot Visibility

Represent visibility with two coarse choices:

- normal actor vision;
- room-controlled vision.

Normal actor vision means the recipient receives the view their current role is ordinarily entitled
to, such as a player's own fog or a spectator projection allowed by the room. Room-controlled vision
means the room can choose the projection for that recipient, such as full world, selected players,
teams, or unions. Authority decides who may change that projection; the visibility policy only says
whether the room is allowed to control it.

Visibility must stay server-authoritative. Full-world and selected-player vision are privileged
projection choices under room-controlled vision, not side effects of being a dev scenario or lab.

### Diagnostic Detail

Add an explicit diagnostic policy for data that is not just visibility:

- diagnostics disabled;
- diagnostics enabled.

Movement debug paths are one diagnostic overlay, not the policy itself. A room should be able to say
whether diagnostic overlays are available without encoding that choice in `Game::debug_path_overlays`,
`StartingLoadout::DebugHuman`, or `devWatch.kind`.

The server-side data scope and the client-side display scope are separate decisions. For example, a
server may include movement path diagnostics only for entities visible under the recipient's
projection, while the client may choose to draw only selected units. Dev scenarios may choose a
full-world diagnostic projection, while normal Debug mode may still display only selected owned units
from the data it receives. The phase plan should preserve those behaviors without turning every draw
mode into a policy variant.

### Start Payload And UI Affordances

Start payloads should advertise the small set of supported capabilities and UI affordances the client
actually needs. This should not become a broad capability manifest or dynamic UI registry. The client
should not infer debug overlay availability from dev scenario identity, and future lab controls should
not need to masquerade as replay controls just to reuse room-controlled time UI.

This does not require renaming every existing wire field immediately. It does require a migration
story where compatibility fields are produced from the policy bundle, and new client code reads
neutral capability metadata where available.

### Persistence And Export

Represent whether a room records match history, captures replay artifacts, suppresses public history,
stores branch metadata, exports lab scenarios, records lab operation logs, or writes nothing. This
must remain room-local and environment-safe.

## Desired Module Responsibilities

`server/src/lobby/session_policy.rs` should become the central place that maps `SessionMode` plus
`SessionPhase` to the full policy bundle. It should include neutral capabilities where behavior is
shared, while still naming product-specific state sources and lifecycle variants when those are real.

`server/src/lobby/tick_control.rs` should consume the coarse clock policy, current room-controlled
time state, and lifecycle state around countdown or staging. Its public outputs should stay modest:
effective tick interval plus the next scheduled room action. Pause, speed, step, and seek handlers
can remain explicit operations, but their permission should come from room-controlled time rather than
from replay/dev identity.

`server/src/lobby/projection.rs` should own both visibility policy and diagnostic policy. It
should be the place that says which recipient gets player fog, union fog, full-world vision, replay
vision, observer analysis, and diagnostic data.

`server/crates/sim/src/game/snapshot.rs` should expose snapshot construction through neutral options
instead of reading durable product-mode state to decide optional diagnostic fields. The `Game` can
own the facts needed to build debug path views, but the room projection policy should decide whether
those facts are included for a recipient.

`server/src/lobby/launch.rs` should keep stamping start payloads, but the payload should derive UI
affordance flags from the session policy. Compatibility fields such as `debugMode` can remain during
migration, but their source should be the policy bundle rather than a sim starting-loadout shortcut.

`client/src/match.js`, `client/src/state.js`, settings UI, replay controls, and lab shell code should
consume explicit capability metadata. The client may keep compatibility fallback behavior for older
payloads, but new behavior should not ask whether the match is a dev scenario when it only needs to
know whether debug path overlays are available.

## Guardrails

Add guardrails only when the boundary is stable and mechanically checkable. Good candidates:

- prevent new snapshot fanout paths that bypass `server/src/lobby/projection.rs`;
- prevent generic clock, projection, launch, or client setting logic from deriving shared behavior
  directly from replay/dev/lab mode names when a neutral policy applies;
- allow product-mode references at setup and routing edges where product identity is real;
- prevent new `Game` snapshot diagnostic flags that are driven by starting loadout or product mode
  instead of projection options;
- keep lab mutation through public `Game` lab APIs and never through lobby-side sim internals;
- keep protocol mirrors and docs synchronized when capability metadata changes wire shape.

Do not add broad allowlists that merely bless the current leakage. A guardrail is useful only if it
would catch the next accidental product-mode shortcut.

## Work Streams For The Future Phase Plan

The later multi-phase plan should probably decompose this by risk, not by module:

- Inventory mode-shaped behavior and add characterization tests before moving code.
- Extend `SessionPolicy` with the missing neutral capabilities and document supported bundles.
- Move movement debug path inclusion behind diagnostic policy.
- Neutralize tick-control decisions enough that realtime vs room-controlled time is the policy, with
  pause, speed, step, and seek treated as operations under that policy.
- Move client debug overlay availability and future lab/replay control affordances to capability
  metadata.
- Clean up remaining compatibility names where safe, and document the names that intentionally remain
  for wire compatibility.
- Add focused guardrails after each boundary is actually stable.

The phase plan should keep PRs small. Prefer one capability boundary per phase when possible, with a
focused test proving no normal, replay, branch, dev, or lab behavior changed unintentionally.

## Non-Goals

- Do not redesign the lab product itself in this plan.
- Do not remove existing replay/dev protocol messages just for naming purity.
- Do not turn every possible capability combination into a supported product mode.
- Do not move room lifecycle, transport, database writes, or AI ownership into `rts-sim`.
- Do not weaken fog or client-trust boundaries while adding full-world vision or diagnostic
  capability.
- Do not replace simple enums and structs with a dynamic plugin system.

## Acceptance Criteria

This follow-up is complete when:

- A future engineer can describe each room mode as a product identity plus an explicit policy bundle.
- Generic helpers no longer attach shared behavior to replay/dev/lab names when a neutral capability
  would describe the behavior.
- Diagnostics are controlled by room projection/diagnostic policy and reflected to the client as a
  capability, not inferred from `DebugHuman` or dev scenario identity.
- Clock behavior is internally represented as fixed realtime ticking or room-controlled time, even if
  compatibility protocol names remain.
- Client settings and overlays consume explicit capability metadata for diagnostics and lab/replay
  affordances.
- The docs name which compatibility terms remain intentionally and why.
- Focused tests cover normal match, spectator, replay viewer, replay branch live, dev scenario, and
  lab-relevant policy classification.

## Verification Expectations

Because this plan will change architecture rather than gameplay balance, each future phase should
run the smallest relevant focused tests plus `git diff --check`. Likely high-signal checks include:

- `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy`
- `cargo test --manifest-path server/Cargo.toml -p rts-server projection`
- `cargo test --manifest-path server/Cargo.toml -p rts-server tick`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `node tests/protocol_parity.mjs` for protocol-facing payload or snapshot changes
- `node tests/client_contracts.mjs` for client capability and overlay behavior
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` when
  changing public `Game` snapshot APIs or sim/lobby boundaries.

Manual smoke should stay targeted: one normal match, one spectator view, one replay with vision
selection, one branch live launch, one dev scenario with pause/step/debug overlays, and one lab room
flow when the affected capability reaches lab.

## Guidance For The Phase-Plan Agent

Do not convert this directly into one giant phase. Start by auditing the exact current leakage on
latest `main`, then cut phases around behavior-preserving seams. If a phase needs a protocol
migration, make the compatibility story explicit and include both server and client tests.

Each resulting phase file should follow the repo convention in `plans/README.md`: scope, expected
touch points, verification, manual test focus, and handoff expectations. Each implementation phase
should use its own `zvorygin/` branch, open an owned PR, arm auto-merge, and wait until the phase
head is reachable from `origin/main` before the next phase starts.

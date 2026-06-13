# Phase 3 - Assignment, Lifecycle, and Command Identity Guardrails

Status: Designed, not implemented.

## Objective

Make every faction assignment path explicit before faction-specific resources, starts, or client UI
are expanded. This phase should close the gaps where a non-default faction could be created by a
dev path, replay branch, quickstart, AI slot, hotkey profile, or stale client descriptor without a
clear validation rule.

## Scope

- Add and maintain a faction lifecycle matrix that lists every match creation and playback path:
  normal lobby start, quickstart/debug start, AI add/remove/start, fixture/dev faction start,
  replay playback, replay branch staging and launch, dev scenarios, self-play, match history replay,
  spectator/no-fog viewing, and post-match replay.
- Define the authoritative faction assignment source for each path:
  - default hidden current-faction assignment for normal play
  - explicit test/dev fixture assignment for architecture coverage
  - explicit rejection or fallback for unsupported AI faction assignment
  - replay/branch assignment copied from the recorded schema, not recomputed from client state
- Add a server-side validation helper that resolves requested faction ids into one of:
  accepted playable faction, accepted fixture/dev-only faction, default current faction, or rejected
  unsupported faction.
- Keep normal lobby faction selection hidden or disabled unless a later phase exposes it; do not
  leave Phase 10 dependent on an unnamed dev path.
- Define stable command identity rules before client faction menus expand:
  command ids must include enough namespace to distinguish command kind, faction, entity kind,
  upgrade id, and ability id where collisions are possible.
- Update hotkey profile validation/import behavior so commands for unavailable factions can be
  preserved as unresolved/inactive instead of corrupting active profiles.
- Add AI fail-closed tests for every path that can create or assign a seat.
- Add prediction fail-closed tests proving unsupported factions disable prediction before the WASM
  adapter is initialized.
- Do not add real second-faction gameplay content in this phase.

## Expected Touch Points

- `plans/faction/` for the lifecycle matrix
- `server/src/lobby/`
- `server/crates/sim/src/game/setup.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/`
- `server/crates/sim/src/game/replay.rs`
- `server/crates/ai/src/` only for validation/restriction seams
- `client/src/hotkey_profiles.js`
- `client/src/state.js`
- `client/src/sim_wasm_adapter.js`
- `tests/`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`

## Verification

- Faction lifecycle matrix added and referenced by this plan.
- Server integration or focused Rust tests for default lobby assignment, fixture/dev assignment,
  AI rejection, quickstart/debug defaults, replay playback assignment, replay branch assignment, and
  dev scenario assignment.
- Hotkey profile tests proving unresolved commands from unavailable factions do not break active
  current-faction hotkeys.
- Prediction-disable test proving unsupported factions do not initialize WASM prediction.
- Protocol/replay tests proving faction assignment is loaded from recorded schema for replay and
  branch flows.

## Manual Testing Focus

Start a normal match and confirm it still defaults to the current faction. If a fixture/dev path is
exposed, start it explicitly and confirm unsupported AI/prediction states are clearly blocked.

## Handoff Expectations

The handoff must name the lifecycle matrix file, the validation helper, every faction assignment
source, the command-id namespace rule, hotkey unresolved-command behavior, and the exact dev path
later phases should use for fixture or second-faction starts before normal lobby selection exists.

## Player-Facing Outcome

No intended gameplay change. Faction choice remains hidden for normal play, but internal/dev/test
paths now have explicit assignment, validation, hotkey, AI, replay, and prediction behavior.

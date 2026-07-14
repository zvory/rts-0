# Phase 2 - Application Ownership

## Phase Status

- [x] Done.

## Objective

Give commands and session semantics one explicit application-layer owner without also changing
subprocess, dependency, private-server, or media behavior. This phase should make an ordinary command
change local and obvious while keeping failures attributable to command routing, validation, and
ordering rather than external-resource lifecycle.

## Target Dependency Shape

```text
cli / daemon entry points
          |
command registry + command service + session coordinator
          |
driver + existing capture / preview / runtime helpers
```

Entry points own transport and process signals. The application layer owns command definitions,
validation, session state, aliases, and semantic execution ordering. The driver owns browser/page
operations and resource-local completion, but no generic command queue.

## Work

### Define each public command once

- Add a static `command_registry.mjs` containing, for each public command:
  - name and daemon/session scope;
  - execution lane;
  - ordinary versus lifecycle/media timeout class;
  - input validator/parser reference;
  - handler key;
  - help descriptor and example.
- Project CLI recognition/help, daemon/runtime timeout choice, service routing, and lane selection
  from that registry. Remove separately maintained public-command lists in the same commit; do not
  land another temporary duplicate.
- Move exact and bounded command input parsing into a focused `command_inputs.mjs` if that keeps the
  registry readable. Preserve runtime validation and structured error details; do not add Zod,
  codegen, a dynamic plugin system, or TypeScript in this phase.

### Give semantic ordering one owner

- Add a small `session_coordinator.mjs` that owns the sole semantic FIFO and queue draining during
  close. Select behavior from registry lane metadata rather than command-name conditionals.
- Start with these explicit lanes and change them only with contract evidence:
  - `serialized`: ordinary session mutation/inspection, time/camera, screenshot, media start/stop,
    setup transfer, and fixed capture;
  - `observation`: daemon status, `record-wait`, and existing safe capture progress;
  - `cancellation`: `capture-cancel`;
  - `lifecycle`: open, close, and shutdown.
- Remove the generic `operationTail`/`enqueue()` semantic queue from `driver.mjs` in the same change
  that installs the coordinator and lane tests. Resource-local completion promises, encoder
  backpressure, watchdogs, and idempotent capture finalization remain with their resource owner.
- Make `daemon.mjs` the sole owner of `SIGINT`, `SIGTERM`, and `SIGHUP`; remove process-level signal
  handling from the driver.

### Add the application architecture ratchet

- Add `scripts/check-interact-architecture.mjs` and wire it into focused static checks and suite
  selection.
- Enforce only the application rules established in this phase:
  - every public command is defined exactly once in the registry with complete metadata;
  - CLI/daemon/application imports follow the intended downward direction;
  - the session coordinator is the only generic semantic queue owner;
  - daemon is the sole process-signal owner;
  - post-extraction `command_service.mjs` and `driver.mjs` stay below ratcheted limits chosen from
    their final Phase 2 sizes with modest headroom.
- Document the registry, execution lanes, queue ownership, and dependency shape in
  `docs/interact-cli.md`.

## Scope Boundary for Phase 3

Do not add a general process runner, extract private-server startup, relocate npm dependencies, or
convert Cargo/FFmpeg/ffprobe/Tailnet calls in this phase. Preserve current external-resource behavior
behind the new application boundary; Phase 3 changes those adapters after command semantics are
settled and protected.

## Expected Touch Points

- `scripts/interact/cli.mjs`
- `scripts/interact/daemon.mjs`
- `scripts/interact/runtime.mjs`
- `scripts/interact/command_service.mjs`
- `scripts/interact/command_help.mjs`
- `scripts/interact/driver.mjs`
- new `scripts/interact/command_registry.mjs`
- new `scripts/interact/command_inputs.mjs`
- new `scripts/interact/session_coordinator.mjs`
- new `scripts/check-interact-architecture.mjs`
- focused `tests/interact_*.mjs`
- `tests/run-all.sh`
- `tests/select-suites.mjs`
- `docs/interact-cli.md`

## Implementation Checklist

- [x] Establish the static command registry and focused input validators.
- [x] Derive recognition, routing, lanes, timeouts, and help from the registry.
- [x] Move semantic ordering to the session coordinator and remove the driver queue atomically.
- [x] Make daemon the sole signal owner.
- [x] Add lane tests using controllable fake driver promises.
- [x] Add and wire the application architecture checker.
- [x] Document the final registry, lane, queue, and import ownership model.
- [x] Leave external-process, private-server, dependency, and media behavior for Phase 3.
- [x] Mark this phase done in this file in the implementation commit.

## Verification

```bash
node scripts/check-interact-architecture.mjs
node scripts/check-source-file-sizes.mjs
node tests/interact_cli_contracts.mjs
node tests/interact_driver_contracts.mjs
node tests/interact_bulk_contracts.mjs
node tests/interact_artifact_contracts.mjs
node tests/interact_recording_contracts.mjs
node tests/interact_fixed_capture_contracts.mjs
node tests/interact_tailnet_preview_contracts.mjs
node tests/select-suites.mjs --verify
node tests/interact_cli_smoke.mjs
node scripts/check-docs-health.mjs
git diff --check
```

Use controllable fake driver promises to prove serialized commands remain ordered, observation work
does not wait behind the semantic FIFO where explicitly allowed, `record-wait` preserves its current
non-blocking behavior, `capture-cancel` reaches active fixed capture promptly, and close drains work
according to the documented lane contract.

## Acceptance Criteria

- Registry coverage proves every public command has exactly one scope, lane, timeout class, validator,
  handler key, and help descriptor.
- CLI recognition/help, runtime timeout selection, service routing, and coordinator lanes derive from
  the registry; no separately maintained public-command list remains.
- Invalid fields and current size/count/range bounds fail before driver work begins.
- The session coordinator is the only semantic FIFO owner; the driver has no generic queue.
- Observation, cancellation, and lifecycle behavior pass focused fake-driver concurrency contracts.
- Only the daemon installs process signals.
- Application import, registry, queue, signal, and size ratchets pass and select for Lab source changes.
- The Phase 1 scripted workflow passes without changing its semantic assertions.

## Manual Test Focus

Run the ordinary open/catalog/spawn/inspect/order/time-pause/time-step/camera/screenshot/close/shutdown
workflow. During a short recording, confirm `record-wait` and an allowed camera command behave as
documented; during fixed capture, confirm `capture-cancel` remains reachable.

## Non-Goals

- Async subprocess conversion, private-server extraction, dependency relocation, or cold-open
  cancellation; those are Phase 3.
- The deep rename, TypeScript, a Rust rewrite, or a new client automation facade.
- A DI framework, generated schema framework, command plugin system, generalized artifact store, or
  full error hierarchy.
- New commands, public behavior redesign, cross-platform IPC, or public service exposure.

## Handoff Expectations

Report the final registry projections, lane assignments, removed duplicate lists/queues, signal owner,
module sizes, and exact application ratchets. Tell the Phase 3 agent to preserve these application
semantics while changing external adapters, and name the ordinary workflow plus record-wait and
capture-cancel behavior for manual re-testing.

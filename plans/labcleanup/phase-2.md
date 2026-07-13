# Phase 2 - Explicit Boundaries and Responsive Operations

## Phase Status

- [ ] Not started.

## Objective

Turn the two proof-of-concept hotspot modules into a small layered application without redesigning
the entire tool. Establish a single source for command definitions, a single owner of semantic
ordering, and responsive/cancellable ownership of long-running child processes while preserving the
Phase 1 behavior and all authority/security constraints.

## Target Dependency Shape

```text
cli / daemon entry points
          |
command registry + command service + session coordinator
          |
driver (browser/page adapter) + private server + capture modules + preview/artifact helpers
          |
process runner / filesystem / Puppeteer / FFmpeg / Rust server
```

Dependencies point downward. Entry points own transport and process signals; the application layer
owns commands and session semantics; adapters own external resources and never import the command
service or entry points.

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
  from that registry. Remove separately maintained public-command name lists.
- Move exact and bounded command input parsing into a focused `command_inputs.mjs` if that keeps the
  registry readable. Preserve runtime validation and structured details; do not add Zod, codegen, or
  a dynamic plugin mechanism.

### Give semantic ordering one owner

- Add a small `session_coordinator.mjs` that owns the sole semantic FIFO and queue draining during
  close. Select behavior from registry lane metadata rather than command-name conditionals.
- Start with these explicit lanes and change them only with test evidence:
  - `serialized`: ordinary session mutation/inspection, time/camera, screenshot, media start/stop,
    setup transfer, and fixed capture;
  - `observation`: daemon status, `record-wait`, and existing safe capture progress;
  - `cancellation`: `capture-cancel`;
  - `lifecycle`: open, close, and shutdown.
- Remove the generic `operationTail`/`enqueue()` semantic queue from `driver.mjs`. Resource-local
  completion promises, encoder backpressure, watchdogs, and idempotent capture finalization may stay
  with the resource that owns them.
- Make `daemon.mjs` the sole owner of `SIGINT`, `SIGTERM`, and `SIGHUP`; remove process-level signal
  handling from the driver.

### Make long operations asynchronous and cancellable

- Add a small `process_runner.mjs` around asynchronous `spawn` for finite child processes. It must
  provide bounded stdout/stderr capture, explicit timeout, `AbortSignal`, TERM then bounded KILL
  fallback, deterministic result/error projection, and no shell invocation.
- Add `private_server.mjs` to own loopback URL reuse/validation, ephemeral port allocation, Cargo
  build, Rust server launch/health polling, server logs, build metadata, and child teardown.
- Give an in-progress open an `AbortController`; shutdown must abort cold startup before waiting for
  it. A slow Cargo build or dependency operation must not make shutdown wait for the normal startup
  timeout.
- Convert long finite daemon-path operations to the asynchronous runner: Cargo build, any remaining
  dependency hydration, FFmpeg/ffprobe capability/probe/post-processing steps, fixed-capture finite
  post-processing, and Tailnet status resolution. Specialized streaming encoders may continue using
  direct `spawn`, and bounded filesystem operations or pre-request Git inspection may remain
  synchronous.
- Make the Lab/Chrome runtime dependency an explicit repository-owned dependency and remove daemon-
  time `npm ci`/self-install behavior. Reuse the same declared dependency from browser tests rather
  than borrowing an implicit test-only installation.
- Keep `driver.mjs` focused on browser/page session ownership and page RPC. Do not split every capture
  or artifact helper unless the split is necessary to achieve the responsibilities above.

### Add a narrow architecture ratchet

- Add `scripts/check-lab-interact-architecture.mjs` and wire it into focused static checks and suite
  selection.
- Enforce only high-value constraints:
  - daemon is the sole process-signal owner;
  - daemon request-path modules do not use `spawnSync`/`execSync`;
  - entry/application/adapter imports do not point upward;
  - external-process and private-server adapters do not import CLI, daemon, or command service;
  - every public command occurs exactly once in the registry;
  - post-extraction `command_service.mjs` and `driver.mjs` stay below ratcheted limits chosen from
    their final Phase 2 sizes with modest headroom.
- Document the resulting dependency shape, lanes, subprocess cancellation, and dependency ownership
  in `docs/lab-interact-cli.md`.

## Expected Touch Points

- `scripts/lab-interact/cli.mjs`
- `scripts/lab-interact/daemon.mjs`
- `scripts/lab-interact/runtime.mjs`
- `scripts/lab-interact/command_service.mjs`
- `scripts/lab-interact/command_help.mjs`
- `scripts/lab-interact/driver.mjs`
- new `scripts/lab-interact/command_registry.mjs`
- new `scripts/lab-interact/command_inputs.mjs`
- new `scripts/lab-interact/session_coordinator.mjs`
- new `scripts/lab-interact/process_runner.mjs`
- new `scripts/lab-interact/private_server.mjs`
- `scripts/lab-interact/recording.mjs`
- `scripts/lab-interact/fixed_capture.mjs`
- `scripts/lab-interact/tailnet_preview.mjs`
- new `scripts/check-lab-interact-architecture.mjs`
- repository npm manifest/lock and browser dependency loader
- focused `tests/lab_interact_*.mjs`
- `tests/run-all.sh`
- `tests/select-suites.mjs`
- `docs/lab-interact-cli.md`

## Implementation Checklist

- [ ] Establish the static command registry and focused input validators.
- [ ] Derive routing, lanes, timeouts, and help from the registry.
- [ ] Move semantic ordering to the session coordinator and remove the driver queue.
- [ ] Make daemon the sole signal owner.
- [ ] Add the bounded/cancellable asynchronous process runner.
- [ ] Extract private-server startup/lifecycle and make cold open abortable.
- [ ] Remove daemon-path long synchronous process work and runtime dependency installation.
- [ ] Add responsive status/shutdown/media-process contract tests.
- [ ] Add and wire the narrow architecture checker.
- [ ] Document the final module/lane/process ownership model.
- [ ] Mark this phase done in this file in the implementation commit.

## Verification

```bash
node scripts/check-lab-interact-architecture.mjs
node scripts/check-source-file-sizes.mjs
node tests/lab_interact_cli_contracts.mjs
node tests/lab_interact_driver_contracts.mjs
node tests/lab_interact_bulk_contracts.mjs
node tests/lab_interact_artifact_contracts.mjs
node tests/lab_interact_recording_contracts.mjs
node tests/lab_interact_fixed_capture_contracts.mjs
node tests/lab_interact_tailnet_preview_contracts.mjs
node tests/select-suites.mjs --verify
node tests/lab_interact_cli_smoke.mjs
node scripts/check-docs-health.mjs
git diff --check
```

Use controllable fake child processes in contracts to prove:

- status promptly reports `opening: true` while a Cargo/dependency child remains open;
- shutdown aborts cold open and reaps the child without waiting for the ordinary startup timeout;
- daemon status remains responsive while a finite media-tool stage is held open;
- `record-wait` does not block unrelated allowed work; and
- `capture-cancel` remains responsive during fixed capture.

## Acceptance Criteria

- Registry coverage proves every command has exactly one scope, lane, timeout class, validator,
  handler key, and help descriptor.
- Invalid fields and current size/count/range bounds fail before driver work begins.
- The session coordinator is the only semantic FIFO owner; the driver has no generic queue.
- Daemon status and cancellation remain responsive during the deliberately slow process cases above.
- Shutdown aborts and reaps in-progress cold startup, and only the daemon installs process signals.
- No long `spawnSync`/`execSync` remains in architecture-checked daemon request-path modules.
- Private servers remain loopback-only/capability-enabled and reused non-loopback URLs are rejected.
- The Phase 1 scripted workflow passes without changing its semantic assertions.
- Architecture and size ratchets pass and select automatically for Lab source changes.

## Manual Test Focus

Run the ordinary open/catalog/spawn/inspect/order/time-pause/time-step/camera/screenshot/close/shutdown
workflow.
During a cold open, issue `status` from another terminal and confirm it responds while Cargo is still
building; then start one short recording and one short fixed capture to confirm media and preview
behavior still work.

## Non-Goals

- The deep rename, TypeScript, a Rust rewrite, or a new client automation facade.
- A DI framework, schema-generation framework, command plugin system, or generalized artifact store.
- Redesigning codecs/capture output, decomposing every helper, or unifying every error class.
- Async conversion of every bounded filesystem operation or pre-request inspection.
- New commands, cross-platform IPC, public service exposure, or broad behavior changes.

## Handoff Expectations

Report the final module dependency shape, registry lane assignments, removed duplicate lists/queues,
which subprocess paths became cancellable, dependency ownership, and exact architecture ratchets.
Tell the Phase 3 agent to translate these boundaries rather than redesign them, and name the ordinary
workflow plus cold-open status/shutdown and short media checks for manual re-testing.

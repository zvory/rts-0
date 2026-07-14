# Phase 3 - Responsive External Adapters

## Phase Status

- [x] Done.

## Objective

Make long-running external work responsive and cancellable behind the Phase 2 application boundary.
Extract only the process and private-server ownership needed to keep status, cancellation, and
shutdown responsive; leave browser/page, capture format, and artifact behavior otherwise intact.

## Target Dependency Shape

```text
command service + session coordinator
          |
driver (browser/page operations) + private server + capture / preview helpers
          |
process runner / filesystem / Puppeteer / FFmpeg / Rust server / Tailscale
```

Adapters own their external resources and never import the command service, coordinator, daemon, or
CLI. The service owns the abort signal for an in-progress open; the private-server/process adapters
honor it and deterministically reap their children.

## Work

### Add a bounded asynchronous process runner

- Add `process_runner.mjs` around asynchronous `spawn` for finite child processes. It must provide:
  - bounded stdout/stderr capture;
  - explicit timeout and `AbortSignal` support;
  - TERM followed by bounded KILL fallback;
  - deterministic result/error projection;
  - direct argv execution with no shell.
- Keep specialized streaming encoders on direct `spawn` when they need stdin/backpressure and their
  own finalization. Bounded filesystem operations and pre-request Git inspection may remain
  synchronous.

### Extract private-server lifecycle and cold-open cancellation

- Add `private_server.mjs` to own loopback URL reuse/validation, ephemeral port allocation, Cargo
  build, Rust server launch/health polling, server logs, build metadata, and child teardown.
- Give an in-progress open an application-owned `AbortController`; shutdown must abort cold startup
  before awaiting it. Keep `ProcessRunner`, `PrivateServer`, and startup abort/reaping tests in the
  same commit so there is no uncancellable intermediate ownership state.
- Keep the driver focused on browser/page session ownership and page RPC after delegating private-
  server lifecycle.

### Convert the high-value blocking paths

- Move long finite daemon request-path work to the asynchronous runner:
  - Cargo build;
  - FFmpeg/ffprobe capability, finite probe, and post-processing stages;
  - fixed-capture finite post-processing/probe;
  - Tailnet status resolution.
- Keep the long-running Rust server on direct `spawn` owned by `private_server.mjs`; its health
  polling and teardown belong to that adapter rather than the finite-child runner.
- Make the Puppeteer runtime library an explicit repository-owned dependency and remove daemon-time
  dependency hydration/`npm ci` behavior. Move the package/lock dependency and update the Lab loader
  plus browser tests atomically so no caller borrows an implicit test-only installation.
- Do not decompose every media/capture/artifact module or normalize every external error. Stop once
  the held-open process tests are responsive and ownership is explicit.

### Extend the architecture ratchet

- Extend `scripts/check-interact-architecture.mjs` with adapter rules:
  - defined daemon request-path modules contain no `spawnSync`/`execSync`; documented bounded
    pre-request exceptions live outside that checked path;
  - process/private-server/media adapters never import application or entry-point modules upward;
  - only the process/private-server or specialized streaming owner manages each request-path/tool
    child; `cli.mjs` remains the explicit daemon-bootstrap owner;
  - driver and adapter size limits are ratcheted from the final split with modest headroom.
- Document process ownership, cold-open cancellation, dependency installation, and the intentional
  remaining synchronous exceptions in `docs/interact-cli.md`.

## Expected Touch Points

- `scripts/interact/command_service.mjs`
- `scripts/interact/driver.mjs`
- new `scripts/interact/process_runner.mjs`
- new `scripts/interact/private_server.mjs`
- `scripts/interact/recording.mjs`
- `scripts/interact/fixed_capture.mjs`
- `scripts/interact/tailnet_preview.mjs`
- `scripts/check-interact-architecture.mjs`
- repository npm manifest/lock and browser dependency loader
- focused `tests/interact_*.mjs`
- `tests/run-all.sh`
- `tests/select-suites.mjs`
- `docs/interact-cli.md`

## Implementation Checklist

- [x] Add the bounded/cancellable asynchronous process runner.
- [x] Extract private-server startup/lifecycle and make cold open abortable.
- [x] Move high-value long finite request-path process work to the runner.
- [x] Establish explicit repository ownership for the browser runtime dependency.
- [x] Remove runtime dependency installation/hydration.
- [x] Add slow-child status, shutdown/reaping, media, and cancellation contracts.
- [x] Extend architecture and size ratchets for adapters and blocking process work.
- [x] Document process, dependency, cancellation, and remaining synchronous ownership.
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

Use controllable fake children to prove:

- status promptly reports `opening: true` while a Cargo build child remains open;
- shutdown aborts cold open, sends TERM, uses bounded KILL only when required, and reaps the child
  without waiting for the ordinary startup timeout;
- daemon status remains responsive while a finite media-tool stage is held open;
- `capture-cancel` remains responsive during fixed capture; and
- timeout/abort output is bounded and does not invoke a shell.

## Acceptance Criteria

- Status and allowed cancellation remain responsive during deliberately slow external-process cases.
- Shutdown aborts and reaps in-progress cold startup deterministically.
- No long `spawnSync`/`execSync` remains in architecture-checked daemon request paths; documented
  bounded exceptions remain outside request handling.
- Private servers remain loopback-only and capability-enabled, and reused non-loopback URLs remain
  rejected.
- Runtime dependency installation is gone and Lab/browser tests use one declared repository-owned
  browser runtime dependency.
- Adapter import/child ownership/size ratchets pass and select for Lab source changes.
- Phase 2 lane/registry contracts and the Phase 1 scripted workflow remain green.

## Manual Test Focus

Run the ordinary open/catalog/spawn/inspect/order/time-pause/time-step/camera/screenshot/close/shutdown
workflow. During a cold open, issue `status` from another terminal, then request shutdown and confirm
the Cargo/private-server child exits promptly; run one short recording and fixed capture to confirm
media and preview output remain intact.

## Non-Goals

- TypeScript or the deep rename; TypeScript is Phase 4 after this adapter boundary lands.
- A Rust rewrite, new client automation facade, DI framework, generalized artifact store, or full
  external-error hierarchy.
- Codec/capture format redesign, decomposition of every helper, async conversion of every bounded
  filesystem/pre-request operation, or new public commands.
- Cross-platform IPC, public service exposure, golden images, performance certification, or long
  media soak testing.

## Handoff Expectations

Report the final adapter dependency shape, which subprocess paths became asynchronous/cancellable,
child termination/reaping behavior, dependency ownership, remaining synchronous exceptions, and
exact adapter ratchets. Provide handoff evidence for the ordinary workflow, cold-open
status/shutdown, cancellation, and short media checks. Tell the Phase 4 agent to translate the
settled modules and ports rather than redesigning them.

# Phase 1 - Reusable Lab Interact Driver

Status: done.

Migration note: this status records completed driver, bridge, and authoritative-control work. Phase
0 must deeply rename that implementation and place it behind the CLI daemon; no legacy agent-tool
name or transport is part of the current supported contract.

## Goal

Provide a transport-independent `LabInteractDriver` that opens and controls a small authoritative
Lab scene in an explicitly selected Git worktree. The driver proves process lifecycle, typed
control, state observation, and teardown without owning the public CLI or media capture.

## Delivered Scope

- A Node-owned driver validates an explicit workspace/worktree root, records branch/head facts, and
  starts or deliberately reuses a private server from that checkout on an ephemeral loopback port.
- Headless Chrome runs with a fixed viewport and device-pixel ratio, a private profile, pointer-lock
  suppression, and a generated safe room id.
- A narrow in-page `LabInteractBridge`, installed only behind the agent-tool launch gate, is composed
  by the app shell and delegates to existing `App`, `Match`, `LabClient`, camera, and state APIs.
- The bridge exposes only typed task operations: readiness/status, bounded catalog and inspection,
  setup mutations, `issueCommandAs`, room-time controls, and camera state. It does not expose raw
  evaluation or internal object references.
- Catalog discovery shares the playable faction/unit/building mirror used by the human Lab spawn
  palettes rather than scraping DOM.
- Driver methods cover opening, status, reset, close, catalog, spawn, update, remove, order, time,
  inspect, and camera operations with authoritative outcome/tick evidence.
- Readiness requires WebSocket connection, start payload, Lab operator role, first applied
  snapshot, required room-time state, and an error-free page frame loop.
- Paused mutations step as necessary and wait for the authoritative snapshot containing the
  accepted result before completing.
- Browser/server logs are bounded under ignored output, and idempotent close/signal/error handlers
  own the private server, browser, profile, timers, and pending operations.

## Driver Shape After Phase 0 Rename

```js
const driver = await LabInteractDriver.open({ workspaceRoot, map, seed, scenario, viewport });
await driver.catalog(query);
await driver.spawn(spec);
await driver.update(operation);
await driver.remove(refs);
await driver.order({ playerId, command });
await driver.time(control);
await driver.inspect(query);
await driver.camera(command);
await driver.reset();
await driver.close();
```

Numeric ids remain valid at this layer. Alias ownership and persistence across commands belong to
the daemon adapter established by Phase 0.

## Delivered Touch Points

- the focused driver/helper area, renamed by Phase 0 under `lab-interact`
- app-shell bridge composition and launch gating in the client
- the shared Lab spawn/catalog helper
- reusable private-server/browser runtime helpers and pinned local tooling dependencies
- focused Node/client/driver contracts and a private-server browser smoke
- client UI design/context and testing documentation

## Constraints

- The driver must not add production HTTP control, bypass Lab operator checks, accept direct
  checkpoint JSON, or infer success from a successful send alone.
- App-shell dependency injection remains intact; Lab transport or UI modules must not enter
  renderer/input/model areas.
- Selected-worktree correctness is blocking and must remain covered.
- Step counts, seek targets, inspection limits, log tails, wait times, and pending operations stay
  bounded.
- Phase 0, not this historical phase, owns daemon auto-start, aliases, CLI parsing, 30-minute idle
  teardown, deep rename, and deletion of the superseded adapter.

## Verification To Preserve During Migration

- Workspace validation, safe room/output naming, process state, timeout/error normalization, and
  bounded inspection query tests.
- Client contracts for the explicit launch gate, bridge API, catalog parity, and bridge
  teardown/reload.
- A private-server browser smoke that opens a blank Lab, pauses, spawns two entities, observes
  authoritative state, issues a normal command, steps until visible, focuses, resets, and closes
  without page/frame errors.
- `node scripts/check-client-architecture.mjs`, focused client contracts, and suite-selection
  verification under their post-rename paths.

## Manual Testing Focus

- Open from a non-main worktree with an obvious visual change and confirm branch/head correctness.
- Spawn, inspect, order, pause/step, reset, and close without manual Chrome interaction.
- Interrupt the driver and confirm the browser, server, profile, and pending operations are cleaned.

## Handoff Record

The underlying driver/bridge seam, readiness conditions, process ownership, and authoritative
confirmation behavior are complete. Phase 0 must preserve this evidence while renaming the public
and internal agent-tool identities, making the driver a single persistent daemon-owned instance,
and proving explicit plus idle teardown through the CLI.

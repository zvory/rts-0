# Phase 1 - Reusable Agent Lab Driver

Status: done.

## Goal

Create a transport-independent `AgentLabDriver` that can open and control a small authoritative lab
scene in an explicitly selected Git worktree. The phase should prove process lifecycle, typed
control, state observation, and teardown without committing the product to MCP schemas or media
capture yet.

## Scope

- Add a Node-owned driver under a focused agent-lab tooling area. It should expose programmatic
  methods rather than parse a broad CLI flag vocabulary.
- Accept an explicit workspace/worktree root, resolve it with Git, and reject a path that is not a
  valid checkout of this repository. Record the root, branch, and head SHA in the session.
- Start or deliberately reuse a private server on an ephemeral loopback port from that worktree.
  Reuse the health checking, Chrome discovery, Puppeteer loading, private profile, and child-process
  cleanup patterns already proven by `scripts/client-perf-harness.mjs` rather than copying large
  chunks blindly.
- Launch headless Chrome with a fixed default viewport and device-pixel ratio, disable automatic
  pointer lock, and open a direct blank or bundled `/lab` URL with a generated safe room id.
- Add a narrow in-page `AgentLabBridge`, installed only for an explicit agent-lab launch parameter,
  that is composed by the app shell and delegates to existing `App`, `Match`, `LabClient`, camera,
  and state APIs. The launch parameter is a discoverability/lifecycle gate, not a substitute for
  server authorization.
- Keep the bridge's API typed and task-specific. It may expose readiness/status, catalog data,
  setup mutations, issue-as commands, room-time controls, bounded inspection, and camera state, but
  it must not expose arbitrary page evaluation or internal object references to the future MCP
  layer.
- Make catalog discovery reuse the same playable faction/unit/building mirror as the human lab
  spawn palettes. Extract a small pure catalog helper if needed instead of making automation scrape
  LabPanel DOM.
- Implement driver methods for:
  - opening, status, reset, and close;
  - listing maps, players, factions, spawnable units/buildings, upgrades, and supported command
    kinds in bounded structured form;
  - spawning, deleting, moving, reassigning, resource/research/god-mode mutation through existing
    lab requests;
  - issuing existing protocol commands through `issueCommandAs`;
  - pausing, resuming, changing speed, stepping a bounded number of ticks, and seeking within the
    retained lab timeline;
  - inspecting bounded entity/player/room/camera state with filters and result limits;
  - setting or focusing the camera without taking a capture.
- Define authoritative readiness explicitly: WebSocket connected, start payload received, lab
  operator role confirmed, first snapshot applied, room-time state known where required, and page
  frame loop free of errors.
- Handle the paused-mutation edge correctly. When a setup mutation is accepted while room time is
  paused, step as needed and wait for the authoritative snapshot containing the returned entity or
  mutation before reporting completion.
- Put browser/server logs in the session's ignored target directory or temporary directory with
  bounded tails in errors. Do not write source files.
- Ensure `close()` and signal/exception handlers terminate the private server, browser, browser
  profile, timers, and pending operations idempotently.

## Initial Driver Shape

The exact module split may change during implementation, but the stable concepts should resemble:

```js
const driver = await AgentLabDriver.open({ workspaceRoot, map, seed, scenario, viewport });
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

Keep numeric ids at this layer; session aliases are an MCP-facing concern in Phase 2. Return the
server's typed outcome plus the tick/snapshot evidence that confirms the client observed it.

## Expected Touch Points

- a focused driver/helper area under `scripts/` or `tools/agent-lab/`
- `client/src/app.js`, `client/src/bootstrap.js`, and a new app-shell bridge module
- a small shared lab spawn/catalog helper extracted from `client/src/lab_panel.js` if needed
- `scripts/client-perf-harness.mjs` only if a small reusable browser-runtime helper can be extracted
  without destabilizing the performance harness
- `tests/package.json` and lockfile for pinned local tooling dependencies if the driver cannot reuse
  the existing dependency set cleanly
- focused Node client/driver contract tests and one private-server browser smoke
- `docs/design/client-ui.md` for the bridge's exported/composed contract
- `docs/context/client-ui.md` and `docs/context/testing.md` if their code maps or commands change

## Constraints

- Do not add MCP configuration, MCP tool names, screenshots, videos, artifact export/import, or
  agent workflow guidance in this phase.
- Do not add a production HTTP remote-control route. The driver controls the normal browser client
  and existing lab protocol over loopback.
- Do not import lab transport or UI modules into renderer/input/model areas; keep app-shell
  composition and dependency injection intact.
- Do not bypass lab operator checks or command validation. A rejected lab op or command must remain
  rejected and return a structured driver error.
- Do not make direct checkpoint JSON an input to mutation methods.
- Do not infer success from a successful send alone. Wait for `labResult` and, for visible state,
  an authoritative snapshot/tick condition.
- Do not assume the main checkout is the desired source tree. Selected-worktree correctness is a
  blocking contract and needs automated coverage.
- Bound step counts, seek targets, inspection limits, log tails, wait times, and concurrent pending
  operations.

## Verification

- Add pure tests for workspace validation, safe room/output naming, process state transitions,
  timeout/error normalization, and bounded inspection queries.
- Add client contracts for the explicit launch gate, bridge API, catalog parity with the human lab
  palette, and bridge teardown/reload behavior.
- Add a private-server browser smoke that opens a blank lab, pauses time, spawns two entities,
  observes their authoritative ids/state, issues one normal command, steps until the order is
  visible, focuses the camera, resets, and closes with no page/frame errors.
- Run `node scripts/check-client-architecture.mjs`.
- Run `node tests/client_contracts.mjs` or the narrow contract entry point selected by the changed
  files.
- Run the focused agent-lab driver browser smoke on a private port.
- Run `node tests/select-suites.mjs --verify` if suite selection changes.

## Manual Testing Focus

- Open from a non-main worktree containing an obvious client visual change and confirm the served
  build/head belongs to that worktree.
- Open, spawn a unit, inspect it, issue a move or attack order, pause/step, reset, and close without
  interacting with Chrome manually.
- Interrupt the driver while Chrome and the server are live and confirm both processes and the
  temporary profile are cleaned up.

## Handoff

After implementation, mark this phase done and report the driver/bridge APIs, selected-worktree
validation, dependency hydration, readiness conditions, process ownership, focused verification,
and known lifecycle gaps. Tell the Phase 2 agent which driver methods are stable enough to expose
as MCP tools and which remain internal diagnostics.

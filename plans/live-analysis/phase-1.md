# Phase 1 - Shared Observer Camera Input

Status: Planned.

## Objective

Remove replay-only camera/navigation duplication before adding more observer UI. Replay viewers
should regain middle-mouse drag panning and share the same basic camera navigation behavior as live
match views, while remaining command-free. Live spectators should keep their current inspection
behavior, active players should keep full gameplay input, and later observer analysis work should
have one stable input foundation instead of another special replay path.

## Scope

- Extract a small shared camera/navigation input collaborator from the live `Input` path, or create
  an equivalent shared helper with the same ownership model. It should own viewport mouse tracking,
  wheel zoom, pan-key state, middle-mouse drag panning, Space+left-drag panning if retained for
  parity, and listener teardown.
- Compose that helper into live `Input` and replay/observer input instead of copying wheel/key/drag
  logic between classes.
- Fix replay middle-mouse drag panning. A replay viewer middle-drag should call the same
  `Camera.panByScreenDelta` behavior used by live matches.
- Keep replay viewers command-free: no gameplay command issuer API, no build placement, no command
  hotkeys, no replay-only command surface.
- Preserve live spectator behavior unless deliberately changed and documented. Today live
  spectators use the full `Input` path for camera navigation and read-only selection inspection;
  do not accidentally remove that inspection path while fixing replay input.
- Preserve active-player behavior, including command targeting, selection, build placement,
  minimap routing, pointer lock, control groups, and command hotkeys.
- Decide explicitly whether replay viewers should gain Space+left-drag panning as part of camera
  parity. If not, document why middle-drag and wheel/key parity are sufficient.
- Update `docs/design/client-ui.md` and `docs/context/client-ui.md` so replay/observer camera input
  has an explicit module contract and does not live only as an undocumented `ReplayCameraInput`
  exception.

## Expected Touch Points

- `client/src/input/index.js`
- `client/src/input/camera_controls.js`
- new `client/src/input/*` helper if extracting camera navigation
- `client/src/replay_camera_input.js`, or a renamed observer input wrapper
- `client/src/match.js`
- `client/src/camera.js` only if the public camera navigation seam needs documentation/comments
- `client/src/input/router.js` only if shared navigation needs routed DOM/pointer-lock events
- `tests/client_contracts.mjs`
- `tests/input_context_menu_contracts.mjs` only if existing mouse routing coverage moves
- `docs/design/client-ui.md`
- `docs/context/client-ui.md`

## Verification

Run focused checks that cover input contracts and client module boundaries:

```bash
node tests/client_contracts.mjs
node tests/input_context_menu_contracts.mjs
node scripts/check-client-architecture.mjs
```

Add or update a focused client contract that proves replay middle-mouse drag pans the camera and
that replay wheel zoom still anchors on the cursor. If a shared helper is introduced, cover the
helper directly rather than only the replay wrapper.

## Manual Testing Focus

Open a replay and confirm mouse-wheel zoom, middle-mouse drag pan, keyboard/edge pan, replay speed
controls, timeline seek, and vision controls still work. Start a normal live match and confirm
middle-drag, Space+left-drag if supported, selection, right-click commands, command-card targeting,
and minimap interactions still work. Join as a live spectator and confirm camera navigation and
read-only selection inspection behave as before.

## Handoff Expectations

The handoff must name the shared camera/input module, list which gestures are guaranteed shared
between replay viewers and live match views, and call out any intentional mode differences for
replay viewers, live spectators, and active players. It should also tell Phase 4 which input module
the observer overlay must avoid interfering with.

## Player-Facing Outcome

Replay viewers regain middle-mouse drag camera panning. Camera navigation behavior becomes harder
to regress because replay and live views no longer maintain separate copies of basic camera input.

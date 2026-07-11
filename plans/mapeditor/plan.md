# Dedicated Map Editor Room Plan

## Outcome

Replace both the legacy standalone HTML editor and the Lab-embedded draft workflow with one real
Map Editor at `/map-editor`. The Map Editor is a dedicated frozen room that reuses the game's Pixi
map view and map-editing controls but does not run simulation ticks or expose Lab unit/gameplay
tools. Authors can move between Map Editor and Lab explicitly; each transition into Lab starts a
fresh empty Lab on the current edited map, and returning to the editor carries only the map.

## Product Contract

- `/map-editor` opens the Map Editor, not the current monolithic `client/map-editor.html` tool.
- The editor can create a blank map or load a bundled map, then edit terrain, player starts,
  natural bases, layouts, name, and description with undo/redo, local save/load, and JSON export.
- The editor room is frozen by policy: no simulation ticks, units, orders, resources, fog gameplay,
  AI, replay timeline, or Lab spawn/command controls.
- Map editing is the only active mode in this room. There is no draft-versus-live-map distinction
  and no user-facing `draft`, `apply draft`, `restart test`, or `run test` language.
- `Open in Lab` validates the current map and creates a fresh ordinary Lab using it. The Lab starts
  from normal map initialization and does not preserve editor or prior-Lab entities.
- `Edit map` in Lab opens that Lab's current map in `/map-editor` and discards the Lab simulation;
  units, orders, resources, elapsed time, and replay history never cross the boundary.
- The editable map survives the round trip so testing in Lab does not destroy unsaved editor work.

## Implementation

1. Add a dedicated map-editor launch/session capability and frozen room policy, reusing only the
   minimum existing Lab map validation/materialization seam. Make the route initialize from a blank,
   bundled, locally saved, or transition-supplied map without constructing a running Lab game.
2. Move the useful Lab map-editor UI and state into a Map Editor-owned screen composed around the
   normal Pixi renderer, camera, input coordinates, and map schema. Extract shared terrain/base
   editing and validation primitives instead of importing `LabPanel`, `LabClient`, or `GameState`
   into the editor.
3. Make painting responsive: update dirty tiles and adjacent terrain edges incrementally, coalesce
   a pointer stroke into one render/undo transaction, and avoid full-map cloning, serialization,
   fingerprinting, canvas regeneration, and PIXI texture replacement per painted tile.
4. Add the two transitions. Use a bounded ephemeral handoff identifier or equivalent non-URL map
   payload owned by the server/session layer; do not put full map JSON in query parameters. Starting
   Lab materializes one authoritative map snapshot, while returning to the editor transfers only
   that map and preserves the editor workspace where possible.
5. Remove the Map Editor window/session/preview wiring from Lab, remove the old `applyMapDraft`
   workflow if it has no remaining caller, and retire the legacy inline HTML editor implementation.
   Update protocol mirrors and the client/server design docs with the final ownership boundary.

## Constraints

- Keep ordinary Lab behavior unchanged apart from removing embedded map authoring and adding
  `Edit map`.
- Keep the server authoritative at the Map Editor-to-Lab boundary and validate all map size,
  terrain, start, natural, and layout inputs before creating the Lab.
- Reuse renderer/camera/map-schema primitives, not whole Lab or Match controllers. The Map Editor
  must have an explicit teardown path for renderer, input, listeners, and room/session state.
- Make transition ownership and expiry deterministic so stale handoff ids fail clearly and cannot
  address arbitrary files or rooms.
- Do not add unit preservation, live terrain mutation, path repair, a ruler, collaborative editing,
  map publishing, or map PR submission in this pass.

## Verification

- Contract tests cover route replacement, frozen-room capabilities, map editing, undo/redo,
  validation, local save/export, both transitions, handoff expiry/failure, Lab editor removal, and
  protocol parity.
- Architecture checks prove the editor uses extracted primitives without creating Lab/UI/simulation
  cross-imports, and focused server tests prove the editor never advances simulation time.
- A manual smoke loads and edits a bundled map without per-tile stalls, opens it in a fresh Lab,
  spawns a unit, returns to edit the same map with no unit state carried back, and repeats once.
- Deliver the implementation through the normal owned-PR workflow and wait for the merge gate; the
  handoff should name the focused checks and the manual smoke results.

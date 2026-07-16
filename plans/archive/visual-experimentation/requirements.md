# Visual Experimentation Requirements

This document captures requirements and developer stories for local visual experimentation in the
lab. It is not an implementation plan.

## Product Goal

Developers need a local way to compare authored visual options inside the real RTS renderer. The
workflow should make it easy to place multiple candidates in the game world, zoom and pan around
them, compare them next to real units and terrain, and decide which candidate reads best at actual
gameplay scale.

The workflow is a local comparison harness for checked-in candidate art. It is not a general custom
asset loader, upload surface, file browser, or asset management system.

The first target is entrenchment visuals, but the same model should later support tank turret
variants, unit SVG rig changes, projectile marks, ground decals, and other renderer-owned
presentation assets.

## Scope

- This is strictly for local, solo developer experimentation.
- Visual experiment state does not need to be shareable between users.
- Visual experiment state does not need to support late joiners or multiplayer consistency.
- Visual experiment entries do not need to become durable public catalog entries.
- Candidate profiles and assets are checked into the repo or explicitly registered by local code.
- Restarting the server or refreshing the browser is acceptable after changing assets or profile
  data.
- Hot reloading is not required.

## Simplified Model

- A lab URL may include one sanitized visual profile id, for example
  `/lab?scenario=entrenchment_inspection&visualProfile=trench-variants-1`.
- The URL selects a profile by id only. It must not name asset paths, SVG text, JavaScript modules,
  remote URLs, local file paths, or arbitrary image sources.
- A visual profile is a checked-in local developer profile. It may be represented as JavaScript or
  data, but it must be loaded through an explicit allowlisted registry.
- A profile may define multiple candidates, labels, renderer-only sample positions, real-entity
  visual override rules, and an initial camera target.
- A profile is immutable for the lifetime of one lab load. Editing a profile or candidate asset
  requires refresh or restart.
- Missing or invalid profile ids fail closed: the client may show a local developer error, but it
  must not try to resolve the id as a path or fetch unregistered assets.

## Requirements

### Real Renderer Context

- Experiments must render through the same PixiJS renderer used by a normal match.
- Candidates must appear in the real game world, not only in a standalone static preview.
- Candidates must be visible with normal terrain, units, buildings, shadows, fog, selection rings,
  HP bars, and camera zoom.
- Developers must be able to pan and zoom normally while inspecting candidates.
- Experiments should support an initial camera target so opening the lab lands on the comparison
  area.
- Experiments should support lightweight world-space labels for candidates, such as `A`, `B`, `C`,
  or short names, so feedback and screenshots are unambiguous.

### Checked-In Candidate Registry

- Candidate assets must be checked in or defined by checked-in local code.
- Candidate assets must be reachable only through explicit registry entries, not through raw URL or
  file-path input.
- One profile may include several candidates of the same presentation type, such as five trench
  decals or five tank rig variants.
- Adding a new candidate should usually mean adding or editing a local registry/profile entry, then
  refreshing the lab.
- Invalid candidate entries must fail soft: log or surface the local error, skip the broken
  candidate where possible, and keep the rest of the lab usable.

### Static Visual Samples

- The system must support renderer-only samples for visuals that do not need game simulation.
- Renderer-only samples may include candidate trench decals, ground marks, impact decals, labels, or
  other local presentation-only objects.
- Renderer-only samples must be driven by a profile-owned renderer read model, separate from
  `GameState` and authoritative snapshot data.
- Renderer-only samples must not be selectable, commandable, attackable, serialized, sent to the
  server, shown as minimap unit blips, or treated as fog/sight sources.
- Renderer-only samples must not mutate `GameState` or authoritative snapshot data.

### Real Unit Experiments

- The system should support visual overrides on real scenario-backed units without creating fake
  unit kinds.
- Real units with visual overrides must remain normal simulated units.
- Developers must be able to select and command those units through normal lab controls.
- Movement, facing, weapon facing, turret rotation, recoil, setup/deploy state, attack behavior,
  damage, death, and other runtime inputs must come from real game state.
- Multiple real units of the same kind should be able to show different visual candidates
  side-by-side.
- Override selection is local profile behavior. A profile may target real entities by simple,
  explicit local selectors such as entity id, unit kind plus ordinal, or unit kind plus nearest
  world position.
- Per-instance visual overrides must be local renderer behavior only; they must not change unit
  kind, stats, commands, combat, pathing, fog, or protocol shape.

### Entrenchment Experiments

- The first useful workflow should compare multiple entrenchment ground visuals at gameplay scale.
- Static entrenchment comparisons may use renderer-only preview trench records from the visual
  profile.
- Static preview trenches must not become real trench state and must not affect infantry behavior,
  slotting, combat, fog, or minimap state.
- Interaction/readability comparisons may use real trench state from a lab or dev scenario, with
  real infantry entering, occupying, leaving, and fighting around trenches.
- The workflow should make it possible to compare trench visuals next to eligible infantry,
  occupied-trench markers, selection rings, HP bars, and fog.

### Authored SVG Experiments

- Authored SVG candidates must be checked in and registered by local code.
- The URL must not load arbitrary SVG, JavaScript, image, or remote asset paths.
- Existing SVG safety constraints should continue to apply: no script, external references,
  `foreignObject`, external images, CSS URLs, filters, masks, clip paths, gradients, or other
  unsupported expensive paint features unless a future design explicitly expands the importer.
- SVG rig candidates should use the existing rig importer and runtime path where practical, so
  candidate behavior matches live unit animation behavior.

### Local Profile Behavior

- A developer should be able to launch an experiment with one local visual profile identifier.
- No profile id means normal lab behavior.
- A visual profile must be separate from authoritative checkpoint-backed lab setup JSON.
- Visual profile data must not be included in local lab setup exports.
- A profile may be tied to a specific scenario or map when that keeps the local comparison simple.
- Profiles may use brittle local selectors when acceptable for developer-only work, but they must
  fail visibly if they no longer match the current lab scene.

### Architectural Boundaries

- Checkpoint-backed lab setup payloads remain authoritative setup data for maps, players,
  resources, research, and real entities.
- Visual experiment metadata must not be added to saved lab setup JSON for the local-only workflow.
- Visual experiment metadata must not change the wire protocol unless a later requirement adds
  shareable or collaborative experiment sessions.
- Renderer-only samples belong to renderer/lab-visual code, not the simulation.
- Real-unit visual overrides belong to renderer routing and must be selected per rendered instance,
  not by changing authoritative entity kind.
- The lab remains a real game/lab room; visual experimentation should compose with existing lab
  controls instead of creating a parallel simulator.
- Any module that holds DOM/window listeners, Pixi objects, or other GPU resources must support
  normal match teardown.

## Non-Goals

- No hot reload.
- No collaborative visual review sessions.
- No persistent public visual experiment catalog.
- No production-facing visual experiment workflow.
- No arbitrary asset upload, file picker, remote asset loading, or URL-provided asset path.
- No general-purpose custom asset loading system.
- No changes to unit balance, collision, commands, combat, fog, or protocol for local visual
  experiments.
- No visual experiment metadata inside checkpoint-backed lab setup payloads or local lab setup
  exports.
- No requirement to make renderer-only samples behave like real game objects.

## Developer Stories

- As a developer, I can create five checked-in entrenchment decal candidates, register one visual
  profile, and open the lab to see them side-by-side on the real terrain.
- As a developer, I can zoom out to normal gameplay scale and verify whether each entrenchment
  candidate still reads clearly.
- As a developer, I can zoom in and inspect pixelation, edges, opacity, and layering against terrain
  and nearby units.
- As a developer, I can place candidate labels in the scene so a screenshot can be discussed without
  ambiguity.
- As a developer, I can compare static trench visuals without needing to create real trench
  simulation state.
- As a developer, I can run an interaction-focused entrenchment scenario with real infantry and real
  trenches to inspect occupied-trench readability.
- As a developer, I can create several checked-in tank turret or rig candidates and assign each
  candidate to a different real tank in the same lab scene through a local profile rule.
- As a developer, I can command those tanks to move and attack so I can compare turret rotation,
  recoil, facing, and movement readability under real gameplay animation inputs.
- As a developer, I can compare multiple unit rig variants of the same unit kind side-by-side without
  creating fake unit kinds or changing balance data.
- As a developer, I can iterate by editing local profile or SVG files, refreshing or restarting, and
  reopening the lab.
- As a developer, I can keep visual experiment work separate from authoritative lab scenarios and
  avoid accidentally including experiment-only metadata in a local scenario export.

## Acceptance Criteria

- A sanitized lab URL profile id can select a checked-in visual profile from a local registry.
- The URL cannot cause the client to load arbitrary paths, SVG, JavaScript, images, or remote
  assets.
- A local visual profile can render multiple labeled visual candidates in an existing lab scene.
- Static preview candidates do not appear in snapshots, selection, commands, minimap unit blips, or
  server state.
- Static preview candidates render from profile-owned renderer data without mutating `GameState`.
- Real scenario units can receive per-instance visual overrides while remaining fully controllable
  and simulated.
- Candidate unit variants update from real runtime animation inputs such as movement facing, weapon
  facing, setup state, and recoil.
- Opening a profile can start with the camera centered on the intended comparison area.
- Invalid profiles or candidates fail soft and do not crash the match loop.
- The implementation does not require protocol changes for the local-only workflow.
- The implementation does not add visual metadata to checkpoint-backed lab setup payloads.

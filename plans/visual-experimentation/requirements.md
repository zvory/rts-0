# Visual Experimentation Requirements

This document captures requirements and developer stories for local visual experimentation in the
lab. It is not an implementation plan.

## Product Goal

Developers need a local way to compare authored visual options inside the real RTS renderer. The
workflow should make it easy to place multiple candidates in the game world, zoom and pan around
them, compare them next to real units and terrain, and decide which candidate reads best at actual
gameplay scale.

The first target is entrenchment visuals, but the workflow should also support later experiments
such as tank turret variants, unit SVG rig changes, projectile marks, ground decals, and other
renderer-owned presentation assets.

## Scope

- This is strictly for local, solo developer experimentation.
- Visual experiment state does not need to be shareable between users.
- Visual experiment state does not need to support late joiners or multiplayer consistency.
- Visual experiment entries do not need to become durable public catalog entries.
- Restarting the server or refreshing the browser is acceptable after changing assets or profile
  data.
- Hot reloading is not required.

## Requirements

### Real Renderer Context

- Experiments must render through the same PixiJS renderer used by a normal match.
- Candidates must appear in the real game world, not only in a standalone static preview.
- Candidates must be visible with normal terrain, units, buildings, shadows, fog, selection rings,
  HP bars, and camera zoom.
- Developers must be able to pan and zoom normally while inspecting candidates.
- Experiments should support an initial camera target so opening the lab lands on the comparison
  area.
- Experiments should support lightweight labels for candidates, such as `A`, `B`, `C`, or short
  names, so feedback and screenshots are unambiguous.

### Static Visual Samples

- The system must support renderer-only samples for visuals that do not need game simulation.
- Renderer-only samples may include candidate trench decals, ground marks, impact decals, or other
  local presentation-only objects.
- Renderer-only samples must not be selectable, commandable, attackable, serialized, or sent to the
  server.
- Renderer-only samples must not mutate `GameState` or authoritative snapshot data.

### Real Unit Experiments

- The system must support visual overrides on real scenario-backed units.
- Real units with visual overrides must remain normal simulated units.
- Developers must be able to select and command those units through normal lab controls.
- Movement, facing, weapon facing, turret rotation, recoil, setup/deploy state, attack behavior,
  damage, death, and other runtime inputs must come from real game state.
- Multiple real units of the same kind must be able to show different visual candidates
  side-by-side.
- Per-instance visual overrides must be local renderer behavior only; they must not change unit kind,
  stats, commands, combat, pathing, fog, or protocol shape.

### Entrenchment Experiments

- Developers must be able to compare multiple entrenchment ground visuals at gameplay scale.
- Static entrenchment comparisons may use renderer-only preview trench records.
- Interaction/readability comparisons should be able to use real trench state from a lab or dev
  scenario, with real infantry entering, occupying, leaving, and fighting around trenches.
- The workflow should make it possible to compare trench visuals next to eligible infantry,
  occupied-trench markers, selection rings, HP bars, and fog.

### Authored Asset Experiments

- Authored SVG candidates must be checked in or otherwise locally allowlisted.
- The URL must not load arbitrary SVG, JavaScript, image, or remote asset paths.
- Existing SVG safety constraints should continue to apply: no script, external references,
  `foreignObject`, external images, CSS URLs, filters, masks, clip paths, gradients, or other
  unsupported expensive paint features unless a future design explicitly expands the importer.
- SVG rig candidates should use the existing rig importer and runtime path where practical, so
  candidate behavior matches live unit animation behavior.

### Local Profile Behavior

- A developer should be able to launch an experiment with a local visual profile identifier.
- A visual profile should be immutable for the lifetime of a lab load.
- A visual profile may define candidate assets, labels, preview sample positions, real-entity
  override rules, and an initial camera target.
- The profile must be separate from authoritative lab scenario JSON.
- Visual profile data must not be submitted through the lab scenario PR workflow.

### Architectural Boundaries

- `LabScenarioV1` remains authoritative setup data for maps, players, resources, research, and real
  entities.
- Visual experiment metadata must not be added to saved scenario JSON for the local-only workflow.
- Visual experiment metadata must not change the wire protocol unless a later requirement adds
  shareable or collaborative experiment sessions.
- Renderer-only samples belong to renderer/lab-visual code, not the simulation.
- Real-unit visual overrides belong to renderer routing and must be selected per rendered instance,
  not by changing authoritative entity kind.
- The lab remains a real game/lab room; visual experimentation should compose with existing lab
  controls instead of creating a parallel simulator.

## Non-Goals

- No hot reload.
- No collaborative visual review sessions.
- No persistent public visual experiment catalog.
- No production-facing visual experiment workflow.
- No arbitrary asset upload or remote asset loading.
- No changes to unit balance, collision, commands, combat, fog, or protocol for local visual
  experiments.
- No requirement to make renderer-only samples behave like real game objects.

## Developer Stories

- As a developer, I can create five entrenchment decal candidates and open the lab to see them
  side-by-side on the real terrain.
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
- As a developer, I can create several tank turret SVG candidates and assign each candidate to a
  different real tank in the same lab scene.
- As a developer, I can command those tanks to move and attack so I can compare turret rotation,
  recoil, facing, and movement readability under real gameplay animation inputs.
- As a developer, I can compare multiple unit rig variants of the same unit kind side-by-side without
  creating fake unit kinds or changing balance data.
- As a developer, I can iterate by editing local profile or SVG files, restarting or refreshing, and
  reopening the lab.
- As a developer, I can keep visual experiment work separate from authoritative lab scenarios and
  avoid accidentally submitting experiment-only metadata as a scenario PR.

## Acceptance Criteria

- A local visual profile can render multiple labeled visual candidates in an existing lab scene.
- Static preview candidates do not appear in snapshots, selection, commands, minimap unit blips, or
  server state.
- Real scenario units can receive per-instance visual overrides while remaining fully controllable
  and simulated.
- Candidate unit variants update from real runtime animation inputs such as movement facing, weapon
  facing, setup state, and recoil.
- Opening a profile starts with the camera centered on the intended comparison area.
- The implementation does not require protocol changes for the local-only workflow.
- The implementation does not add visual metadata to `LabScenarioV1`.

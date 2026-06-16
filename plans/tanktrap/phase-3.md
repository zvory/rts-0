# Phase 3 - Authoritative Construction and Gameplay Rules

Status: Done.

## Goal

Make Tank Trap authoritative on the server: workers can receive valid Tank Trap build orders after
Training Centre, resources are checked and charged through the existing construction model, Tank
Traps can be destroyed, and they do not keep a player alive.

## Scope

- Add Tank Trap to server-side worker build eligibility for the default faction once a completed
  Training Centre exists.
- Route issue-time and arrival-time build validation through one authoritative
  `BuildPlacementPolicy` or equivalent helper selected by building kind. Tank Trap policy may allow
  infantry overlap but must reject terrain, resources, ordinary buildings, vehicle-body units,
  invalid/out-of-bounds footprints, and stale or destroyed sites.
- Preserve current reserve-on-arrival construction semantics:
  - command-time placement and affordability produce feedback only
  - final placement and affordability are re-checked when the worker arrives
  - 15 steel is charged when the site starts/resumes under current construction rules
- Ensure Tank Trap construction uses the vehicle-only blocker policy from Phase 2 immediately,
  including incomplete construction sites.
- Exclude Tank Traps from elimination-survival building counts while keeping them attackable and
  removable through normal death cleanup.
- Introduce `world_query::owned_survival_buildings` or an equivalent survival helper and make
  `Game::alive_players` use it, leaving broad building iterators available for cleanup, scoring,
  memory, and generic building behavior.
- Ensure Tank Traps do not act as production anchors, rally targets, supply providers, research
  structures, or trainable-unit producers.
- Verify zero sight does not grant owner fog reveal. Confirm enemy discovery uses the normal
  visible/remembered building projection path.
- Add focused server tests for:
  - Training Centre prerequisite is enforced
  - cost and build time match the spec
  - Tank Trap construction blocks vehicles before completion
  - Tank Trap site is allowed under infantry, rejected under vehicle-body units/resources/buildings,
    and rechecked on worker arrival
  - Tank Trap does not prevent owner elimination
  - Tank Trap can be damaged and destroyed
  - Tank Trap produces no sight/fog reveal

## Expected Deliverables

- Server accepts valid Tank Trap build commands after Training Centre.
- Server rejects or notices invalid Tank Trap build attempts consistently with other buildings.
- Tank Trap gameplay rules are complete server-side while the client UI may still hide construction.
- No special cancel/repair/salvage behavior is added.

## Out of Scope

- Worker build-card exposure.
- Client line placement.
- Polished art or sound.
- AI build logic.

## Verification

- Run focused Rust tests for construction, economy, elimination, combat, and fog behavior touched by
  this phase.
- Run `cargo fmt` for touched Rust crates.
- Run architecture checks if new service edges or helper exports are added.

## Manual Testing Focus

If a debug scenario or command injection path exists, issue a Tank Trap build command after creating
a Training Centre. Confirm steel is charged when construction starts, vehicles avoid the site, and
destroying all non-trap buildings eliminates the player.

## Handoff Expectations

The handoff must summarize server-side construction status, prerequisite and cost behavior,
elimination handling, fog behavior, tests run, and any client work Phase 4 must mirror.

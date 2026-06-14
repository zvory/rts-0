# Phase 1 - Map Atlas Foundation

Status: Planned.

## Objective

Generate a deterministic static atlas for every authored map. This phase should not change live AI
decisions; it should only create and validate the map facts that later routing code can trust.

## Required Atlas Fields

- Movement-class passability: required because different units can have different terrain and body
  constraints, and route choice must know whether a Scout Car route is actually usable by Scout
  Cars.
- Connected components: required so route queries can reject impossible paths cheaply and avoid
  pretending two locations are connected when terrain says they are not.
- Clearance field: required because route quality depends on width, not only pass/fail
  reachability. This is the machine-checkable answer to claims about narrow passages.
- Regions: required to compress raw tile terrain into open areas that strategy code can reason
  over.
- Portals: required to connect regions and record passage facts such as center, width, movement
  classes, and adjacent regions.
- Semantic anchors: required to connect strategic concepts to topology. Starts, mains, naturals,
  resource clusters, and selected resource-line approach anchors should map to components and
  regions.

Do not add authored lanes, route summaries, dynamic influence maps, or atlas visualization in this
phase. Lanes and summaries should be derived from atlas queries later; influence maps need live
observations; the human-readable static atlas view is handled separately in Phase 1.5 after the
atlas data exists.

## Scope

- Add a map-atlas data model near the map/simulation boundary where authored map terrain is already
  loaded.
- Generate the atlas deterministically from authored terrain, selected map sites, and resource
  placement data.
- Keep generated ids stable for review and tests. Prefer deterministic ordering by coordinate and
  semantic id.
- Compute atlas facts for each relevant movement class. Start with the current ground movement
  classes needed by infantry and vehicles; keep the representation extensible for future units.
- Attach authored main/natural sites and generated resource clusters to atlas components and
  regions.
- Validate atlas generation for every bundled authored map.
- Add tests that prove generated atlas facts are internally consistent: every portal connects two
  regions in the same component, anchors attach to passable regions, and clearance values are
  bounded and deterministic.
- Document atlas ownership and limits in the relevant design doc.

## Expected Touch Points

- `server/crates/sim/src/game/map.rs`
- `server/crates/sim/src/game/map/authored.rs`
- New atlas module under `server/crates/sim/src/game/map/` or another map-owned location
- Map tests under `server/crates/sim/src/game/map/`
- `docs/design/server-sim.md` if the map API or ownership boundary changes
- `docs/design/ai.md` only to note that later AI routing will consume public atlas facts

## Verification

Run focused map and sim tests:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim map
```

If the atlas module has a narrower test filter, run that too. Do not run broad live Node suites for
this docs/data-foundation phase unless a server-facing contract changes.

## Manual Testing Focus

No gameplay manual test is required because live behavior should not change. If a map-loading smoke
check is desired, start a local match on each bundled authored map and confirm match creation still
succeeds.

## Handoff Expectations

The handoff must list the atlas fields implemented, where atlas generation lives, and which bundled
maps are covered by tests. It must explicitly call out any atlas facts that were deferred, such as
derived lane labels, route summaries, dynamic influence, or visualization-only diagnostics needed
by Phase 1.5.

## Player-Facing Outcome

No intended player-facing change. This phase creates the map knowledge needed for later AI routing
without changing unit commands.

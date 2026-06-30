# Artillery UX Requirements

Status: Draft product requirements. This document describes the desired player-facing behavior for
artillery point fire, blanket fire, setup, queueing, and range targeting. It is not an
implementation plan and does not authorize code, protocol, balance, art, or test changes by itself.

## Purpose

Artillery should support two deliberate fire modes:

- `Point Fire`: repeatedly shell a chosen target point.
- `Blanket Fire`: repeatedly shell random points around a chosen target point.

Both modes should reduce setup micro without making artillery automatically walk or stage itself
into firing position. The player chooses a target direction or point; artillery either uses its
current setup cone or redeploys in place to face the chosen target.

Implementation should model each mode as its own terminal fire order, not as a synthetic queued
`Set Up` order followed by a separate fire order. The fire order owns any setup/redeploy transition
needed before the first shot.

## Command Card

- Add `Blanket Fire` for artillery in the bottom-right command-card slot. In the default grid
  hotkey layout, this is `C`.
- `Blanket Fire` always requires a target click, even when every selected artillery piece is already
  deployed and idle.
- `Point Fire` remains a separate targeted command.
- `Stop` cancels active point fire, active blanket fire, setup or redeploy orders created by these
  commands, and queued orders.

## Setup And Redeploy Behavior

- Point fire and blanket fire both auto-set-up artillery if the gun is packed or otherwise not
  deployed.
- This is an intentional change to Point Fire: a packed artillery piece can now accept an immediate
  Point Fire order, set up in place toward the effective fire point, then begin point fire once
  deployed.
- If an artillery piece is already deployed and the clicked or locked target point is inside its
  current cone, it fires from the current setup.
- If an artillery piece is deployed and the clicked or locked target point is outside its current
  cone, it tears down, rotates, sets up toward that point, then starts the requested fire mode.
- This behavior applies to immediate commands and queued commands.
- Point Fire and Blanket Fire both redeploy in place when the effective fire point is outside the
  current cone instead of requiring the player to issue a separate setup command first.

## Queueing

- Point fire and blanket fire are both queueable.
- Point fire and blanket fire are terminal per artillery piece. Once either fire order is accepted
  for a gun, later queued unit orders for that same gun are not appended behind it. Other selected
  units that did not accept the terminal fire order may still accept compatible queued orders.
- A queued point fire or blanket fire command after a movement order should use the selected
  artillery piece's future queued position for preview and for computing the stored effective fire
  point when that queued position is available.
- Example flow: right-click move, hold `Shift`, press point fire or blanket fire, preview from the
  future position where possible, click target, then the gun moves, sets up or redeploys, and begins
  firing.
- Queued fire commands should not cause automatic walking or staging to get a target into range.
- Execution remains server-authoritative. If the stored effective fire point is stale or invalid
  when the queued fire order promotes, the gun skips that fire order safely instead of walking,
  relocking the original click, or firing outside its valid band.

## Range Targeting

- Automatic walking to bring a point into range is out of scope.
- Clicks outside valid artillery range should not be rejected solely because of range.
- Instead, each artillery piece locks the clicked point to its own valid range along the ray from
  the gun to the cursor:
  - If the clicked point is inside minimum range, lock to minimum range.
  - If the clicked point is outside maximum range, lock to maximum range.
  - If the clicked point is already in valid range, use the clicked point.
- The per-gun locked point is the command/order target stored for execution. Do not store the raw
  clicked point and reinterpret it later.
- For queued commands, compute this locked point from the authoritative future queued position when
  the server can infer one; otherwise use the gun's current position.
- For a zero-length ray, use the gun's current or planned setup facing as the ray direction. If no
  setup facing exists yet, use the gun's current body facing.
- Clamp the final stored point to the playable map along the same ray. If no in-map point exists on
  that ray within the valid range band for that gun, that gun ignores the command rather than
  walking or firing at an invalid point.
- Cone checks and previews use this per-gun locked point.
- With multiple artillery pieces selected, different guns may lock to different points because each
  gun has its own origin.

## Blanket Fire Behavior

- Blanket fire repeats until stopped or replaced by another order.
- Blanket fire uses the same clicked or locked effective fire point rules as Point Fire. The
  effective center point must be inside that artillery piece's valid firing cone and range band.
- Each blanket fire shot picks a deterministic pseudo-random impact point uniformly from a 15-tile
  radius circle centered on the stored effective fire point.
- The blanket radius is centered on the chosen point even when that point is near the edge of the
  firing cone or range band. Individual sampled impact points are not re-clamped to the cone or
  range band after they are chosen.
- Blanket fire randomness must remain deterministic for command-log replay. Seed each shot from
  authoritative simulation inputs such as match seed or tick, artillery id, owner, and shot number;
  do not use nondeterministic runtime RNG.
- Blanket fire does not select enemy units or visible enemy positions. It blankets terrain.
- Blanket fire should use the same shell projectile, ammunition cost, reload cadence, shell delay,
  impact radius, damage behavior, no-ammo behavior, and fog or reveal handling as artillery point
  fire unless a later requirement explicitly changes one of those surfaces. The only firing
  difference is shot placement: Point Fire repeats against its stored effective fire point, while
  Blanket Fire samples a deterministic uniform point within the 15-tile blanket radius around that
  same stored effective fire point for each shot.

## Preview UX

- Point fire and blanket fire targeting should preview, per selected artillery piece, whether the
  command will use the current cone or cause redeploy.
- If a gun can fire from its current cone, show the current cone.
- If a gun needs to rotate or redeploy, show the new setup cone following the locked mouse target.
- Blanket fire previews should show the 15-tile blanket radius around each gun's stored effective
  center point.
- For queued commands, previews should account for queued movement destinations where the current
  setup-preview system can already infer a future position.
- The preview should make out-of-range locking legible enough that players can see the effective
  firing direction and valid cone before committing the click.

## Balance Requirement

- Increase artillery minimum range by 10 tiles.
- The current intended change is from 15 tiles to 25 tiles.
- Set Blanket Fire's sampled impact radius to 15 tiles around the stored effective center point.
- Mirror these values anywhere artillery range or blanket radius is surfaced to players.
- Preserve the current accuracy feel across the remaining valid range band when making this change:
  the minimum-range error should still apply at the new 25-tile floor, the maximum-range error
  should still apply at the 55-tile ceiling, and interpolation should be recalculated across the
  new 25-to-55 tile band.

## Ballistic Tables Decision

- Keep Ballistic Tables repeated-shot accuracy tightening Point-Fire-only. Blanket Fire remains
  deterministic uniform area suppression over the fixed 15-tile blanket radius instead of
  tightening into precision fire over time.

## Non-Goals

- Do not make artillery walk, path, or stage itself to get a clicked point into range.
- Do not change non-artillery unit behavior.
- Do not make blanket fire target enemies, visible entities, or fog-derived enemy positions.
- Do not replace point fire with blanket fire.
- Do not treat this requirements document as implementation approval for code, protocol, balance,
  rendering, or test changes.

# Artillery UX Requirements

Status: Draft product requirements. This document describes the desired player-facing behavior for
artillery point fire, blanket fire, setup, queueing, and range targeting. It is not an
implementation plan and does not authorize code, protocol, balance, art, or test changes by itself.

## Purpose

Artillery should support two deliberate fire modes:

- `Point Fire`: repeatedly shell a chosen target point.
- `Blanket Fire`: repeatedly shell random points inside a chosen firing cone.

Both modes should reduce setup micro without making artillery automatically walk or stage itself
into firing position. The player chooses a target direction or point; artillery either uses its
current setup cone or redeploys in place to face the chosen target.

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
- If an artillery piece is already deployed and the clicked or locked target point is inside its
  current cone, it fires from the current setup.
- If an artillery piece is deployed and the clicked or locked target point is outside its current
  cone, it tears down, rotates, sets up toward that point, then starts the requested fire mode.
- This behavior applies to immediate commands and queued commands.
- Point fire keeps its current convenience behavior of redeploying in place when the target is
  outside the current cone.
- Blanket fire should match that convenience behavior instead of requiring the player to issue a
  separate setup command first.

## Queueing

- Point fire and blanket fire are both queueable.
- A queued point fire or blanket fire command after a movement order should use the selected
  artillery piece's future queued position for preview and execution when that information is
  available.
- Example flow: right-click move, hold `Shift`, press point fire or blanket fire, preview from the
  future position where possible, click target, then the gun moves, sets up or redeploys, and begins
  firing.
- Queued fire commands should not cause automatic walking or staging to get a target into range.

## Range Targeting

- Automatic walking to bring a point into range is out of scope.
- Clicks outside valid artillery range should not be rejected solely because of range.
- Instead, each artillery piece locks the clicked point to its own valid range along the ray from
  the gun to the cursor:
  - If the clicked point is inside minimum range, lock to minimum range.
  - If the clicked point is outside maximum range, lock to maximum range.
  - If the clicked point is already in valid range, use the clicked point.
- Cone checks and previews use this per-gun locked point.
- With multiple artillery pieces selected, different guns may lock to different points because each
  gun has its own origin.

## Blanket Fire Behavior

- Blanket fire repeats until stopped or replaced by another order.
- Each blanket fire shot picks a random target point inside that artillery piece's valid firing
  cone and range band.
- Blanket fire does not select enemy units or visible enemy positions. It blankets terrain.
- Blanket fire should use the same shell projectile, ammunition cost, reload cadence, shell delay,
  impact radius, damage behavior, and fog or reveal handling as artillery point fire unless a later
  requirement explicitly changes one of those surfaces.

## Preview UX

- Point fire and blanket fire targeting should preview, per selected artillery piece, whether the
  command will use the current cone or cause redeploy.
- If a gun can fire from its current cone, show the current cone.
- If a gun needs to rotate or redeploy, show the new setup cone following the locked mouse target.
- For queued commands, previews should account for queued movement destinations where the current
  setup-preview system can already infer a future position.
- The preview should make out-of-range locking legible enough that players can see the effective
  firing direction and valid cone before committing the click.

## Balance Requirement

- Increase artillery minimum range by 10 tiles.
- The current intended change is from 15 tiles to 25 tiles.
- Mirror the value anywhere artillery range is surfaced to players.

## Open Product Decision

- Decide whether blanket fire benefits from Ballistic Tables repeated-shot accuracy tightening.
- Current recommendation: keep the accuracy ramp point-fire-only so blanket fire remains area
  suppression instead of eventually becoming precision fire.

## Non-Goals

- Do not make artillery walk, path, or stage itself to get a clicked point into range.
- Do not change non-artillery unit behavior.
- Do not make blanket fire target enemies, visible entities, or fog-derived enemy positions.
- Do not replace point fire with blanket fire.
- Do not treat this requirements document as implementation approval for code, protocol, balance,
  rendering, or test changes.

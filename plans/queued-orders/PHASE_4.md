# Phase 4 - Mixed Attack and Movement Queues

Goal: allow mixed control sequences such as move, attack target, attack-move, move.

## Scope

- Add queued explicit `Attack` intents for owned combat units.
- Promotion validation:
  - Target exists.
  - Target is enemy targetable.
  - Target visibility rules remain compatible with current command validation.
  - Unit still exists, is owned by the player, and can receive combat orders.
- Completion:
  - Promote after target death.
  - Promote after target becomes invalid.
  - Add an explicit unreachable/failure rule before allowing attack orders to stall forever.
- Mixed queues:
  - Attack can be followed by move or attack-move.
  - Move and attack-move can be followed by attack.
  - Normal attack command clears queued orders unless sent with `queued: true`.

## Design Notes

This phase should not weaken fog. If an attack target is no longer valid or targetable when the
queued attack promotes, skip it. Do not retain hidden target positions in queued marker data.

The hardest design point is "unreachable." A simple first rule can be conservative: if pathing has
failed and the unit cannot fire for a bounded number of completion checks, skip the queued attack.
That rule needs tests because it can otherwise create surprising behavior near walls.

## Tests

- Unit executes move, attack target, then move.
- Queued attack is skipped when the target dies before promotion.
- Queued attack is skipped when the target is no longer targetable.
- Attack followed by attack-move preserves the attack-move destination behavior.
- Attack queue markers do not reveal hidden target positions to other players.

## Done

- Mixed attack/movement queues are reliable enough for army micro.
- Explicit attack cannot stall the queue forever on stale or invalid targets.


# Phase 3: Smoke Command Execution and Queueing

## Objective

Wire Smoke's gameplay command end to end: caster resolution, launch costs, cooldowns, movement into
range, Shift queueing, notices, and immediate cloud creation on launch.

## Caster Resolution

For each `UseAbility(smoke, units, x, y, queued)`:

- Filter to owned, alive, completed, ability-capable units.
- Filter out units on cooldown.
- Require completed Steelworks.
- Validate finite target coordinates and clamp/reject out-of-map points consistently with existing
  point commands.
- If at least one eligible selected carrier is in range, choose the furthest in-range carrier from
  the target point.
- If no eligible selected carrier is in range, choose the closest eligible selected carrier and
  assign a move-to-launch order.
- If there are no eligible carriers, ignore or emit a minimal notice consistent with existing
  command behavior.

## Launch Semantics

- Re-check resources at launch time.
- If resources are insufficient, emit `Not enough oil` when oil is short, otherwise `Not enough
  steel` when steel is short, matching existing resource-shortage priority.
- Do not reserve resources during movement.
- On successful launch:
  - subtract 25 steel and 25 oil;
  - start 20-second cooldown on the caster;
  - spawn a smoke cloud immediately at the target point;
  - clear the caster's smoke order and leave it idle unless later queued orders exist.

## Movement Into Range

- Represent pending launch as an active ability order with target point, ability kind, and launch
  staging point.
- Compute the staging point from target/caster geometry at or just inside 9 tiles.
- Use existing movement/pathing through `MoveCoordinator` where possible.
- On arrival or tolerant arrival, attempt launch with current resources, cooldown, tech, and target
  validity.
- If the path fails, skip the ability order and promote the next queued order if any.

## Shift Queueing

- `queued: true` appends an ability intent to each eligible carrier's queue, bounded by existing
  queue caps.
- Promotion validates current tech/resources/cooldown/range and either launches, moves into range,
  or skips invalid intent.
- Normal non-queued Smoke replaces the active order and clears existing queued orders for the chosen
  caster only after the server resolves the caster. Avoid disrupting unchosen selected units.
- `Stop` clears active ability orders and queued ability intents.

## Done

- In-range Smoke launches from the furthest in-range selected scout car.
- Out-of-range Smoke moves the closest selected scout car into range, launches, then idles.
- Launch-time affordability and cooldown behavior matches the confirmed rules.
- Shift-queued Smoke works deterministically and remains bounded.
- Existing move/attack/build/train/charge behavior is unchanged.

## Verification

- `cd server && cargo test`
- Add replay/log tests so queued and out-of-range Smoke replay deterministically.
- Run Node integration suites if protocol or room command handling changes.

# Phase 2 - AT Gun and Tank Unlock Upgrades

## Objective

Add explicit investment gates for the key stage-two units: AT Guns and Tanks. Mortars and Scout Cars
should be available at path entry, while AT Guns and Tanks require commitment.

## Work

- Add a Steelworks research upgrade that unlocks AT Gun training.
- Add a Vehicle Works research upgrade that unlocks Tank training.
- Keep Scout Cars trainable immediately from Vehicle Works.
- Make the client command card show locked AT Gun and Tank buttons with clear requirement text.
- Mirror upgrade definitions across server and client where needed.
- Update protocol and design docs if new upgrade identifiers are added to the wire contract.
- Add server-side validation so clients cannot train AT Guns or Tanks before their upgrades finish.

## Tuning Intent

- The Tank upgrade is the Mobile Warfare stage-two surge investment.
- The AT Gun upgrade is the Superior Firepower defensive commitment.
- Both upgrades should be meaningful choices, not automatic zero-opportunity-cost buttons.

## Verification

- Unit tests cover rejected AT Gun training before the Steelworks upgrade.
- Unit tests cover accepted AT Gun training after the Steelworks upgrade.
- Unit tests cover rejected Tank training before the Vehicle Works upgrade.
- Unit tests cover accepted Tank training after the Vehicle Works upgrade.
- Client HUD shows correct locked/unlocked train states.

## Player-Facing Outcome

Players can field Scout Cars and Mortars before the heavy counter units arrive. Tanks and AT Guns
become deliberate tech commitments instead of automatic building unlocks.


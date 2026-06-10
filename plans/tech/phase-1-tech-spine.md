# Phase 1 - Tech Spine and Vehicle Works Framing

## Objective

Clarify the two-path structure with minimal mechanical risk. Mobile Warfare is already mostly
implemented through Scout Cars and Tanks, so this phase should preserve working behavior while
making the path structure explicit.

## Work

- Reframe the current Factory as **Vehicle Works** in player-facing UI and docs.
- Prefer keeping the internal kind as `factory` for now to avoid unnecessary protocol migration.
- Confirm Training Centre remains the shared prerequisite before either advanced path.
- Confirm Vehicle Works trains Scout Cars immediately.
- Confirm Steelworks is the Superior Firepower path building.
- Update balance/design docs that describe unit roles and production buildings.

## Verification

- Build a normal base progression and confirm the renamed Vehicle Works can still be built.
- Confirm Scout Cars remain trainable from Vehicle Works without an extra unlock.
- Confirm no protocol kind rename is required in this phase.

## Player-Facing Outcome

Players see a clearer Mobile Warfare production building while gameplay remains mostly unchanged.
This creates room for later upgrades without destabilizing existing vehicle behavior.


# Phase 3 - Gun Works AT Gun Production

## Objective

Move AT Gun production from Barracks to Gun Works so the Superior Firepower path has a coherent
production identity.

## Work

- Remove AT Gun from Barracks trainables.
- Add AT Gun to Gun Works trainables.
- Keep AT Gun training locked behind the Gun Works AT Gun upgrade from Phase 2.
- Update server rules definitions, client config, HUD labels/tooltips, and balance docs together.
- Audit AI and self-play scripts that assume AT Guns are trained from Barracks.
- Audit tests that queue AT Guns from Barracks and update them to use Gun Works.

## Verification

- Barracks can no longer train AT Guns.
- Gun Works can train AT Guns only after the unlock upgrade.
- Existing production queue, rally point, cancel, and affordability behavior works for Gun Works.
- AI and self-play tests either avoid AT Guns or build the correct tech path first.

## Player-Facing Outcome

Superior Firepower players use Gun Works as their crew-served weapons hub. Barracks remains an
early infantry building instead of carrying late defensive tech.


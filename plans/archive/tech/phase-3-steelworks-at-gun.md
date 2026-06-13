# Phase 3 - Gun Works Anti-Tank Gun Production

## Objective

Move Anti-Tank Gun production from Barracks to Gun Works so the Superior Firepower path has a coherent
production identity.

## Work

- Remove Anti-Tank Gun from Barracks trainables.
- Add Anti-Tank Gun to Gun Works trainables.
- Keep Anti-Tank Gun training locked behind the Gun Works Anti-Tank Gun upgrade from Phase 2.
- Update server rules definitions, client config, HUD labels/tooltips, and balance docs together.
- Audit AI and self-play scripts that assume Anti-Tank Guns are trained from Barracks.
- Audit tests that queue Anti-Tank Guns from Barracks and update them to use Gun Works.

## Verification

- Barracks can no longer train Anti-Tank Guns.
- Gun Works can train Anti-Tank Guns only after the unlock upgrade.
- Existing production queue, rally point, cancel, and affordability behavior works for Gun Works.
- AI and self-play tests either avoid Anti-Tank Guns or build the correct tech path first.

## Player-Facing Outcome

Superior Firepower players use Gun Works as their crew-served weapons hub. Barracks remains an
early infantry building instead of carrying late defensive tech.


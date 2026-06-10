# Phase 3 - Steelworks AT Gun Production

## Objective

Move AT Gun production from Barracks to Steelworks so the Superior Firepower path has a coherent
production identity.

## Work

- Remove AT Gun from Barracks trainables.
- Add AT Gun to Steelworks trainables.
- Keep AT Gun training locked behind the Steelworks AT Gun upgrade from Phase 2.
- Update server rules definitions, client config, HUD labels/tooltips, and balance docs together.
- Audit AI and self-play scripts that assume AT Guns are trained from Barracks.
- Audit tests that queue AT Guns from Barracks and update them to use Steelworks.

## Verification

- Barracks can no longer train AT Guns.
- Steelworks can train AT Guns only after the unlock upgrade.
- Existing production queue, rally point, cancel, and affordability behavior works for Steelworks.
- AI and self-play tests either avoid AT Guns or build the correct tech path first.

## Player-Facing Outcome

Superior Firepower players use Steelworks as their crew-served weapons hub. Barracks remains an
early infantry building instead of carrying late defensive tech.


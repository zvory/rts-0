# Phase 6 - Stats, Docs, And Data Surface

## Phase Status

Status: pending.

## Objective

Align player-facing data surfaces and source-of-truth docs with the implemented weapon-profile and
Tank coax behavior. This phase should make the feature understandable in generated references
without changing Tank commands or primary stat presentation beyond the approved requirement.

## Scope

- Update `docs/design/server-sim.md` to describe weapon profiles, Tank cannon versus coax firing,
  independent cooldowns, arc gating, and panic-free stale target behavior.
- Update `docs/design/balance.md` to describe Tank coax range, damage, cooldown, weapon class,
  overpenetration, target priority, and unchanged Tank cost/supply/sight/trainability.
- Update `docs/design/protocol.md` if the final attack weapon identity field or compact schema
  differs from the Phase 3 draft.
- Update `docs/design/client-ui.md` to describe weapon-specific attack feedback and Tank coax rig
  treatment if Phase 5 changed renderer contracts.
- Update generated stats/wiki surfaces if implementation exposes secondary weapons there. The Tank
  primary displayed range should remain the main-cannon range unless a later requirement adds a
  separate coax range display.
- Ensure client config mirrors only the data the UI/render/fog surface consumes. Do not add
  unnecessary mirrored damage policy if the client only needs feedback ids.
- Refresh [requirements.md](requirements.md) only for decisions made during implementation, such as
  the final infantry-priority group, if they are not already captured.
- Collect factual patch-note bullets for the final implementation.
- Do not add a command-card button, toggle, upgrade, research, or new range display.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/balance.md`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `plans/coax/requirements.md`
- `server/crates/rules/src/bin/dump-faction-catalog.rs` or wiki data helpers if secondary weapons
  become generated
- `server/src/wiki*` or related wiki/stat generation files if applicable
- `client/src/config*.js` only if a consumed mirror is required
- `node scripts/check-wiki.mjs` related fixtures if generated stats change

## Edge Cases To Cover

- Docs do not imply that the Tank command card, cost, supply, sight, trainability, or primary range
  display changed.
- Docs distinguish Tank cannon AP behavior from coax small-arms behavior.
- Docs state that coax overpenetrates with small-arms damage.
- Protocol docs match the actual Rust and JS attack-event weapon field names and compact slot
  shape.
- Wiki/generated stats either mention the secondary weapon accurately or intentionally omit it
  until secondary weapons are supported.

## Verification

- `node scripts/check-docs-health.mjs`.
- `node scripts/check-wiki.mjs` if generated stats/wiki surfaces are touched.
- `node scripts/check-faction-catalog-parity.mjs` if visible rules/catalog mirrors are touched.
- `node tests/protocol_parity.mjs` if protocol docs or constants are touched.
- `git diff --check`.

## Manual Test Focus

Manual gameplay is optional for this docs/data phase. If a wiki or stats page is changed, open it
locally and confirm the Tank entry is clear and does not overstate UI behavior.

## Handoff Expectations

List the final player-facing patch notes and the docs/data surfaces updated. Call out any deliberate
omissions, especially if secondary weapons are not yet represented in generated wiki tables.

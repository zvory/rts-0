# Phase 2 - Roster, Stats, and Balance Tables

Status: pending.

## Goal

Bring active roster, stat, upgrade, ability, and balance documentation in line with current rules
and client mirrors.

## Scope

- Compare active balance docs and generated wiki stats with the authoritative Rust rules and
  faction catalogs.
- Audit units, buildings, resources, upgrades, abilities, costs, supply, sight, footprints, speed,
  range, cooldown, armor, weapon class, training locations, and build/research timing.
- Update stale docs or generated-reference labels only when confirmed by current source.
- Do not tune gameplay values, rewrite faction catalogs, or broaden into behavioral changes.

## Suggested Evidence

- `docs/context/balance.md` first, then `docs/design/balance.md`.
- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/faction.rs`
- `server/crates/rules/src/balance.rs`
- `client/src/config.js`
- `tests/client_contracts.mjs`
- `tests/faction_integration.mjs`

Useful searches:

```bash
rg -n "HP|Damage|Range|Cooldown|Speed|Sight|Supply|cost|trained at|requires" docs/design/balance.md docs/context/balance.md
rg -n "const .*:|trained_at|build_requires|train_requires|cost_|supply|range_tiles|cooldown" server/crates/rules/src
```

## Verification

Run focused checks that match the final diff. Likely commands:

```bash
node scripts/check-faction-catalog-parity.mjs
node scripts/check-wiki.mjs
node scripts/check-docs-health.mjs
git diff --check
```

If a Rust doc-generation or rules test is touched, add the narrow Rust test the executor identifies.

## Manual Testing Focus

Inspect `/wiki/stats` and the relevant `docs/design/balance.md` tables for the units/buildings
changed by the phase. Confirm they agree on player-facing names, costs, requirements, and stats.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must name the source files compared,
the docs/stat tables updated, verification run, and any stats left unresolved.

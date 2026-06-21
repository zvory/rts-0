# Phase 4 - Combat, Abilities, Fog, and Orders

Status: done.

## Goal

Fix stale active docs for combat, special abilities, order queues, targeting, fog authority, and
privacy-sensitive projection behavior.

## Scope

- Audit combat stats, targeting rules, min/max ranges, projectile/tracer behavior, mortar and
  artillery behavior, hero/ability behavior, autocast, charges, cooldowns, queued orders, and attack
  movement semantics.
- Audit fog and visibility docs for entity snapshots, events, target ids, positional information,
  ownership exceptions, and observer behavior.
- Fix active docs that contradict current sim/rules/protocol/hardening behavior.
- Do not change combat, ability, fog, or order behavior.

## Suggested Evidence

- `docs/context/server-sim.md`
- `docs/context/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/hardening.md`
- `docs/design/balance.md`
- `server/crates/sim/src/game/services/combat/**`
- `server/crates/sim/src/game/mortar.rs`
- `server/crates/sim/src/game/artillery.rs`
- `server/crates/sim/src/game/ability*.rs`
- `server/crates/sim/src/game/hero_abilities.rs`
- `server/crates/sim/src/game/fog.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/rules/src/faction.rs`
- `server/crates/rules/src/balance.rs`
- `client/src/renderer/feedback.js`

Useful searches:

```bash
rg -n "range|cooldown|ability|autocast|charge|attack|target|fog|visibility|visible|owner|observer|projectile|mortar|artillery|order" docs/design docs/context server/crates client/src -S
```

## Verification

Run focused checks that match the final diff. Likely commands:

```bash
node scripts/check-wiki.mjs
node scripts/check-docs-health.mjs
git diff --check
```

If protocol or compact snapshot docs change, also run the relevant protocol/client contract check
identified by the executor.

## Manual Testing Focus

Later manual smoke should focus on ability command cards, targeting previews, fog-gated events, and
whether docs describe what a normal player can actually see.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must list combat/ability/fog claims
changed, source evidence for each, verification run, and any areas intentionally left as uncertain.

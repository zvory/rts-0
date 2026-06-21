# Phase 3 - Economy, Production, and Tech Tree

Status: pending.

## Goal

Fix stale documentation about resources, construction, production chains, command-card locks, and
research/tech progression.

## Scope

- Audit economy rules, resource nodes, gathering, supply, construction, build prerequisites,
  training prerequisites, research locations, upgrade dependencies, and command-card locked states.
- Fix docs that describe old tech buildings, old unlock paths, old production anchors, or stale
  resource/supply behavior.
- Update generated stats or deterministic checks only when needed to keep the docs reference
  accurate.
- Do not change economy, tech, or production gameplay.

## Suggested Evidence

- `docs/context/balance.md`
- `docs/design/balance.md`
- `docs/design/client-ui.md` command-card sections
- `docs/design/server-sim.md` production and command sections
- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/economy.rs`
- `server/crates/rules/src/faction.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/production.rs`
- `client/src/hud.js`
- `client/src/config.js`

Useful searches:

```bash
rg -n "requires|unlock|research|trained at|buildable|trainable|supply|resource|steel|oil|command card|locked" docs/design docs/context client/src server/crates -S
rg -n "build_requires|train_requires|research|UpgradeKind|queue|cost|supply" server/crates/rules/src server/crates/sim/src/game/services
```

## Verification

Run focused checks that match the final diff. Likely commands:

```bash
node scripts/check-faction-catalog-parity.mjs
node scripts/check-wiki.mjs
node scripts/check-docs-health.mjs
git diff --check
```

If command-card tests or client contract fixtures are touched, run the narrow relevant Node test
identified by the executor.

## Manual Testing Focus

In a local match or unit lab later, inspect command-card lock tooltips for advanced buildings,
training, and research. Confirm active docs use the same prerequisite names.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must list every tech/economy claim
changed, source evidence for each claim, verification run, and any command-card behavior that still
needs manual checking.

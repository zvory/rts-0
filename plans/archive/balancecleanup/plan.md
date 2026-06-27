# Balance Cleanup Plan

## Purpose

Reduce review and merge risk around the balance/config mirror without changing gameplay. This plan
exists because `plans/archive/hotspotcleanup/phase-10.md` found a safe path only if validation
expands before any source files move. It has been rebaselined against the current balance,
catalog, client-architecture, and hotspot guardrails on `origin/main`; earlier assumptions that the
structured mirror snapshot did not exist are stale. Every phase is behavior-preserving: no cost,
supply, sight, size, range, duration, ability, upgrade, faction, catalog, wiki-visible value, UI
affordance, or exported API may change.

## Gate Evidence

- `docs/design/balance.md` names `server/crates/rules/src/balance.rs`,
  `server/crates/rules/src/defs.rs`, and `server/crates/rules/src/faction.rs` as Rust-authoritative
  for shared scalars, stat records, faction catalogs, upgrades, and ability metadata.
- `server/src/config.rs` and `server/crates/sim/src/config.rs` are compatibility shims that
  re-export `rts_rules::balance::*`; they should not gain new server-shell or sim-only constants.
- `client/src/config.js` currently mixes Rust-owned mirror values with client-owned presentation
  data such as colors, fog alpha, camera defaults, labels, icons, layout hints, command budget
  constants, Kriegsia and Ekat client catalogs, and the fixture-only `phase2_empty_fixture` catalog.
- `scripts/check-faction-catalog-parity.mjs` already compares the Rust catalog dump and its
  `clientConfig` payload against client catalogs, unit/building stats, bodies, resource amounts,
  upgrade metadata, ability descriptors, Rust-owned ability effect fields, compact ability/order
  codes, playable faction exposure, and many client-visible scalar constants.
- `scripts/check-wiki.mjs` wraps generated wiki route/table checks and faction catalog parity for
  visible stat, faction, upgrade, and ability changes.
- `scripts/hotspot-analysis.mjs` still groups only the current exact balance/config paths; future
  `client/src/config/**`, `client/src/config_*.js`, and `server/crates/rules/src/balance/**` split
  files are not yet guaranteed to roll up into the `balance-and-config` group.
- `scripts/check-client-architecture.mjs` currently treats only `client/src/config.js` as the
  pinned shared rules mirror. A client config split must classify and allow any internal config
  modules deliberately, then update `docs/design/client-ui.md` and `docs/context/client-ui.md`.
- Remaining no-drift risk is not broad numeric coverage. The open gap is that
  `BASE_COMMAND_SUPPLY_CAP` and `COMMAND_CAR_SUPPLY_CAP_BONUS` are mirrored between
  `client/src/config.js` and the sim command service outside the Rust rules dump, and client-only
  presentation fields must stay excluded from Rust authority unless a later design explicitly moves
  them.
- `server/crates/rules/src/balance.rs` still contains some sim-only movement/recovery constants
  consumed through `server/crates/sim/src/config.rs`. The Rust cleanup must either keep public
  compatibility re-exports or stop for a design gate before removing exported names.

## Overall Constraints

- Keep Rust authoritative. The client remains a UI/render/fog mirror plus presentation owner.
- Keep `server/crates/rules/src/balance.rs`, `server/src/config.rs`,
  `server/crates/sim/src/config.rs`, and `client/src/config.js` as stable public import surfaces
  unless a phase explicitly stops for a new plan.
- Do not change any numeric gameplay value, catalog membership, ability descriptor, upgrade
  metadata, generated wiki value, command-card behavior, or compact protocol code.
- Do not move server-only simulation internals into client mirrors, and do not move client-only
  presentation labels, colors, camera defaults, fog alpha, or layout hints into Rust authority
  without a new design gate.
- Before any file split, close the remaining no-drift gaps by comparing server-owned command budget
  values and stable client config export names through structured data rather than brittle
  source-order snapshots.
- If a phase creates new split files, update `scripts/hotspot-analysis.mjs` and
  `docs/hotspot-analysis.md` in that same phase so the balance mirror remains one logical
  hotspot group.
- If a phase creates client config split files, update `scripts/check-client-architecture.mjs`,
  `docs/design/client-ui.md`, and `docs/context/client-ui.md` so the new files remain a pinned
  rules-mirror area instead of ad-hoc cross-area imports.
- Treat command budget as sim command-service policy unless Phase 2 deliberately documents a
  different owner. Do not move it into `rts_rules::balance` just to make parity easier.
- Do not remove exported Rust balance names when relocating sim-only constants unless that phase
  explicitly proves no downstream public import depends on them and updates the design docs.
- Use focused verification. At minimum, code-moving phases run
  `node scripts/check-faction-catalog-parity.mjs`, `node scripts/check-wiki.mjs`,
  `node tests/client_contracts.mjs`, and `git diff --check`; Rust-moving phases also run focused
  `rts-rules` tests.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, and waited on until GitHub reports the PR merged and the phase head is reachable
  from `origin/main`.
- After each phase, mark that phase document done in the implementation commit and provide a handoff
  that names exact verification, what moved or was protected, unchanged public APIs, remaining risk,
  next-phase guidance, and manual testing focus.

## Phase Summaries

### [Phase 1 - Rebaseline Guardrails And Grouping](phase-1.md)

Refresh the plan's guardrail baseline against current `origin/main` and protect future split paths
as one logical balance/config hotspot. This phase does not move balance source code; it verifies the
current parity payload, records only the remaining mirror gaps, and updates hotspot grouping for
future Rust and JS split files. It should stop with a no-go handoff if grouping or guardrail updates
would require balance behavior changes.

### [Phase 2 - Command Budget And Export Snapshot](phase-2.md)

Close the remaining split-blocking no-drift gaps before manual file movement. This phase should add
a structured comparison for the server-owned command budget constants and a split-safe public export
snapshot for `client/src/config.js`. It should leave runtime imports stable and reaffirm whether
validation-only checks remain safer than generated client config.

### [Phase 3 - Client Config Split](phase-3.md)

Split `client/src/config.js` behind the same public exports after command-budget and export-name
guardrails are in place. Keep Rust-owned mirror data separate from client-only presentation modules,
preserve all exported constants, `STATS`, `ABILITIES`, `UPGRADES`, `FACTION_CATALOGS`, fixture
catalog handling, helper functions, and import paths, and update the client architecture checker for
the new rules-mirror modules. This phase should be mechanical and must fail closed on any snapshot,
wiki, parity, architecture, or client-contract drift.

### [Phase 4 - Rust Balance Ownership Split](phase-4.md)

Split Rust balance internals by ownership while preserving `rts_rules::balance::*` as the stable
public surface. Keep `defs.rs`, `faction.rs`, `server/src/config.rs`, and
`server/crates/sim/src/config.rs` behavior-compatible, and treat sim-only movement/recovery
constants as a deliberate ownership decision rather than another balance submodule by default. This
phase should not rename exports, hide public API breaks, or move sim command-service policy into
rules just for cleanup convenience.

### [Phase 5 - Cleanup Closeout](phase-5.md)

Run the no-drift closeout after the client and Rust splits. This phase updates design and hotspot
tracking references created by the plan, reruns hotspot analysis, and compares before/after
structured balance outputs so reviewers can confirm the mirror stayed intact. It does not move
additional balance logic unless an earlier phase explicitly deferred a tiny mechanical cleanup.

## Phase Index

1. [Phase 1 - Rebaseline Guardrails And Grouping](phase-1.md)
2. [Phase 2 - Command Budget And Export Snapshot](phase-2.md)
3. [Phase 3 - Client Config Split](phase-3.md)
4. [Phase 4 - Rust Balance Ownership Split](phase-4.md)
5. [Phase 5 - Cleanup Closeout](phase-5.md)

## Non-Goals

- Do not tune gameplay, economy, combat, faction, upgrade, ability, map-resource, command-budget, or
  wiki-visible values.
- Do not make generated JavaScript config or client-authored config the authoritative source of
  balance.
- Do not combine this cleanup with protocol compact-code changes, generic-resource migration,
  faction admission changes, command semantics rewrites, or UI redesign.
- Do not move client-owned presentation fields into Rust authority unless a future design gate
  proves the product should own those fields server-side.

## Suggested Execution

Run one phase at a time and wait for each PR to merge before starting the next phase:

```bash
scripts/phase-runner.sh --plan balancecleanup phase-1 phase-2 phase-3 phase-4 phase-5 --pr --wait
```

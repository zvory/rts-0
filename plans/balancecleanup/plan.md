# Balance Cleanup Plan

## Purpose

Reduce review and merge risk around the balance/config mirror without changing gameplay. This plan
exists because `plans/archive/hotspotcleanup/phase-10.md` found a safe path only if validation expands before
any source files move. Every phase is behavior-preserving: no cost, supply, sight, size, range,
duration, ability, upgrade, faction, catalog, wiki-visible value, UI affordance, or exported API may
change.

## Gate Evidence

- `docs/design/balance.md` names `server/crates/rules/src/balance.rs`,
  `server/crates/rules/src/defs.rs`, and `server/crates/rules/src/faction.rs` as Rust-authoritative
  for shared scalars, stat records, faction catalogs, upgrades, and ability metadata.
- `server/src/config.rs` and `server/crates/sim/src/config.rs` are compatibility shims that
  re-export `rts_rules::balance::*`; they should not gain new server-shell or sim-only constants.
- `client/src/config.js` currently mixes Rust-owned mirror values with client-owned presentation
  data such as colors, fog alpha, camera defaults, labels, icons, and layout hints.
- `scripts/check-faction-catalog-parity.mjs` already compares the Rust catalog dump against client
  catalogs, unit/building stats, bodies, resource amounts, upgrade metadata, ability descriptors,
  and many client-visible scalar constants.
- `scripts/check-wiki.mjs` wraps generated wiki route/table checks and faction catalog parity for
  visible stat, faction, upgrade, and ability changes.
- Remaining risk is not numeric coverage alone: command budget constants are mirrored between the
  client and sim command service, and client-only presentation fields must stay excluded from Rust
  authority unless a later design explicitly moves them.

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
- Before any file split, add no-drift guardrails that compare Rust-owned mirror data and client
  exports through structured data rather than brittle source-order snapshots.
- If a phase creates new split files, update `scripts/hotspot-analysis.mjs` and
  `docs/hotspot-analysis.md` in that same phase so the balance mirror remains one logical
  hotspot group.
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

### [Phase 1 - Mirror Guardrail Baseline](phase-1.md)

Strengthen the tests and hotspot grouping that make later balance movement reviewable. This phase
does not move balance source code; it records the stable Rust and JS public surfaces, protects future
split paths as one balance/config hotspot, and identifies any Rust-owned client-visible values still
missing from structured parity. It should stop with a no-go handoff if guardrails cannot be improved
without changing balance behavior.

### [Phase 2 - Structured Mirror Snapshot](phase-2.md)

Build a no-drift comparison path before manual file splits. This phase should extend or supplement
the Rust rules/catalog dump and client-side checks so Rust-owned mirror data, including command
budget values if they remain player-visible, can be compared as structured data. It should leave
runtime imports stable and decide explicitly whether source generation is useful or whether
validation-only checks are the safer long-term guardrail.

### [Phase 3 - Client Config Split](phase-3.md)

Split `client/src/config.js` behind the same public exports after the structured mirror snapshot is
in place. Keep Rust-owned mirror data separate from client-only presentation modules, and preserve
all exported constants, `STATS`, `ABILITIES`, `UPGRADES`, `FACTION_CATALOGS`, helper functions, and
import paths. This phase should be mechanical and must fail closed on any snapshot, wiki, parity, or
client-contract drift.

### [Phase 4 - Rust Balance Split](phase-4.md)

Split Rust balance internals into focused modules while preserving `rts_rules::balance::*` as the
stable public surface. Keep `defs.rs`, `faction.rs`, `server/src/config.rs`, and
`server/crates/sim/src/config.rs` behavior-compatible, and move only constants or helper structs
whose ownership is already documented in `docs/design/balance.md`. This phase should not rename
exports or move sim-only behavior constants into the compatibility shims.

### [Phase 5 - Cleanup Closeout](phase-5.md)

Run the no-drift closeout after the client and Rust splits. This phase updates design and hotspot
tracking references created by the plan, reruns hotspot analysis, and compares before/after
structured balance outputs so reviewers can confirm the mirror stayed intact. It does not move
additional balance logic unless an earlier phase explicitly deferred a tiny mechanical cleanup.

## Phase Index

1. [Phase 1 - Mirror Guardrail Baseline](phase-1.md)
2. [Phase 2 - Structured Mirror Snapshot](phase-2.md)
3. [Phase 3 - Client Config Split](phase-3.md)
4. [Phase 4 - Rust Balance Split](phase-4.md)
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

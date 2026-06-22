# Phase 10 - Balance Mirror Cleanup Gate

Status: planned.

## Goal

Decide whether balance/config cleanup is safe enough to become its own phase-runner plan. This phase
is a design gate: it should not move balance or config code unless the user explicitly redirects it
into an implementation phase.

## Scope

- Read `docs/context/balance.md` and the relevant sections of `docs/design/balance.md`.
- Re-read the balance/config sections of `plans/hotspots/responsibility-map.md` and
  `plans/hotspots/extraction-candidates.md`.
- Inspect the current boundaries among `server/crates/rules/src/balance.rs`,
  `server/crates/rules/src/defs.rs`, `server/crates/rules/src/faction.rs`, `server/src/config.rs`,
  `server/crates/sim/src/config.rs`, `client/src/config.js`, wiki checks, faction parity checks, and
  config client contracts.
- Prefer validation or generation of client-visible mirror data before manual rearrangement.
- If a safe path exists, create a separate `plans/<one-word-name>/` balance cleanup plan with phase
  files. Prefer a short name such as `balancecleanup` if unused.
- If no safe path exists, mark this phase done with a clear no-go decision and evidence in this
  phase file or a small companion note.

## Touch Points

- `plans/hotspotcleanup/phase-10.md`
- possible new `plans/balancecleanup/plan.md` and phase files
- no balance/config source files unless the user explicitly changes this phase from gate to
  implementation

## Constraints

- Do not change any cost, supply, sight, size, range, duration, ability, upgrade, faction, catalog, or
  generated wiki value.
- Do not make the client mirror authoritative.
- Do not mix server-only balance internals with client presentation descriptors.
- Do not rename exported constants without checking Rust shims and JS imports.
- Any future balance cleanup plan must require parity/wiki checks and an explicit no-numeric-drift
  review step.

## Verification

- `node scripts/check-faction-catalog-parity.mjs` if any generated evidence or catalog mirror file is
  touched
- `node scripts/check-wiki.mjs` if docs/wiki-visible balance references are touched
- `node tests/client_contracts.mjs` if config contract files are touched
- `node scripts/check-docs-health.mjs` if a new plan or docs links are added
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected because this is a design gate. Manually review that the proposed
plan, if created, proves no numeric drift and keeps Rust authoritative with the client as a mirror.

## Handoff

After implementation, mark this phase done and summarize either the new balance cleanup plan path or
the no-go reason, the exact mirror invariants that blocked movement, commands run, and what human
decision is needed before balance/config source files move.

# Phase 1 - Mirror Guardrail Baseline

Status: planned.

## Goal

Make later balance/config movement safe to review by tightening current guardrails and group
tracking before any balance or config source code is moved.

## Scope

- Inspect current public exports from `server/crates/rules/src/balance.rs`,
  `server/src/config.rs`, `server/crates/sim/src/config.rs`, and `client/src/config.js`.
- Add or strengthen focused assertions that lock stable public balance/config surfaces used by
  downstream Rust and JS modules.
- Update `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` so future
  `server/crates/rules/src/balance/**`, `client/src/config/**`, or `client/src/config_*.js` split
  files stay in the `balance-and-config` group.
- Record any Rust-owned client-visible values that are not yet covered by structured parity,
  especially command budget values mirrored between the sim command service and client config.
- Do not move Rust or JS balance/config source code in this phase.

## Touch Points

- `scripts/check-faction-catalog-parity.mjs`
- `tests/client_contracts/config_contracts.mjs` or focused config contract files, only for public
  surface assertions that do not belong in parity
- `scripts/hotspot-analysis.mjs`
- `plans/hotspots/group-map.md`
- `docs/design/balance.md`, only for boundary wording or guardrail inventory gaps

## Constraints

- Preserve every balance number, catalog row, ability descriptor, upgrade descriptor, wiki-visible
  value, and exported API.
- Do not add brittle source-order snapshots that make later mechanical moves noisy without checking
  behavior.
- Prefer structured assertions against imported Rust/JS exports and the rules catalog dump.
- If the needed guardrail requires source generation or a new contract schema, stop and report that
  as Phase 2 scope instead of forcing it into this phase.

## Verification

- `node scripts/check-faction-catalog-parity.mjs`
- `node tests/client_contracts.mjs` if client config contracts are touched
- `node scripts/check-docs-health.mjs` if docs or plan links are touched
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected. Manually review that public Rust exports, public JS exports,
known mirror gaps, and future split paths are protected before source files move.

## Handoff

Mark this phase done only after committing the guardrail and group-map changes. Summarize which
exports or gaps are now protected, which future split paths remain grouped, which verification
passed, and whether Phase 2 can build the structured no-drift snapshot.

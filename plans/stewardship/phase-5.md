# Phase 5 - Give Rule Identities One Typed Owner

Status: Incomplete.

## Objective

Make `rts-rules` the typed authority for upgrade and ability identities, stable string ids, and
ability target modes. Preserve gameplay and protocol behavior while removing competing exhaustive
identity registries from the simulation.

## Work

- Define the typed upgrade and ability identities and their stable ids in `rts-rules`, adjacent to
  the declarative catalog data they identify.
- Make ability target mode use that same rules-owned type.
- Remove competing exhaustive identity and string-decoding registries from `rts-sim`. Retain thin
  compatibility re-exports at existing simulation module paths when that materially reduces caller
  churn or protects the public seam guarded in Phase 4.
- Keep simulation-specific planner codes, effect hooks, order execution, and ability dispatch in
  the simulation.
- Make catalog lookup return a normal error or typed absence rather than panic when definitions
  drift. Do not replace this with another unreachable or expect-based path.
- Add focused tests proving every typed identity round-trips through its stable id, resolves to one
  catalog row, and has total simulation handling where required.
- Update the server-simulation design source of truth to name the rules-owned identity boundary.

## Non-goals

- Do not change balance values, ability effects, research availability, catalog membership, wire
  strings, replay formats, or fog behavior.
- Do not consolidate AI profile registries.
- Do not introduce a generalized registry framework or broad compatibility layer.
- Do not move command-list limits or reconstruction behavior; Phases 6 and 7 own those jobs.

## Likely Touch Points

- `server/crates/rules/src/faction.rs` and a small adjacent rules module if useful
- `server/crates/sim/src/game/ability.rs`
- `server/crates/sim/src/game/upgrade.rs`
- direct typed-kind consumers only where compatibility re-exports are insufficient
- focused `rts-rules` and `rts-sim` tests
- `docs/design/server-sim.md`

## Verification

- Focused Rust tests for stable-id round trips, uniqueness, catalog resolution, and total effect
  handling.
- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -p rts-rules -p rts-sim`
- `node scripts/check-wiki.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-crate-boundaries.mjs`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `git diff --check`

## Manual Test Focus

In one local session, confirm representative research plus one self-targeted and one world-targeted
ability behave as before.

## Handoff

Mark this phase done in its implementation commit. Report the final typed owner, compatibility
re-exports retained, panic-on-drift path removed, and focused identity evidence. Tell the Phase 6
agent that command unit-list limits remain duplicated and are its only server contract target.

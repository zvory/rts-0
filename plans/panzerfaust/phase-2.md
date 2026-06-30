# Phase 2 - Hidden Vocabulary, Balance, And Protocol Contract

## Phase Status

Status: done.

## Objective

Create the shared Panzerfaust vocabulary and hidden contract needed by later runtime phases without
making the unit trainable in normal matches. This phase should let Rust and JavaScript agree on the
kind, numbers, parser vocabulary, and fog-safe event strategy before combat behavior depends on it.

## Scope

- Add a Panzerfaust entity kind with stable ids and mirrored Rust/JS conversion paths.
- Add hidden rules-owned unit metadata for the approved checklist values:
  - 60 steel / 15 oil, 1 supply, 400-tick build time.
  - 45 HP, 8-tile sight, 9 px collision/selection/render radius.
  - 1.44 px/tick loaded speed, ordinary infantry pathing, ordinary infantry passability.
  - Loaded weapon constants: 3-tile base range, 60 damage, Tank-only target filter,
    15-tick windup, 15-tick travel, 15-tick recovery, no reload.
  - Methamphetamines setup/recovery timing constants and Entrenchment range interaction vocabulary
    if those are easiest to centralize with the hidden rules data.
- Keep the Panzerfaust out of normal player exposure:
  - Do not add it to the current faction's trainable catalog yet.
  - Do not add it to product-playable lab spawn catalogs before Phase 4 unless this phase also
    adds a deliberate hidden-inspection policy. Current lab spawn lists derive from faction catalog
    units, so catalog membership can expose the unit before production does.
  - Do not show a Barracks command-card button yet.
  - Do not let AI build plans choose it yet.
  - Do not require a human-facing production path to work in this phase.
- Decide and document the wire representation for:
  - Panzerfaust unit kind in snapshots.
  - Loaded-shot launch, travel, impact, and same-id conversion feedback.
  - Whether existing attack/death/state events are sufficient or new event tags/fields are needed.
  - How event payloads avoid leaking hidden target ids, hidden impact positions, or hidden paths.
- Add parser placeholders or ignored-safe client handling for any new event fields introduced here,
  so Phase 3 can emit them without breaking clients.
- Update protocol and balance docs for the new hidden contract. Do not defer these source-of-truth
  updates to a later cleanup phase if this phase changes the wire or balance mirror.
- Do not implement Panzerfaust attack behavior, production exposure, command-card UI, final visuals,
  audio, or AI training behavior in this phase.

## Expected Touch Points

- `server/crates/rules/src/kind.rs`
- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/balance.rs`
- `server/crates/rules/src/balance/*.rs`
- `server/crates/rules/src/combat.rs`
- `server/crates/rules/src/faction.rs`
- `server/crates/rules/src/bin/dump-faction-catalog.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/crates/protocol/src/contract_metadata.rs`
- `server/crates/protocol/src/compact_snapshot.rs`
- `server/crates/sim/src/protocol.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `client/src/protocol_constants.js`
- `client/src/protocol_snapshot.js`
- `client/src/config.js`
- `client/src/config/*.js`
- `docs/design/protocol.md`
- `docs/design/balance.md`
- `plans/panzerfaust/checklist.md`

## Edge Cases To Cover

- The new kind round-trips through Rust stable ids, protocol DTOs, compact snapshot metadata if
  touched, and JS parsing.
- The Panzerfaust is not trainable from Barracks in normal current-faction command cards after this
  phase.
- The Panzerfaust does not accidentally appear in product-playable lab spawn lists unless Phase 2
  explicitly chooses and documents hidden lab exposure.
- Generic faction/catalog parity checks either exclude the hidden unit from current production or
  explicitly describe its hidden state.
- Existing Rifleman, Machine Gunner, Anti-Tank Gun, Tank, Entrenchment, and Methamphetamines rules
  are unchanged.
- Clients that receive no Panzerfaust entities or events behave exactly as before.

## Verification

- Focused Rust tests for kind stable id parsing, hidden stats, attack constants, and current-faction
  non-exposure.
- Protocol representative snapshot and compact snapshot tests if the kind or event shape touches
  those paths.
- `node tests/protocol_parity.mjs`.
- `node tests/client_contracts/protocol_contracts.mjs` if protocol parsing changes.
- `node scripts/check-faction-catalog-parity.mjs` if catalog or config mirror data changes.
- `node scripts/check-wiki.mjs` if generated stats or wiki surfaces change.
- `node scripts/check-docs-health.mjs` if docs change.
- `git diff --check`.

## Manual Test Focus

No manual gameplay test is required for this hidden contract phase. If a smoke check is performed,
start a normal match and confirm Barracks still exposes only the pre-existing trainable units.

## Handoff Expectations

Name the final Panzerfaust kind id, Rust/JS conversion paths, hidden-stat location, and event or
snapshot vocabulary chosen for later phases. Call out exactly how Phase 3 should spawn or construct
Panzerfaust entities in tests while the unit remains hidden from normal production.

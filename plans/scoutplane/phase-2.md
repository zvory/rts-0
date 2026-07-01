# Phase 2 - Hidden Vocabulary, Balance, And Protocol Contract

## Phase Status

Status: done.

## Objective

Create the shared Scout Plane vocabulary and hidden contract needed by later runtime phases without
making the unit trainable in normal matches. Rust and JavaScript should agree on the kind, approved
numbers, parser vocabulary, and fog-safe state strategy before launch, movement, upkeep, or UI
exposure depends on it.

## Scope

- Add a Scout Plane entity kind with stable ids and mirrored Rust/JS conversion paths.
- Add hidden rules-owned unit metadata for the approved requirements values:
  - 50 Steel / 50 Oil, 0 supply, 600-tick build time.
  - 40 HP, 12-tile sight, 2 px/tick movement.
  - no attack range, no damage, no cooldown, no projectile, no combat target filter.
  - 4-tile orbit radius.
  - upkeep of one Pump Jack average, represented as 1 Oil every 20 ticks.
  - 5-second fuel tank, rounded to 8 Oil worth of reserve for integer accounting.
- Decide and document the unit's body metadata for hidden parsing/render support:
  - collision/pathing body should not reserve or block occupancy.
  - selection/render size should be explicit enough for client hit testing and health bars.
  - visual size may be rough, but must not be implied or left to accidental defaults.
- Keep the Scout Plane out of normal player exposure:
  - Do not add it to the current faction's City Centre trainables yet.
  - Do not show a City Centre command-card button yet.
  - Do not let AI build plans choose it.
  - Do not require a human-facing production path to work in this phase.
- Decide and document the wire representation for:
  - Scout Plane unit kind in snapshots and compact snapshots.
  - Any persistent entity state needed later for orbit center, fuel/upkeep, or dismiss affordances.
  - Whether an existing `move` command is enough for retargeting, or whether a plane-specific command
    is required for dismiss only.
  - Whether dismissal can use existing cancel/ability command vocabulary or needs a new command tag.
  - How queued movement and hidden private state avoid leaking to enemies.
- Add parser placeholders or ignored-safe client handling for any new snapshot fields introduced
  here, so later phases can emit them without breaking clients.
- Update protocol and balance docs for the new hidden contract. Do not defer source-of-truth updates
  to a later cleanup phase if this phase changes wire shape or client-visible balance mirrors.
- Do not implement Scout Plane launch, orbit movement, fog stamping, upkeep, dismissal, UI exposure,
  final visuals, audio, or AI behavior in this phase.

## Expected Touch Points

- `server/crates/rules/src/kind.rs`
- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/balance.rs`
- `server/crates/rules/src/balance/*.rs`
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
- `plans/scoutplane/requirements.md` only if implementation discovers a requirement ambiguity

## Edge Cases To Cover

- The new kind round-trips through Rust stable ids, protocol DTOs, compact snapshot metadata if
  touched, and JS parsing.
- The Scout Plane remains absent from normal current-faction City Centre train buttons after this
  phase.
- The Scout Plane does not accidentally appear in product-playable lab spawn lists unless this phase
  explicitly chooses and documents hidden lab exposure.
- Generic faction/catalog parity checks either exclude the hidden unit from current production or
  explicitly describe its hidden state.
- Existing Worker, Scout Car, Tank, Command Car, City Centre, Gun Works, and Vehicle Works rules are
  unchanged.
- Clients that receive no Scout Plane entities behave exactly as before.

## Verification

- Focused Rust tests for kind stable id parsing, hidden stats, non-combat classification, hidden
  production non-exposure, and any new balance constants.
- Protocol representative snapshot and compact snapshot tests if the kind or entity fields touch
  those paths.
- `node tests/protocol_parity.mjs` if protocol vocabulary changes.
- `node tests/client_contracts/protocol_contracts.mjs` if protocol parsing changes.
- `node scripts/check-faction-catalog-parity.mjs` if catalog or config mirror data changes.
- `node scripts/check-wiki.mjs` if generated stats or wiki surfaces change.
- `node scripts/check-docs-health.mjs` if docs change.
- `git diff --check`.

## Manual Test Focus

No manual gameplay test is required for this hidden contract phase. If a smoke check is performed,
start a normal match and confirm City Centre still exposes only pre-existing trainable units.

## Handoff Expectations

Name the final Scout Plane kind id, Rust/JS conversion paths, hidden-stat location, body/selection
metadata choice, and any snapshot or command vocabulary chosen for later phases. Call out exactly how
Phase 3 should create Scout Plane entities in tests while the unit remains hidden from normal
production.

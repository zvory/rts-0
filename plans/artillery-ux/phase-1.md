# Phase 1 - Mirrored Blanket Fire Contract Skeleton

## Phase Status

Status: done.

## Objective

Create the mirrored `blanketFire` command identity that later phases can use for real gameplay.
This phase should add vocabulary, compact codes, faction/catalog descriptors, and client mirror data
without exposing a usable command-card button or changing current Point Fire runtime behavior.

## Scope

- Add a Rust rules catalog constant for `blanketFire` and a faction ability entry carried only by
  Artillery.
- Assign stable compact protocol codes for the `blanketFire` ability and order stage. Use the next
  available codes unless implementation evidence shows an existing reserved code must be reused.
- Mirror `blanketFire` in protocol metadata, JS protocol constants, JS config/faction mirrors, and
  catalog parity expectations.
- Add `ARTILLERY_BLANKET_RADIUS_TILES = 15` to the authoritative rules balance surface and mirror it
  to the client where artillery ability descriptors or preview code will consume it.
- Keep `blanketFire` hidden from normal command-card exposure until the runtime and client targeting
  phases are ready. If the catalog shape requires a descriptor, mark it non-command-card or guard it
  with an explicit implementation note.
- Do not add a server runtime effect, shell sampling, command-card button, input handler, or order
  execution behavior in this phase.
- Update `docs/design/protocol.md` and `docs/design/balance.md` to describe the new reserved
  mirrored identity and blanket radius, while clearly stating that the gameplay runtime lands in
  later phases.

## Expected Touch Points

- `server/crates/rules/src/faction.rs`
- `server/crates/rules/src/balance.rs` and/or `server/crates/rules/src/balance/abilities.rs`
- `server/crates/protocol/src/contract_metadata.rs`
- `client/src/protocol_constants.js`
- `client/src/protocol.js`
- `client/src/config.js`
- `client/src/config/rules_mirror.js`
- `client/src/config/factions.js`
- `scripts/check-faction-catalog-parity.mjs`
- `tests/protocol_parity.mjs`
- `tests/client_contracts/config_contracts.mjs`
- `docs/design/protocol.md`
- `docs/design/balance.md`

## Edge Cases To Cover

- Protocol parity recognizes `blanketFire` as a distinct ability and order stage, not an alias for
  `pointFire`.
- Faction catalog parity proves the Rust catalog and client mirror agree on the new id, carriers,
  target mode, queueability, radius, cost, cooldown, and compact codes.
- Existing Point Fire descriptors, compact codes, hotkeys, and command-card behavior remain
  unchanged.
- Hidden/non-exposed Blanket Fire cannot accidentally be selected by the normal HUD before the
  runtime exists.
- Existing compact snapshot decoding and order-plan decoding still handle older Point Fire markers.

## Verification

- `node tests/protocol_parity.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `node tests/client_contracts.mjs --grep config` if the harness supports grep, otherwise the
  focused config contract command used by nearby client contract work.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

No gameplay manual test is required for this hidden contract phase. If a local smoke test is
performed, confirm the artillery command card still shows the current Point Fire behavior and does
not expose a broken Blanket Fire button.

## Handoff Expectations

List the exact ability and order-stage codes chosen for `blanketFire`. Note whether the descriptor
is hidden by `command_card: false` or another guard, and call out every mirror that was updated so
the server runtime executor can rely on the identity.

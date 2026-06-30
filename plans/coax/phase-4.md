# Phase 4 - Attack Event Weapon Identity Plumbing

## Phase Status

Status: pending.

## Objective

Make attack events capable of carrying fog-safe weapon identity across Rust contracts, compact
snapshots, JavaScript protocol decoding, and fallback client feedback. Existing attacks should
continue to look and sound exactly as they do today.

## Scope

- Add optional `weapon_kind: Option<String>` to semantic `Event::Attack`, serialized as
  `weaponKind` on JSON/JS.
- Emit default weapon ids for current direct-fire attacks, including `tank_cannon` for Tanks. It is
  acceptable to omit default ids only if the phase explicitly documents why and all fallback tests
  cover both missing and present hints.
- Update compact attack event encoding to include a trailing `weaponKind` slot after `toPos`.
- Bump `COMPACT_SNAPSHOT_VERSION` unless the final implementation proves old and new compact
  decoders can safely share a version.
- Add a compact weapon-kind code table if that matches local protocol patterns; otherwise document
  why plain strings are used in the trailing slot.
- Update `server/crates/protocol/src/lib.rs`, compact metadata, JS constants/decoding, protocol
  contract tests, and protocol docs.
- Teach client audio and visual-effect helpers to accept `weaponKind` while mapping missing/default
  hints to current attacker-kind behavior.
- Preserve the exact attack-event recipient set. Weapon identity may only be added to events that
  would already be projected.
- Preserve replay and legacy fixture compatibility for attack events without `weaponKind`.

## Out Of Scope

- No `tank_coax` live firing.
- No Tank rig coax barrel.
- No weapon-specific feedback differences yet.
- No target acquisition or priority changes.
- No changes to `Event::Overpenetration`; it remains a secondary event without shooter/audio/recoil.

## Expected Touch Points

- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/compact_snapshot.rs`
- `server/crates/protocol/src/contract_metadata.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs` if adapter exports are affected
- `server/crates/sim/src/game/services/combat/events.rs`
- `server/crates/sim/src/game/services/combat/damage.rs`
- `server/crates/sim/src/game/services/commands.rs` for artillery self-attack events
- `client/src/protocol_constants.js`
- `client/src/protocol_snapshot.js`
- `client/src/protocol.js`
- `client/src/combat_audio.js`
- `client/src/match_combat_audio.js`
- `client/src/state_visual_effects.js`
- `client/src/renderer/feedback.js`
- `tests/protocol_parity.mjs`
- `tests/client_contracts/protocol_contracts.mjs`
- `tests/client_contracts/audio_contracts.mjs`
- `tests/client_contracts/state_input_contracts.mjs`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`

## Edge Cases To Cover

- Existing compact attack event forms without `weaponKind` still decode.
- New compact attack event forms with default weapon identity decode.
- Missing `weaponKind` and default `weaponKind` render/play exactly like current mainline.
- `tank_cannon` from a Tank still plays cannon audio and starts cannon recoil.
- Artillery self-reveal attack events still do not create tracers or combat audio.
- Weapon hints do not change attack-event projection, fog gating, or replay visibility.
- Projection unions do not duplicate the same attack because one recipient has `None` and another
  has an explicit default id.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-protocol compact_snapshot`
- `node tests/protocol_parity.mjs`
- `node tests/client_contracts/protocol_contracts.mjs`
- Focused client audio/visual-effect contract tests proving missing/default weapon hints preserve
  current feedback.
- `node scripts/check-client-architecture.mjs` if client module wiring changes.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  architecture boundaries move.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Run a short local match or dev scenario only if client feedback code changes materially. Confirm
Rifleman, Machine Gunner, Anti-Tank Gun, Scout Car, Tank, Mortar, and Artillery attack feedback
still sounds and looks like current mainline.

## Handoff Expectations

Name the final attack-event weapon field, compact slot/encoding rule, compact version decision, and
weapon ids emitted for existing attacks. Describe exactly how Phase 8 should distinguish
`tank_cannon` from `tank_coax` without breaking missing-hint fallbacks.

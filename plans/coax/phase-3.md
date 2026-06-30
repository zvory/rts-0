# Phase 3 - Weapon Cooldown And Event Plumbing

## Phase Status

Status: pending.

## Objective

Prepare cooldown and feedback plumbing for more than one weapon per entity while preserving current
single-weapon gameplay. This phase should also make attack events capable of carrying optional
weapon identity so clients can eventually distinguish Tank cannon and Tank coax shots.

## Scope

- Replace or wrap the single `CombatState::attack_cd` usage with a weapon-aware cooldown interface.
  The stored shape may remain compact, but callers should be able to tick, read, and set cooldowns
  by weapon identity.
- Preserve the existing `attack_cd()` and `set_attack_cd()` API where needed as default-weapon
  compatibility shims, or migrate callers in a single behavior-preserving pass.
- Make the normal combat system use the default weapon cooldown through the new interface.
- Extend semantic attack events with an optional weapon identity field such as `weapon` or
  `weaponKind`. Use stable string ids that are safe to expose in fog-gated event payloads.
- Update compact snapshot event encoding, compact slot metadata, JS protocol constants/decoding,
  and protocol docs for the optional weapon field.
- Teach client audio and visual feedback helpers to accept a weapon hint but fall back to the
  existing attacker-kind behavior when the hint is missing or is the default weapon.
- Keep all current rendered/audio behavior identical for existing attacks.
- Do not add live Tank coax firing, Tank rig coax art, or coax-specific client feedback in this
  phase.

## Expected Touch Points

- `server/crates/sim/src/game/entity/state.rs`
- `server/crates/sim/src/game/entity/entity.rs`
- `server/crates/sim/src/game/services/combat/mod.rs`
- `server/crates/sim/src/game/services/combat/damage.rs`
- `server/crates/sim/src/game/services/combat/events.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/compact_snapshot.rs`
- `server/crates/protocol/src/contract_metadata.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs` if adapter exports are affected
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

- Existing compact attack event forms without a weapon field still decode.
- New compact attack event forms with default weapon identity decode and render/play exactly like
  old events.
- Weapon hints do not change attack-event projection. The same recipients should get the same
  event they got before, plus only the safe weapon id.
- Default cooldown behavior remains unchanged for all current combatants.
- Firing-reveal response delay still applies to the default weapon exactly as before.
- Replays and snapshot fixtures do not break on missing optional weapon hints.

## Verification

- `node tests/protocol_parity.mjs`.
- `node tests/client_contracts/protocol_contracts.mjs`.
- Focused Rust protocol representative snapshot tests for optional attack weapon identity.
- Focused Rust combat tests for default cooldown parity.
- Focused client audio/visual-effect contract tests proving default weapon hints preserve current
  feedback.
- `node scripts/check-client-architecture.mjs` if client module wiring changes.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  architecture boundaries move.
- `node scripts/check-docs-health.mjs`.
- `git diff --check`.

## Manual Test Focus

Run a short local match or dev scenario only if client feedback code changes materially. Confirm
Rifleman, Machine Gunner, Anti-Tank Gun, Scout Car, Tank, and artillery-related attack feedback
still sound and look like current mainline.

## Handoff Expectations

Name the attack-event weapon field, the compact slot position or encoding rule, and the weapon ids
currently emitted. Describe the cooldown API Phase 4 should use for `tank_coax` without disturbing
the cannon cooldown.

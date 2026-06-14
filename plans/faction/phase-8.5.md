# Phase 8.5 - Architecture Cleanup Before Second-Faction Spec

Status: Done.

## Objective

Clean up the architectural drift found after Phases 5.5 through 8 before Phase 9 starts the real
second-faction brief. This phase is a cleanup and guardrail repair pass, not a gameplay-content
phase. It should keep Kriegsia behavior unchanged while making the faction architecture ready for
approved Ekaterina design work.

## Review Findings To Address

- `node scripts/check-faction-assumptions.mjs` currently fails because direct current-faction
  special-case budgets are exceeded in `server/crates/rules/src/faction.rs` and
  `server/crates/sim/src/game/services/commands.rs`.
- Replay artifact faction/loadout validation is duplicated between match-history compatibility
  checks and the authoritative replay-session validator. The authoritative replay validator rejects
  loadout records for unknown players, but the match-history compatibility helper can report the
  same artifact as replayable.
- `PointFire` remains intentionally bespoke even after the ability registry and effect-hook phases.
  It is faction-guarded today, but it needs an explicit extension policy before Phase 11 adds a
  real second-faction signature ability.
- The client faction catalog remains hand-authored in `client/src/config.js` with parity coverage.
  That is acceptable for the fixture phase, but Phase 9 should know whether Phase 10 may keep the
  checked mirror or must switch to generation before real faction data grows.

## Scope

- Restore the faction assumption checker to green.
  - Prefer reducing new direct current-faction special cases by routing through catalog helpers or
    named ability/economy helpers.
  - If a count increase is genuinely intentional catalog data, raise the ratchet with a short,
    concrete reason in the same change.
  - Do not weaken the checker by broadening file allowlists or deleting the high-risk budgets.
- Centralize replay artifact faction/loadout validation so match-history availability checks,
  replay launch, replay branch launch, and any dev replay path agree on:
  - duplicate player ids
  - duplicate loadout records
  - loadouts for unknown players
  - missing per-player loadouts
  - missing, unknown, fixture-only, or unsupported faction ids
  - loadout faction mismatches
  - empty or unknown loadout ids
- Add focused tests for the validation mismatch, including an artifact with an extra loadout record
  for a non-existent player.
- Audit `PointFire` and the ability effect-hook split.
  - Keep the current behavior if it is still clearer than forcing artillery into a generic hook.
  - Document the rule for Phase 11: new signature abilities should add a narrow explicit hook or a
    named one-off path with faction validation, cost validation, and fog-safe event tests.
  - Add or adjust tests only where the audit finds an uncovered bypass.
- Decide and document the client catalog path for Phase 10.
  - If the current checked mirror is kept, name `node scripts/check-faction-catalog-parity.mjs` as
    the required gate and state that every real-faction descriptor must be compared against the Rust
    dump.
  - If generation is required first, add a follow-up before Phase 10 rather than hiding it in the
    real-faction implementation phase.
- Update the relevant design docs and context capsules only where contracts or section structure
  change.

## Non-Goals

- Do not add Ekaterina catalog entries, lobby selection, units, buildings, abilities, art, AI
  behavior, or prediction support.
- Do not implement the Phase 9 faction brief or rules/balance spec.
- Do not redesign the ability system into a generic scripting engine.
- Do not migrate to generic resources or server-sent command-card descriptors.
- Do not change Kriegsia balance or intended current-faction command-card behavior.

## Expected Touch Points

- `scripts/check-faction-assumptions.mjs`
- `server/crates/rules/src/faction.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/src/main.rs`
- `server/src/lobby/room_task.rs`
- replay artifact validation helpers if a shared helper/module is introduced
- focused tests under `server/src/main.rs` and `server/src/lobby/room_task.rs`
- `server/crates/sim/src/game/ability.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `docs/design/faction-architecture-inventory.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md` if the client catalog policy changes

## Verification

- `node scripts/check-faction-assumptions.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- Focused Rust tests for shared replay artifact faction/loadout validation.
- Focused Rust tests for unknown-player loadout rejection in both match-history compatibility and
  replay-session launch paths.
- Focused ability tests if the `PointFire` audit changes any command, cost, cooldown, or event
  path.
- `node tests/protocol_parity.mjs` if replay/start payload contracts move.
- `node tests/hud_command_card.mjs` and `node tests/hotkey_profiles.mjs` only if client catalog or
  command-id policy changes.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if
  shared replay validation or ability helpers move across sim/server seams.
- `git diff --check`

## Manual Testing Focus

No gameplay manual testing should be required unless this phase touches live replay launch or
ability execution behavior. If it does, manually verify one existing Kriegsia replay can launch,
branch, and play back, and verify Artillery Point Fire still targets, spends Steel, emits visible
events, and respects fog as before.

## Handoff Expectations

The handoff must state whether the faction assumption checker is green and list any ratchet changes.
It must identify the single replay validation path that future code should call, describe the
`PointFire`/ability hook policy for Phase 11, and state whether Phase 10 may keep the checked
client catalog mirror or must first introduce generated client catalog data.

## Player-Facing Outcome

No intended gameplay change. This phase reduces architecture drift before real second-faction
design and implementation begin.

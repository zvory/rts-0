# Phase 7 - Tank Coax Server Runtime And Minimum Feedback

## Phase Status

Status: done.

## Objective

Implement the server-authoritative Tank coax firing behavior using the refactored weapon profile,
damage, cooldown, event, target facts, and priority-policy surfaces. This phase makes Tanks actually
fire the coax and includes the minimum weapon-specific client feedback needed for the live behavior
to be shippable: `tank_coax` must use machine-gun-scale feedback and must not look or sound like a
Tank cannon. Detailed Tank rig muzzle anchoring and art polish remain in Phase 8.

## Scope

- Add the live `tank_coax` weapon profile with 6-tile range, 4 damage, 6-tick cooldown, small-arms
  weapon class, direct-fire legality, and overpenetration enabled.
- Give Tanks an additional secondary weapon without changing their default `tank_cannon` profile.
- Tick and reset `tank_coax` cooldown independently from `tank_cannon`.
- Implement a Tank-only secondary firing pass that evaluates legal targets inside the current
  authoritative turret/weapon facing arc.
- Run the secondary coax pass after the normal Tank cannon aim/firing/relaxation logic for that Tank
  has completed for the tick. The coax uses that post-main-pass authoritative weapon facing as a
  read-only snapshot and must not call rotation helpers or update desired weapon facing.
- Gate coax shots to targets within 10 degrees on either side of current turret facing. Use a named
  constant.
- Reuse direct-fire hostile, visibility, smoke, line-of-sight, targetability, resource-node
  exclusion, friendly-hard-blocker safety checks, and the intended-target legality mode that rejects
  targets where the shot would resolve to an intervening enemy hard blocker.
- Use the machine-gun-like priority policy from Phase 6: infantry-priority first, fallback legal
  targets second, distance/id ties.
- Emit `Event::Attack` with `weaponKind: "tank_coax"`. When the Tank cannon and coax both fire in
  the same tick, emit/process the cannon attack first and the coax attack second.
- Apply coax overpenetration with small-arms damage and coax overpenetration policy.
- Keep `Event::Overpenetration` as a secondary damage event with no separate shooter, audio, recoil,
  tracer, or weapon identity. Coax overpenetration tail presentation comes from the originating coax
  `attack` event.
- Add minimum client feedback for `tank_coax`: machine-gun combat sound, small muzzle flash/tracer,
  small overpenetration tail scale derived from the attack feedback record, and no Tank cannon
  recoil. It may use the existing Tank muzzle/reach approximation until Phase 8 replaces the origin
  with the authored coax barrel anchor.
- Add a minimal dev or lab scenario that makes in-arc coax inspection practical.
- Preserve Tank cannon target selection, turret rotation, stationary range ramp, cooldown, firing
  reveal, overpenetration, movement/path retention, and event behavior.
- Collect factual patch-note bullets for the new live gameplay and temporary feedback limitations.

## Out Of Scope

- No client Tank rig coax barrel.
- No final coax-specific muzzle anchor or authored barrel; Phase 8 owns the Tank rig origin polish.
- No command-card UI, toggle, upgrade, research, range display, cost/supply/sight/trainability
  change, or balance tuning outside the approved coax profile.
- No change to explicit Tank cannon intent or pathing.

## Expected Touch Points

- `server/crates/rules/src/combat.rs`
- `server/crates/rules/src/defs.rs` or the weapon-profile module from Phase 1
- `server/crates/sim/src/game/entity/state.rs`
- `server/crates/sim/src/game/entity/entity.rs`
- `server/crates/sim/src/game/services/combat/mod.rs`
- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/priority.rs`
- `server/crates/sim/src/game/services/combat/weapons.rs`
- `server/crates/sim/src/game/services/combat/damage.rs`
- `server/crates/sim/src/game/services/combat/events.rs`
- `server/crates/sim/src/game/services/combat/tests*.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/**` or equivalent lab/dev scenario setup
- `client/src/combat_audio.js`
- `client/src/match_combat_audio.js`
- `client/src/state_visual_effects.js`
- `client/src/renderer/feedback.js`
- `tests/client_contracts/audio_contracts.mjs`
- `tests/client_contracts/state_input_contracts.mjs`
- `tests/client_contracts/renderer_feedback_contracts.mjs`
- `docs/design/balance.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md` if minimum feedback contracts are documented here instead of Phase 8

## Edge Cases To Cover

- In-arc Worker, Rifleman, or Machine Gunner takes 4 base small-arms damage from coax.
- Ekat, Golem, Mortar Team, Artillery, and Anti-Tank Gun are not infantry-priority for coax.
- Armored fallback targets take reduced small-arms damage, not Tank AP damage.
- Coax overpenetration hits secondary targets with small-arms damage and does not use Tank cannon
  facing multipliers.
- Coax prioritizes in-arc infantry-priority targets over fallback targets.
- Coax fires at fallback vehicles or buildings only when no infantry-priority target is legal in
  arc.
- Coax does not choose an in-arc infantry-priority intended target when an enemy Tank/building hard
  blocker would be the resolved shot victim first.
- Coax does not fire outside the 10-degree arc, outside 6-tile range, through smoke, through
  blocked LOS, through friendly hard blockers, at resources, at non-hostile entities, or at hidden
  targets.
- Coax cooldown and cannon cooldown are independent in both directions.
- Coax firing-reveal response delay is independent from cannon response delay in both directions.
- Coax does not rotate the turret, set desired weapon facing, change `target_id` used by the cannon,
  clear paths, request chase paths, or alter current movement intent.
- A Tank that fires cannon and coax in the same tick emits cannon attack feedback before coax attack
  feedback, and both attacks remain visible to eligible recipients.
- `tank_coax` client feedback uses MG sound, small flash/tracer/tail, and no cannon recoil.
- `tank_cannon` and missing/default Tank attack events keep cannon sound, large flash/tracer/tail,
  and cannon recoil.
- Stale targets, dead targets, non-finite facing, missing combat state, and dead Tanks are safe
  no-ops.

## Verification

- Focused Rust combat tests for coax damage, small-arms armor reduction, overpenetration,
  independent cooldown, arc gating, range gating, target priority, fallback targeting, and no turret
  rotation/pathing changes.
- Focused Rust regression tests for existing Tank cannon targeting, cooldown, stationary range
  ramp, moving-fire path retention, overpenetration, and firing reveal.
- Focused Rust tests for same-tick cannon/coax event order, post-main-pass facing snapshot behavior,
  intended-target hard-blocker rejection, and weapon-keyed firing-reveal response delay.
- Focused client contract tests for `tank_coax` audio, small flash/tracer/tail, no cannon recoil,
  same-tick cannon/coax feedback preservation, and missing/default Tank fallback behavior.
- `cargo test --manifest-path server/Cargo.toml -p rts-sim coax`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim tank_combat`
- `node tests/protocol_parity.mjs` if weapon ids or event fields are touched.
- `node tests/client_contracts/audio_contracts.mjs`
- `node tests/client_contracts/state_input_contracts.mjs`
- `node tests/client_contracts/renderer_feedback_contracts.mjs`
- `node tests/server_integration.mjs` with a running server, or `tests/run-all.sh --only-live-node`.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node scripts/check-client-architecture.mjs` if client files are touched.
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`

## Manual Test Focus

In a local dev scenario, point a Tank turret at a mixed infantry/vehicle/building group and confirm
the coax fires only through the turret arc while cannon behavior remains recognizable. Confirm coax
uses MG-scale feedback instead of cannon feedback, then check a moving Tank and a reloading/rotating
cannon case to confirm coax opportunity fire does not make the Tank chase, snap the turret, or drop
its path.

## Handoff Expectations

State the final coax weapon id, profile values, infantry-priority definition, cooldown behavior,
post-main-pass facing snapshot behavior, event ordering decision, minimum client feedback mapping,
temporary muzzle-origin limitation that Phase 8 must replace, verification commands, manual scenario
used, and factual patch-note bullets.

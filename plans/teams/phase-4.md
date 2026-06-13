# Phase 4 - Team-Safe Damage, Effects, and Notices

Status: planned.

## Goal

Make damage resolution and combat feedback respect team relationships. Allies should not be damaged,
credited, or treated as hostile through direct-fire, overpenetration, support-weapon, or damage
reaction paths under the initial no-friendly-fire rule.

## Scope

- Replace hostile checks in direct damage and damage attribution with the relationship API.
- Overpenetration must not damage allied entities behind an enemy.
- Mortar and artillery area damage must not damage allies unless a future explicit friendly-fire rule
  is added.
- Smoke, point-fire, mortar-fire, and other ability effect paths must classify allies separately from
  enemies where they interact with damage or attack feedback.
- Last-damage owner, last-damage position, kill credit, and score increments should record only enemy
  damage/kills.
- Worker retreat should react to enemy damage, not allied or non-hostile damage.
- Under-attack notices should go to the victim's team where appropriate, but this phase should not
  broaden fog/event visibility beyond the owner/team recipients explicitly tested here.
- Keep victory/game-over semantics unchanged until the next phase.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/hardening.md`
- `server/crates/sim/src/game/teams.rs`
- `server/crates/sim/src/game/mortar.rs`
- `server/crates/sim/src/game/artillery.rs`
- `server/crates/sim/src/game/services/combat/`
- `server/crates/sim/src/game/services/death.rs`
- `server/crates/sim/src/game/scoring.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `tests/team_integration.mjs`
- `tests/regression.mjs`

## Verification

```bash
cd server && cargo test team --workspace
cd server && cargo test combat --workspace
node tests/regression.mjs
node tests/team_integration.mjs
```

Required Rust scenarios:

- Overpenetration does not damage an allied entity behind an enemy.
- Mortar and artillery area damage do not damage allies under the selected team rules.
- Kill credit is not awarded for allied or non-hostile damage.
- Last-damage owner/position are not updated by allied or non-hostile damage.
- Worker retreat is triggered by enemy damage and not by allied/non-hostile damage.
- Under-attack notices are sent to the victim's team and not to unrelated players.

Required Node scenarios:

- A malicious client cannot use allied attack ids or support-fire commands to damage allied units.
- FFA damage and scoring behavior remains compatible.

## Acceptance Criteria

- Damage, score attribution, and damage reaction behavior use central relationship helpers.
- Initial team games have no friendly fire through normal combat and support-weapon paths.
- Remaining raw owner comparisons are documented as strict ownership, neutral/resource logic, or
  explicit follow-up work.

## Manual Testing Focus

None expected unless support-weapon behavior needs a one-off visual inspection. Prefer a scripted
dev scenario if visual inspection is needed.

## Handoff Requirements

The phase handoff must list every damage/effect surface audited, describe the friendly-fire rule
implemented, and call out any intentionally owner-only notices or feedback.

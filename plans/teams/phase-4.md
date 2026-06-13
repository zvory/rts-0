# Phase 4 - Team-Safe Damage, Effects, and Notices

Status: planned.

## Goal

Make damage resolution and combat feedback respect team relationships. Direct-fire, overpenetration,
damage attribution, and damage reaction paths should not treat allies as hostile, while mortar and
artillery area damage remain intentional friendly-fire surfaces: if a shell can damage the firing
player's own entities, it must also damage allied entities on the same team.

## Scope

- Replace hostile checks in direct damage and damage attribution with the relationship API.
- Overpenetration must not damage allied entities behind an enemy.
- Mortar and artillery area damage must use the same splash victim rules for allied entities that
  already apply to the firing player's own entities. Manual mortar fire, mortar autocast impact
  resolution, and artillery point-fire impacts should be able to damage same-team units/buildings
  inside the blast, including the firing player's own entities, instead of filtering them out as
  non-hostile.
- Mortar/autocast shot selection may still avoid unsafe shots according to its existing safety rule,
  but that prediction must consider same-team entities if it considers owned entities.
- Smoke, point-fire, mortar-fire, and other ability effect paths must classify allies separately from
  enemies where they interact with damage or attack feedback.
- Last-damage owner, last-damage position, kill credit, and score increments should record only enemy
  damage/kills. Friendly-fire splash may reduce health or kill allied/owned entities, but should not
  award enemy kill credit or combat score against that ally.
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
- Mortar area damage damages allied entities under the selected team rules, matching the existing
  self-damage behavior for owned entities.
- Artillery area damage damages allied entities under the selected team rules, matching the existing
  self-damage behavior for owned entities.
- Mortar/autocast friendly-fire prediction treats allied entities like owned entities when deciding
  whether a shot would hit a same-team entity.
- Kill credit is not awarded for allied or non-hostile damage.
- Last-damage owner/position are not updated by allied or non-hostile damage.
- Worker retreat is triggered by enemy damage and not by allied/non-hostile damage.
- Under-attack notices are sent to the victim's team and not to unrelated players.

Required Node scenarios:

- A malicious client cannot use allied direct attack ids to damage allied units.
- Support-fire commands consistently apply the explicit mortar/artillery friendly-fire rule to
  same-team entities in the blast radius.
- FFA damage and scoring behavior remains compatible.

## Acceptance Criteria

- Damage, score attribution, and damage reaction behavior use central relationship helpers.
- Initial team games have no friendly fire through normal direct combat paths.
- Mortar and artillery splash intentionally keep self-damage and team-damage behavior, with tests
  proving same-team entities are not filtered out of blast damage and are not scored as enemy kills.
- Remaining raw owner comparisons are documented as strict ownership, neutral/resource logic, or
  explicit follow-up work.

## Manual Testing Focus

None expected unless support-weapon behavior needs a one-off visual inspection. Prefer a scripted
dev scenario if visual inspection is needed.

## Handoff Requirements

The phase handoff must list every damage/effect surface audited, describe the friendly-fire rule
implemented, explicitly call out mortar/artillery team damage versus direct-fire ally safety, and
call out any intentionally owner-only notices or feedback.

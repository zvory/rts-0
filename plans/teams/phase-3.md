# Phase 3 - Team-Safe Simulation Combat and Victory

Status: planned.

## Goal

Make team relationships authoritative for hostility and match resolution. Allies should not attack
or damage each other through normal hostile-target logic, and a team should lose only when every
member is defeated.

## Scope

- Replace hostile checks in simulation services with the central relationship API.
- Raw `Attack` commands against allies must be ignored or rejected safely.
- Auto-acquisition, ordered attack retention, AT-team tank preference, moving fire retention, and
  building attacks must ignore allies.
- Overpenetration and support-weapon area damage must not damage allies unless a future explicit
  friendly-fire rule is added.
- Last-damage owner and kill credit should record only enemy damage.
- Worker retreat should react to enemy damage, not allied/non-hostile damage.
- Under-attack notices should go to the victim's team, not only the victim owner.
- Team victory should replace per-player victory in team games:
  - one-player FFA remains unchanged
  - one-player sandbox remains never-ending
  - a player losing all buildings should not receive a losing `gameOver` while any teammate keeps
    the team alive
  - final `gameOver` should include `winnerTeamId`
- Keep economies, build authority, production, and resources per-player.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/hardening.md`
- `server/crates/sim/src/game/teams.rs`
- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/services/world_query.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/combat/`
- `server/crates/sim/src/game/mortar.rs`
- `server/crates/sim/src/game/artillery.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `server/crates/sim/src/game/services/death.rs`
- `server/crates/sim/src/game/scoring.rs`
- `server/src/lobby/room_task.rs`
- `tests/team_integration.mjs`
- `tests/regression.mjs`

## Verification

Rust tests should cover the rules close to the simulation:

```bash
cd server && cargo test team --workspace
cd server && cargo test combat --workspace
node tests/regression.mjs
node tests/team_integration.mjs
```

Required Rust scenarios:

- Allied riflemen near each other do not auto-acquire.
- Raw attack command against an ally is ignored.
- Ordered attackers drop an allied target.
- Overpenetration does not damage an allied entity behind an enemy.
- Mortar and artillery area damage do not damage allies under the selected team rules.
- Kill credit is not awarded for allied damage.
- A 2v2 does not end when one player on a team loses all buildings.
- A 2v2 ends when all players on one team are defeated.

Required Node scenarios:

- Malicious client cannot attack allied entity ids.
- Defeated player on a living team does not receive early `gameOver`.
- Final team victory sends winning result to every surviving connected teammate.

## Acceptance Criteria

- No simulation hostile behavior depends on raw owner inequality.
- FFA remains compatible because every player has a singleton team.
- Team victory and defeat are authoritative in the room task.
- Regression tests cover malicious allied-target commands.

## Manual Testing Focus

Optional single-browser check of a scripted 2v2 AI setup only if automated team victory coverage is
ambiguous.

## Handoff Requirements

The phase handoff must list every hostile surface audited, call out any known remaining raw owner
comparisons that are intentionally own-control checks, and describe the team victory tests.

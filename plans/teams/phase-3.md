# Phase 3 - Team-Safe Command Targeting

Status: planned.

## Goal

Make authoritative command validation and target selection treat allies as non-hostile. This phase
is deliberately limited to choosing or retaining targets; damage, notices, fog, and victory remain
unchanged.

## Scope

- Replace hostile target checks in command validation and target acquisition with the central
  relationship API.
- Raw `Attack` commands against allies must be ignored or rejected safely.
- Auto-acquisition, attack-move acquisition, ordered attack retention, AT-team tank preference,
  moving-fire retention, and building attacks must ignore allies.
- Keep strict owner checks for command authority, production, gather/build/train/research/cancel,
  control surfaces, and economy.
- Add or update the raw-owner audit list so remaining `owner == player` or `owner != player` checks
  are classified as strict ownership, neutral/resource handling, or follow-up hostile surfaces.
- Do not change direct damage, overpenetration, mortar/artillery area effects, kill credit, worker
  retreat, under-attack notices, team victory, shared vision, or client selection behavior in this
  phase.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/hardening.md`
- `server/crates/sim/src/game/teams.rs`
- `server/crates/sim/src/game/services/world_query.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/combat/`
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

- Allied riflemen near each other do not auto-acquire.
- Raw attack command against an ally is ignored.
- Ordered attackers drop or refuse an allied target.
- Attack-move and building attack acquisition ignore allied entities.
- AT-team tank preference chooses enemy tanks and not allied tanks.

Required Node scenarios:

- Malicious client cannot assign allied entity ids as hostile attack targets.
- FFA command targeting remains behavior-compatible because every player has a singleton team.

## Acceptance Criteria

- No command validation or target acquisition hostile behavior depends on raw owner inequality.
- Strict own-control checks are documented and remain strict.
- Team setup tests can create allied seats, and allied targets are not accepted as hostile command
  targets.

## Manual Testing Focus

None expected unless a targeted test cannot cover a command path; prefer adding a scriptable
scenario first.

## Handoff Requirements

The phase handoff must list the command/targeting surfaces audited, name any raw owner comparisons
left for later damage/event phases, and identify the tests that prove allied targets are rejected.

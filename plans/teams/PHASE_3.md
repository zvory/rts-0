# Phase 3 - Shared Vision and Snapshot Projection

Goal: allies share current line of sight, explored history, and full allied entity details.

The server remains authoritative for current visibility. The client accumulates explored history
from all allied current-vision sources over time.

## Server Fog

Update `server/src/game/fog.rs`.

Current behavior stores one current-visibility grid per player and stamps only that player's own
entities. Replace this with team-aware recompute behavior.

Acceptable implementation:

- Build current visibility by team first, then copy or reference the team grid for each player.
- Or stamp each entity into every allied player's grid.

Required behavior:

- A player's grid includes sight from all living allied units and buildings.
- Neutral resource nodes still grant no vision.
- Players with no own entities but living allies still receive allied team vision.
- A fully defeated team has no vision.

Keep `Fog` current-only on the server. Do not add explored-history server state.

## Projection Rules

Update `server/src/rules/projection.rs`.

Entity visibility:

- Own entities are always visible.
- Allied entities are always visible and expose full details.
- Enemy entities are visible if the viewer's team can currently see them.
- Neutral resource nodes follow existing resource visibility policy, but current remaining deltas
  are shared through team vision.

Full allied details means:

- Production queue length is visible to allies.
- Production kind/progress is visible to allies.
- Build progress is visible to allies.
- Worker latched node is visible to allies.
- Combat `targetId` may be visible to allies under the same rules as own target ids.

Do not expose allied resources, tech, supply, or command authority.

## Event Delivery

Update combat/build/death event gates.

Required behavior:

- Allies receive attack/build/death events for allied entities.
- Enemy attack/death events are delivered if the viewer's team can see the event origin or relevant
  target position.
- Hidden enemy attacks remain hidden from the whole team.

Audit `target_id` tracers and death positions for fog leaks.

## Client Shared Explored History

Update `client/src/main.js` and `client/src/fog.js` usage.

Current `Match.ownEntities()` only returns `owner === playerId`. Replace this with a team-vision
source helper:

- Include own entities.
- Include allied entities.
- Exclude enemies and neutrals.

Because `Fog.update()` writes both `visibleGrid` and cumulative `exploredGrid`, this gives shared
explored history as soon as allied entities appear in snapshots.

## Files to Touch

- `docs/design/*.md`
- `server/src/game/fog.rs`
- `server/src/game/mod.rs`
- `server/src/rules/projection.rs`
- `server/src/game/services/combat.rs`
- `server/src/game/services/construction.rs`
- `server/src/game/services/death.rs`
- `client/src/main.js`
- `client/src/state.js`
- tests under `server/src/game/*`
- `tests/server_integration.mjs`

## Tests

Add Rust tests:

- Ally scout reveals an enemy to the viewer's snapshot.
- Enemy outside all allied sight is absent from every teammate snapshot.
- Allied production building snapshot includes full production queue details.
- A player with no own entities but a living ally still has nonempty team vision.
- Death/attack event delivery does not leak hidden enemy positions.

Add client/integration tests:

- A client marks tiles explored from allied entities.
- Explored tiles remain dim after allied units move away.

Run:

```bash
cd server && cargo test
node tests/server_integration.mjs
node tests/client_contracts.mjs
```

## Acceptance Criteria

- Shared current line of sight works authoritatively on the server.
- Shared explored history works visually on the client.
- Allied entity snapshots expose full details.
- Hidden enemies remain hidden unless any teammate sees them.

import { assert } from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { buildCommandCardDescriptors } from "../../client/src/hud_command_card.js";
import { factionCatalog, statsForFaction } from "../../client/src/config.js";
import { KIND, LAB_ROLE, ABILITY } from "../../client/src/protocol.js";

const state = new GameState({
  playerId: 1, spectator: true, map: { width: 64, height: 64, tileSize: 32, terrain: [] },
  players: [1, 2].map((id) => ({ id, factionId: "kriegsia" })),
});
const policy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
const snapshot = (factionId, tick) => state.applySnapshot({
  tick, entities: [], events: [], playerFactions: [{ id: 2, factionId }],
  playerResources: [{ id: 1, steel: 1000, oil: 1000, upgrades: [] }],
}, tick);
snapshot("cultivators", 1);
assert(state.playerById(2).factionId === "cultivators", "snapshot updates P2 faction identity");
assert(state.playerById(1).factionId === "kriegsia", "snapshot preserves other seats");
const context = (entity) => ({
  spectator: true, commandSurfaceEnabled: true, state, controlPolicy: policy,
  commandOwner: 2, playerId: 2, factionId: policy.commandFactionId(state, 2),
  selection: [entity], entities: [entity], resources: policy.commandResources(state, 2),
  upgrades: [], groupCooldownClocks: () => [], playerHasCompleteKind: () => true,
});
const novice = { id: 20, owner: 2, kind: KIND.WARRIOR, insight: 60 };
const card = buildCommandCardDescriptors(context(novice));
for (const ability of [ABILITY.EVOLVE_WIND, ABILITY.EVOLVE_WAR, ABILITY.EVOLVE_MAGIC]) {
  assert(card.abilityAffordances.some((row) => row.definition.ability === ability && row.readyIds.includes(20)),
    `P2 full-insight Novice can choose ${ability}`);
}
// Derive expectations from the faction catalog so new research stays covered.
for (const [kind, upgrades] of Object.entries(factionCatalog("cultivators").research)) {
  const card = buildCommandCardDescriptors(context({ id: 21, owner: 2, kind, buildProgress: null }));
  for (const upgrade of upgrades) {
    assert(card.slots.some((slot) => slot?.commandId?.endsWith(`.research.${upgrade}`)),
      `P2 ${kind} exposes catalog research ${upgrade}`);
  }
}
assert(statsForFaction(KIND.RESOURCE_DEPOT, state.playerById(2).factionId).label === "Nexus", "P2 depot has Nexus identity");
snapshot("kriegsia", 0);
assert(policy.commandFactionId(state, 2) === "kriegsia", "rewound snapshot restores faction identity");

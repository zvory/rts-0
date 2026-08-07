import { autoBuild } from "../../client/src/state_auto_build.js";
import { productionGridEntries, productionRepeatCommand } from "../../client/src/tab_menu.js";
import { DEFAULT_FACTION_ID, KIND } from "../../client/src/protocol.js";
import { assert, assertDeepEqual } from "./assertions.mjs";

assertDeepEqual(
  autoBuild(null),
  { paused: false, reserveSteel: 0, reserveOil: 0, ack: 0 },
  "Auto-Build client fallback starts running with zero Steel and Oil floors",
);

const entities = [
  { id: 10, owner: 1, kind: KIND.BARRACKS, prodRepeatKinds: [KIND.RIFLEMAN] },
  { id: 11, owner: 1, kind: KIND.BARRACKS, buildProgress: 0.5, prodRepeatKinds: [] },
  { id: 20, owner: 1, kind: KIND.STEELWORKS, prodRepeatKinds: [KIND.ARTILLERY] },
  { id: 30, owner: 1, kind: KIND.FACTORY, prodRepeatKinds: [KIND.TANK] },
  { id: 99, owner: 2, kind: KIND.BARRACKS, prodRepeatKinds: [KIND.RIFLEMAN] },
];
const state = {
  playerId: 1,
  localFactionId: DEFAULT_FACTION_ID,
  entitiesInterpolated: () => entities,
};
const gridEntries = productionGridEntries(state, {
  getActiveProfile: () => ({ mode: "grid" }),
});
assert(gridEntries.length === 9, "global Auto-Build grid exposes all nine Barracks, Gunworks, and Vehicle Works units");
assertDeepEqual(
  gridEntries.map((entry) => entry.hotkey),
  ["Q", "W", "E", "A", "S", "D", "Z", "X", "C"],
  "global Auto-Build grid uses the complete command-card keyboard grid",
);
const rifleman = gridEntries.find((entry) => entry.unit === KIND.RIFLEMAN);
assertDeepEqual(rifleman.producerIds, [10, 11], "global Auto-Build includes every owned compatible producer, including unfinished buildings");
assert(rifleman.activeCount === 1, "global Auto-Build reports the authoritative active/compatible producer count");
assertDeepEqual(
  productionRepeatCommand(state, KIND.RIFLEMAN, 1),
  { c: "adjustProductionRepeat", buildings: [10, 11], unit: KIND.RIFLEMAN, delta: 1 },
  "global Auto-Build adds across every owned compatible producer",
);
assertDeepEqual(
  productionRepeatCommand(state, KIND.RIFLEMAN, -1),
  { c: "adjustProductionRepeat", buildings: [10, 11], unit: KIND.RIFLEMAN, delta: -1 },
  "Shift global Auto-Build removes across every owned compatible producer",
);

const conflictingClassicEntries = productionGridEntries(state, {
  getActiveProfile: () => ({ mode: "direct" }),
  hotkeyCodeForCommand: () => "KeyR",
});
assertDeepEqual(
  conflictingClassicEntries.map((entry) => entry.hotkey),
  ["Q", "W", "E", "A", "S", "D", "Z", "X", "C"],
  "conflicting classic bindings fall back to the deterministic grid",
);

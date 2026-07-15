// Shared playable spawn catalog for the human Lab panel and local agent bridge.
// Keep this independent of DOM and transport code so both callers expose the same
// server-validated kinds without scraping rendered controls.

import { PLAYABLE_FACTIONS } from "./lobby_view.js";
import { DEFAULT_FACTION_ID, KIND } from "./protocol.js";
import { factionCatalog, STATS } from "./config.js";

const LAB_ONLY_UNIT_SPAWNS_BY_FACTION = Object.freeze({
  [DEFAULT_FACTION_ID]: Object.freeze([KIND.SCOUT_PLANE]),
});

export function labSpawnFactionOptions() {
  return PLAYABLE_FACTIONS.filter((entry) => labSpawnUnitKindsForFaction(entry.id).length > 0);
}

export function labSpawnUnitKindsForFaction(factionId) {
  const catalogUnits = factionCatalog(factionId).units;
  const labOnlyUnits = LAB_ONLY_UNIT_SPAWNS_BY_FACTION[factionId] || [];
  return [...catalogUnits, ...labOnlyUnits].filter((kind, index, units) =>
    STATS[kind] && units.indexOf(kind) === index,
  );
}

export function labBuildingSpawnFactionOptions() {
  return PLAYABLE_FACTIONS.filter((entry) => labSpawnBuildingKindsForFaction(entry.id).length > 0);
}

export function labSpawnBuildingKindsForFaction(factionId) {
  return factionCatalog(factionId).buildings.filter((kind) => STATS[kind]);
}

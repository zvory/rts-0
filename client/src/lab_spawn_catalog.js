// Shared playable spawn catalog for the human Lab panel and local agent bridge.
// Keep this independent of DOM and transport code so both callers expose the same
// server-validated kinds without scraping rendered controls.

import { PLAYABLE_FACTIONS } from "./lobby_view.js";
import { KIND, daoForKind } from "./protocol.js";
import { factionCatalog, STATS, statsForFaction } from "./config.js";

const ABILITY_ONLY_UNIT_KINDS = Object.freeze(new Set([KIND.SCOUT_PLANE]));

export function labSpawnFactionOptions() {
  return PLAYABLE_FACTIONS.filter((entry) => labSpawnUnitKindsForFaction(entry.id).length > 0);
}

export function labSpawnUnitKindsForFaction(factionId) {
  return factionCatalog(factionId).units.filter((kind) =>
    STATS[kind] && !ABILITY_ONLY_UNIT_KINDS.has(kind)
  );
}

export function labBuildingSpawnFactionOptions() {
  return PLAYABLE_FACTIONS.filter((entry) => labSpawnBuildingKindsForFaction(entry.id).length > 0);
}

export function labSpawnBuildingKindsForFaction(factionId) {
  return factionCatalog(factionId).buildings.filter((kind) => STATS[kind]);
}

export function labSpawnUnitOptionsForFaction(factionId) {
  const unitKinds = labSpawnUnitKindsForFaction(factionId);
  const baseOptions = unitKinds.map((kind) => ({
    kind, dao: null, label: statsForFaction(kind, factionId).label,
  }));
  const masterOptions = unitKinds.filter((kind) => daoForKind(kind))
    .map((kind) => ({
      kind, dao: null, daoMaster: true, label: statsForFaction(kind, factionId, null, true).label,
    }));
  return [...baseOptions, ...masterOptions];
}

import { ABILITY, DEFAULT_FACTION_ID, KIND, UPGRADE } from "../protocol.js";
import {
  ABILITIES,
  EKAT_FACTION_ID,
  FIXTURE_FACTION_ID,
  WORKER_BUILDABLE,
} from "./rules_mirror.js";

function freezeCatalog(catalog) {
  const trainables = {};
  for (const [building, units] of Object.entries(catalog.trainables || {})) {
    trainables[building] = Object.freeze([...units]);
  }
  const research = {};
  for (const [building, upgrades] of Object.entries(catalog.research || {})) {
    research[building] = Object.freeze([...upgrades]);
  }
  return Object.freeze({
    ...catalog,
    units: Object.freeze([...(catalog.units || [])]),
    buildings: Object.freeze([...(catalog.buildings || [])]),
    buildables: Object.freeze([...(catalog.buildables || [])]),
    trainables: Object.freeze(trainables),
    research: Object.freeze(research),
    abilities: Object.freeze([...(catalog.abilities || [])]),
  });
}

export const FACTION_CATALOGS = Object.freeze({
  [DEFAULT_FACTION_ID]: freezeCatalog({
    id: DEFAULT_FACTION_ID,
    loadoutId: "kriegsia.standard",
    units: [
      KIND.WORKER,
      KIND.RIFLEMAN,
      KIND.MACHINE_GUNNER,
      KIND.PANZERFAUST,
      KIND.ANTI_TANK_GUN,
      KIND.MORTAR_TEAM,
      KIND.ARTILLERY,
      KIND.TANK,
      KIND.SCOUT_CAR,
      KIND.SCOUT_PLANE,
      KIND.COMMAND_CAR,
    ],
    buildings: [
      KIND.CITY_CENTRE,
      KIND.DEPOT,
      KIND.BARRACKS,
      KIND.TRAINING_CENTRE,
      KIND.FACTORY,
      KIND.RESEARCH_COMPLEX,
      KIND.STEELWORKS,
      KIND.TANK_TRAP,
      KIND.PUMP_JACK,
    ],
    buildables: WORKER_BUILDABLE,
    trainables: {
      [KIND.CITY_CENTRE]: [KIND.WORKER, KIND.SCOUT_PLANE],
      [KIND.BARRACKS]: [KIND.RIFLEMAN, KIND.MACHINE_GUNNER, KIND.PANZERFAUST],
      [KIND.FACTORY]: [KIND.SCOUT_CAR, KIND.TANK, KIND.COMMAND_CAR],
      [KIND.STEELWORKS]: [KIND.MORTAR_TEAM, KIND.ANTI_TANK_GUN, KIND.ARTILLERY],
    },
    research: {
      [KIND.TRAINING_CENTRE]: [UPGRADE.METHAMPHETAMINES, UPGRADE.ENTRENCHMENT],
      [KIND.RESEARCH_COMPLEX]: [
        UPGRADE.ANTI_TANK_GUN_UNLOCK,
        UPGRADE.BALLISTIC_TABLES,
        UPGRADE.TANK_UNLOCK,
        UPGRADE.COMMAND_CAR_UNLOCK,
        UPGRADE.MORTAR_AUTOCAST,
        UPGRADE.SMOKE_PLUS,
        UPGRADE.ARTILLERY_UNLOCK,
      ],
    },
    abilities: [
      ABILITY.SMOKE,
      ABILITY.MORTAR_FIRE,
      ABILITY.POINT_FIRE,
      ABILITY.BLANKET_FIRE,
      ABILITY.BREAKTHROUGH,
    ],
  }),
  [FIXTURE_FACTION_ID]: freezeCatalog({
    id: FIXTURE_FACTION_ID,
    loadoutId: "phase2_empty_fixture.scout_depot",
    units: [KIND.SCOUT_CAR],
    buildings: [KIND.DEPOT],
    buildables: [],
    trainables: {},
    research: {},
    abilities: [],
  }),
  [EKAT_FACTION_ID]: freezeCatalog({
    id: EKAT_FACTION_ID,
    loadoutId: "ekat.standard",
    units: [KIND.EKAT, KIND.GOLEM],
    buildings: [KIND.ZAMOK],
    buildables: [],
    trainables: {
      [KIND.ZAMOK]: [KIND.GOLEM],
    },
    research: {},
    abilities: [
      ABILITY.EKAT_TELEPORT,
      ABILITY.EKAT_LINE_SHOT,
      ABILITY.EKAT_MAGIC_ANCHOR,
      ABILITY.EKAT_CONSUME_GOLEM,
    ],
  }),
});

const EMPTY_CLIENT_CATALOG = freezeCatalog({
  id: "unknown",
  loadoutId: "",
  units: [],
  buildings: [],
  buildables: [],
  trainables: {},
  research: {},
  abilities: [],
});

export function factionCatalog(factionId = DEFAULT_FACTION_ID) {
  return FACTION_CATALOGS[factionId] || EMPTY_CLIENT_CATALOG;
}

export function workerBuildablesForFaction(factionId) {
  return factionCatalog(factionId).buildables;
}

export function trainableUnitsForFaction(factionId, buildingKind) {
  return factionCatalog(factionId).trainables[buildingKind] || [];
}

export function researchableUpgradesForFaction(factionId, buildingKind) {
  return factionCatalog(factionId).research[buildingKind] || [];
}

export function commandCardAbilitiesForFaction(factionId) {
  return factionCatalog(factionId)
    .abilities
    .map((ability) => ABILITIES[ability])
    .filter((entry) => entry && entry.commandCard !== false);
}

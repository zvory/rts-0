import {
  ABILITY_BY_CODE,
  ABILITY_OBJECT_KIND_BY_CODE,
  COMPACT_SNAPSHOT_VERSION,
  KIND_BY_CODE,
  MAX_COMPACT_ABILITIES,
  MAX_COMPACT_ABILITY_OBJECTS,
  MAX_COMPACT_BUILDING_FOOTPRINT,
  MAX_COMPACT_DEBUG_WAYPOINTS,
  MAX_COMPACT_ENTITIES,
  MAX_COMPACT_EVENTS,
  MAX_COMPACT_ORDER_PLAN,
  MAX_COMPACT_REMEMBERED_BUILDINGS,
  MAX_COMPACT_RESOURCE_DELTAS,
  MAX_COMPACT_SMOKES,
  MAX_COMPACT_VISIBLE_TILES,
  ORDER_STAGE_BY_CODE,
  S,
  SETUP_BY_CODE,
  STATE_BY_CODE,
  UPGRADE_BY_CODE,
} from "./protocol_constants.js";
import { decodeCompactEvent } from "./protocol_snapshot_events.js";
import { decodeCompactTrenches } from "./protocol_snapshot_trenches.js";

export function decodeCompactSnapshot(raw) {
  if (raw.v !== COMPACT_SNAPSHOT_VERSION) {
    throw new Error(`unsupported compact snapshot version: ${raw.v}`);
  }

  const scalars = readArray(raw.s, "snapshot scalars", 5);
  if (scalars.length !== 5) throw new Error("compact snapshot scalar count mismatch");

  return {
    t: S.SNAPSHOT,
    tick: readU32(scalars[0], "tick"),
    worldCombatPosition: raw.wc == null
      ? null
      : decodeCompactPoint(raw.wc, "worldCombatPosition"),
    steel: readU32(scalars[1], "steel"),
    oil: readU32(scalars[2], "oil"),
    supplyUsed: readU32(scalars[3], "supplyUsed"),
    supplyCap: readU32(scalars[4], "supplyCap"),
    entities: readArray(raw.e, "entities", MAX_COMPACT_ENTITIES).map(decodeCompactEntity),
    resourceDeltas: readOptionalArray(
      raw.r,
      "resourceDeltas",
      MAX_COMPACT_RESOURCE_DELTAS,
    ).map(decodeCompactResourceDelta),
    smokes: readOptionalArray(raw.sm, "smokes", MAX_COMPACT_SMOKES).map(decodeCompactSmoke),
    abilityObjects: readOptionalArray(
      raw.ao,
      "abilityObjects",
      MAX_COMPACT_ABILITY_OBJECTS,
    ).map(decodeCompactAbilityObject),
    trenches: decodeCompactTrenches(raw.tr),
    visibleTiles: decodeVisibilityRuns(raw.fg),
    rememberedBuildings: readOptionalArray(
      raw.mb,
      "rememberedBuildings",
      MAX_COMPACT_REMEMBERED_BUILDINGS,
    ).map(decodeCompactRememberedBuilding),
    events: readOptionalArray(raw.ev, "events", MAX_COMPACT_EVENTS).map(decodeCompactEvent),
    playerResources: readOptionalArray(raw.pr, "playerResources", 32).map(
      decodeCompactPlayerResource,
    ),
    upgrades: readOptionalArray(raw.u, "upgrades", 32).map((code, index) =>
      readCode(code, UPGRADE_BY_CODE, `upgrade.${index}`),
    ),
    netStatus: decodeCompactNetStatus(raw.n),
  };
}

function decodeVisibilityRuns(record) {
  if (record == null) return [];
  const runs = readArray(record, "visibleTiles", MAX_COMPACT_VISIBLE_TILES + 1);
  if (runs.length < 2) throw new Error("visibleTiles run data must include a value and length");
  let value = readU32(runs[0], "visibleTiles.first");
  if (value !== 0 && value !== 1) throw new Error("visibleTiles.first must be 0 or 1");
  const out = [];
  for (let i = 1; i < runs.length; i++) {
    const len = readU32(runs[i], `visibleTiles.run.${i}`);
    if (len === 0) throw new Error("visibleTiles run length must be positive");
    if (out.length + len > MAX_COMPACT_VISIBLE_TILES) {
      throw new Error("visibleTiles exceeds compact bounds");
    }
    for (let j = 0; j < len; j++) out.push(value);
    value = value === 1 ? 0 : 1;
  }
  return out;
}

function decodeCompactRememberedBuilding(record, index) {
  const fields = readArray(record, `remembered building ${index}`, 7);
  if (fields.length !== 7) throw new Error(`remembered building ${index} field count mismatch`);
  return {
    id: readU32(fields[0], "rememberedBuilding.id"),
    owner: readU32(fields[1], "rememberedBuilding.owner"),
    kind: readCode(fields[2], KIND_BY_CODE, "rememberedBuilding.kind"),
    x: readNumber(fields[3], "rememberedBuilding.x"),
    y: readNumber(fields[4], "rememberedBuilding.y"),
    footprint: readArray(
      fields[5],
      "rememberedBuilding.footprint",
      MAX_COMPACT_BUILDING_FOOTPRINT,
    ).map((tile, tileIndex) => {
      const pair = readArray(tile, `rememberedBuilding.footprint.${tileIndex}`, 2);
      if (pair.length !== 2) {
        throw new Error(`rememberedBuilding.footprint.${tileIndex} field count mismatch`);
      }
      return [
        readU32(pair[0], `rememberedBuilding.footprint.${tileIndex}.x`),
        readU32(pair[1], `rememberedBuilding.footprint.${tileIndex}.y`),
      ];
    }),
    observedTick: readU32(fields[6], "rememberedBuilding.observedTick"),
  };
}

function decodeCompactSmoke(record, index) {
  const fields = readArray(record, `smoke ${index}`, 5);
  if (fields.length !== 5) throw new Error(`smoke ${index} field count mismatch`);
  return {
    id: readU32(fields[0], "smoke.id"),
    x: readNumber(fields[1], "smoke.x"),
    y: readNumber(fields[2], "smoke.y"),
    radiusTiles: readNumber(fields[3], "smoke.radiusTiles"),
    expiresIn: readU32(fields[4], "smoke.expiresIn"),
  };
}

function decodeCompactAbilityObject(record, index) {
  const fields = readArray(record, `ability object ${index}`, 9);
  if (fields.length !== 9) throw new Error(`ability object ${index} field count mismatch`);
  const object = {
    id: readU32(fields[0], "abilityObject.id"),
    owner: readU32(fields[1], "abilityObject.owner"),
    ability: readCode(fields[2], ABILITY_BY_CODE, "abilityObject.ability"),
    kind: readCode(fields[3], ABILITY_OBJECT_KIND_BY_CODE, "abilityObject.kind"),
    x: readNumber(fields[4], "abilityObject.x"),
    y: readNumber(fields[5], "abilityObject.y"),
  };
  if (fields[6] != null) object.expiresIn = readU32(fields[6], "abilityObject.expiresIn");
  if (fields[7] != null) {
    object.sourceCasterId = readU32(fields[7], "abilityObject.sourceCasterId");
  }
  if (fields[8] != null) {
    object.ownerState = decodeCompactAbilityObjectOwnerState(fields[8]);
  }
  return object;
}

function decodeCompactAbilityObjectOwnerState(record) {
  const fields = readArray(record, "abilityObject.ownerState", 6);
  if (fields.length !== 6) throw new Error("abilityObject.ownerState field count mismatch");
  const state = {};
  if (fields[0] != null) {
    state.earliestReturnTick = readU32(fields[0], "abilityObject.ownerState.earliestReturnTick");
  }
  if (fields[1] != null) state.hp = readU32(fields[1], "abilityObject.ownerState.hp");
  if (fields[2] != null) state.radius = readNumber(fields[2], "abilityObject.ownerState.radius");
  if (fields[3] != null) {
    state.destroyedLockoutTicks = readU32(
      fields[3],
      "abilityObject.ownerState.destroyedLockoutTicks",
    );
  }
  if (fields[4] != null) {
    state.distanceTraveled = readNumber(fields[4], "abilityObject.ownerState.distanceTraveled");
  }
  if (fields[5] != null) state.ticksOut = readU32(fields[5], "abilityObject.ownerState.ticksOut");
  return state;
}

function decodeCompactNetStatus(record) {
  const fields = readArray(record, "netStatus", 8);
  if (fields.length !== 5 && fields.length !== 8) throw new Error("netStatus field count mismatch");
  const flags = readU32(fields[2], "netStatus.flags");
  const status = {
    serverLagMs: readU32(fields[0], "netStatus.serverLagMs"),
    tickMs: readU32(fields[1], "netStatus.tickMs"),
    slowTick: !!(flags & 1),
    slowTickCount: readU32(fields[3], "netStatus.slowTickCount"),
    headOfLine: !!(flags & 2),
    headOfLineCount: readU32(fields[4], "netStatus.headOfLineCount"),
  };
  if (fields.length === 8) {
    status.predictionVersion = readU32(fields[5], "netStatus.predictionVersion");
    status.lastSimConsumedClientSeq = readU32(fields[6], "netStatus.lastSimConsumedClientSeq");
    status.lastSimConsumedClientTick =
      fields[7] == null ? null : readU32(fields[7], "netStatus.lastSimConsumedClientTick");
  }
  return status;
}

function decodeCompactPlayerResource(record, index) {
  const fields = readArray(record, `playerResource ${index}`, 5);
  if (fields.length < 5) throw new Error(`playerResource ${index} is too short`);
  return {
    id: readU32(fields[0], "playerResource.id"),
    steel: readU32(fields[1], "playerResource.steel"),
    oil: readU32(fields[2], "playerResource.oil"),
    supplyUsed: readU32(fields[3], "playerResource.supplyUsed"),
    supplyCap: readU32(fields[4], "playerResource.supplyCap"),
  };
}

function decodeCompactEntity(record, index) {
  const fields = readArray(record, `entity ${index}`, 37);
  if (fields.length < 8) throw new Error(`entity ${index} is too short`);
  const entity = {
    id: readU32(fields[0], "entity.id"),
    owner: readU32(fields[1], "entity.owner"),
    kind: readCode(fields[2], KIND_BY_CODE, "entity.kind"),
    x: readNumber(fields[3], "entity.x"),
    y: readNumber(fields[4], "entity.y"),
    hp: readU32(fields[5], "entity.hp"),
    maxHp: readU32(fields[6], "entity.maxHp"),
    state: readCode(fields[7], STATE_BY_CODE, "entity.state"),
  };

  assignOptional(entity, "facing", fields, 8, readNumber);
  assignOptional(entity, "weaponFacing", fields, 9, readNumber);
  assignOptionalCode(entity, "prodKind", fields, 10, KIND_BY_CODE);
  assignOptional(entity, "prodProgress", fields, 11, readNumber);
  assignOptional(entity, "prodQueue", fields, 12, readU32);
  assignOptional(entity, "buildProgress", fields, 13, readNumber);
  assignOptional(entity, "latchedNode", fields, 14, readU32);
  assignOptional(entity, "targetId", fields, 15, readU32);
  assignOptionalCode(entity, "setupState", fields, 16, SETUP_BY_CODE);
  assignOptional(entity, "remaining", fields, 17, readU32);
  assignRally(entity, fields, 18);
  assignOptional(entity, "oilUsed", fields, 19, readNumber);
  assignOptional(entity, "setupFacing", fields, 20, readNumber);
  assignOrderPlan(entity, fields, 21);
  assignOptional(entity, "chargeCooldownLeft", fields, 22, readU32);
  assignAbilities(entity, fields, 23);
  assignOptional(entity, "breakthroughTicks", fields, 24, readU32);
  assignOptional(entity, "visionOnly", fields, 25, readBool);
  assignDebugPath(entity, fields, 26);
  assignRallyPlan(entity, fields, 27);
  assignOptionalCode(entity, "prodUpgrade", fields, 28, UPGRADE_BY_CODE);
  assignOptional(entity, "buildActive", fields, 29, readBool);
  assignOptional(entity, "deconstructProgress", fields, 30, readNumber);
  assignOptional(entity, "weaponRangeTiles", fields, 31, readNumber);
  assignOptional(entity, "occupiedTrenchId", fields, 32, readU32);
  assignScoutPlane(entity, fields, 33);
  assignOptional(entity, "prodScoutPlaneQueued", fields, 34, readBool);
  assignOptional(entity, "panzerfaustLoaded", fields, 35, readBool);
  assignOptionalCodeList(entity, "prodRepeatKinds", fields, 36, KIND_BY_CODE);
  return entity;
}

function assignOptionalCodeList(target, key, fields, index, codeMap) {
  if (index >= fields.length || fields[index] == null) return;
  target[key] = readArray(fields[index], `entity.${key}`).map((value, listIndex) =>
    readCode(value, codeMap, `entity.${key}[${listIndex}]`));
}

/** Decode the optional rally-point slot ([x, y] world px, owner-only) into `entity.rally`. */
function assignRally(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const pair = readArray(fields[index], "entity.rally", 2);
  if (pair.length !== 2) throw new Error("entity.rally must have two elements");
  target.rally = [readNumber(pair[0], "entity.rally.x"), readNumber(pair[1], "entity.rally.y")];
}

/** Decode owner-only current + queued order stages into `entity.orderPlan`. */
function assignOrderPlan(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const markers = readArray(fields[index], "entity.orderPlan", MAX_COMPACT_ORDER_PLAN);
  target.orderPlan = markers.map((record, markerIndex) =>
    readOrderPlanMarker(record, `entity.orderPlan.${markerIndex}`),
  );
}

/** Decode owner-only building rally stages into `entity.rallyPlan`. */
function assignRallyPlan(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const markers = readArray(fields[index], "entity.rallyPlan", 4);
  target.rallyPlan = markers.map((record, markerIndex) =>
    readOrderPlanMarker(record, `entity.rallyPlan.${markerIndex}`),
  );
}

/** Decode owner-only Scout Plane orbit telemetry into `entity.scoutPlane`. */
function assignScoutPlane(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const record = readArray(fields[index], "entity.scoutPlane", 1);
  if (record.length !== 1) throw new Error("entity.scoutPlane field count mismatch");
  target.scoutPlane = {};
  if (record[0] != null) {
    target.scoutPlane.orbitCenter = decodeCompactPoint(record[0], "entity.scoutPlane.orbitCenter");
  }
}

function readOrderPlanMarker(record, label) {
  const marker = readArray(record, label, 3);
  if (marker.length !== 3) {
    throw new Error(`${label} field count mismatch`);
  }
  return {
    kind: readCode(marker[0], ORDER_STAGE_BY_CODE, `${label}.kind`),
    x: readNumber(marker[1], `${label}.x`),
    y: readNumber(marker[2], `${label}.y`),
  };
}

/** Decode owner-only ability cooldown affordances into `entity.abilities`. */
function assignAbilities(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const cooldowns = readArray(fields[index], "entity.abilities", MAX_COMPACT_ABILITIES);
  target.abilities = cooldowns.map((record, abilityIndex) =>
    readAbilityCooldown(record, `entity.abilities.${abilityIndex}`),
  );
}

function readAbilityCooldown(record, label) {
  const fields = readArray(record, label, 8);
  if (fields.length < 2 || fields.length > 8) throw new Error(`${label} field count mismatch`);
  const ability = {
    ability: readCode(fields[0], ABILITY_BY_CODE, `${label}.ability`),
    cooldownLeft: readU32(fields[1], `${label}.cooldownLeft`),
  };
  if (fields.length > 2 && fields[2] != null) {
    ability.remainingUses = readU32(fields[2], `${label}.remainingUses`);
  }
  if (fields.length > 3 && fields[3] != null) {
    ability.autocastEnabled = readBool(fields[3], `${label}.autocastEnabled`);
  }
  if (fields.length > 4 && fields[4] != null) {
    ability.activeObjectId = readU32(fields[4], `${label}.activeObjectId`);
  }
  if (fields.length > 5 && fields[5] != null) {
    ability.availableTick = readU32(fields[5], `${label}.availableTick`);
  }
  if (fields.length > 6 && fields[6] != null) {
    ability.lockoutUntilTick = readU32(fields[6], `${label}.lockoutUntilTick`);
  }
  if (fields.length > 7 && fields[7] != null) {
    ability.expiresIn = readU32(fields[7], `${label}.expiresIn`);
  }
  return ability;
}

/** Decode projection-policy movement path diagnostics. */
function assignDebugPath(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const record = readArray(fields[index], "entity.debugPath", 6);
  if (record.length !== 6) throw new Error("entity.debugPath field count mismatch");
  target.debugPath = {
    waypoints: readArray(record[0], "entity.debugPath.waypoints", MAX_COMPACT_DEBUG_WAYPOINTS).map(
      (point, pointIndex) => decodeCompactDebugPoint(point, `entity.debugPath.waypoints.${pointIndex}`),
    ),
    goal: record[1] == null ? null : decodeCompactDebugPoint(record[1], "entity.debugPath.goal"),
    lastRepathTick: readU32(record[2], "entity.debugPath.lastRepathTick"),
    stuckTicks: readU32(record[3], "entity.debugPath.stuckTicks"),
    staticBlockedTicks: readU32(record[4], "entity.debugPath.staticBlockedTicks"),
    totalWaypoints: readU32(record[5], "entity.debugPath.totalWaypoints"),
  };
}

function decodeCompactDebugPoint(record, label) {
  const [x, y] = decodeCompactPoint(record, label);
  return { x, y };
}

function decodeCompactResourceDelta(record, index) {
  const fields = readArray(record, `resource delta ${index}`, 2);
  if (fields.length !== 2) throw new Error(`resource delta ${index} field count mismatch`);
  return {
    id: readU32(fields[0], "resourceDelta.id"),
    remaining: readU32(fields[1], "resourceDelta.remaining"),
  };
}

function decodeCompactPoint(record, label) {
  const pair = readArray(record, label, 2);
  if (pair.length !== 2) throw new Error(`${label} must have two elements`);
  return [readNumber(pair[0], `${label}.x`), readNumber(pair[1], `${label}.y`)];
}

function assignOptional(target, field, fields, index, reader) {
  if (index >= fields.length || fields[index] == null) return;
  target[field] = reader(fields[index], `entity.${field}`);
}

function assignOptionalCode(target, field, fields, index, table) {
  if (index >= fields.length || fields[index] == null) return;
  target[field] = readCode(fields[index], table, `entity.${field}`);
}

function readOptionalArray(value, name, maxLength) {
  if (value == null) return [];
  return readArray(value, name, maxLength);
}

function readArray(value, name, maxLength) {
  if (!Array.isArray(value)) throw new Error(`${name} must be an array`);
  if (value.length > maxLength) throw new Error(`${name} exceeds max length ${maxLength}`);
  return value;
}

function readNumber(value, name) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${name} must be a finite number`);
  }
  return value;
}

function readU32(value, name) {
  const number = readNumber(value, name);
  if (!Number.isInteger(number) || number < 0 || number > 0xffffffff) {
    throw new Error(`${name} must be a u32`);
  }
  return number;
}

function readBool(value, name) {
  if (typeof value !== "boolean") throw new Error(`${name} must be a boolean`);
  return value;
}

function readCode(value, table, name) {
  const code = readU32(value, name);
  if (!Object.prototype.hasOwnProperty.call(table, code)) {
    throw new Error(`${name} has unknown code ${code}`);
  }
  return table[code];
}

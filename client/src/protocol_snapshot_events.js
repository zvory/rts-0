import {
  EVENT,
  EVENT_BY_CODE,
  KIND_BY_CODE,
  NOTICE_SEVERITY,
  NOTICE_SEVERITY_BY_CODE,
  SETUP_BY_CODE,
  WEAPON_KIND_BY_CODE,
} from "./protocol_constants.js";

export function decodeCompactEvent(record, index) {
  const fields = readArray(record, `event ${index}`, 6);
  if (fields.length < 1) throw new Error(`event ${index} is too short`);
  const eventKind = readCode(fields[0], EVENT_BY_CODE, "event.kind");
  switch (eventKind) {
    case EVENT.ATTACK:
      if (fields.length < 3 || fields.length > 6) {
        throw new Error(`attack event ${index} field count mismatch`);
      }
      {
        const ev = {
          e: EVENT.ATTACK,
          from: readU32(fields[1], "event.from"),
          to: readU32(fields[2], "event.to"),
        };
        if (fields.length > 3 && fields[3] != null) {
          ev.reveal = decodeCompactAttackReveal(fields[3], index);
        }
        if (fields.length > 4 && fields[4] != null) {
          ev.toPos = decodeCompactPoint(fields[4], "event.toPos");
        }
        if (fields.length > 5) {
          const weaponKind = readOptionalWeaponKind(fields[5], "event.weaponKind");
          if (weaponKind) ev.weaponKind = weaponKind;
        }
        return ev;
      }
    case EVENT.OVERPENETRATION:
      requireLength(fields, 2, `overpenetration event ${index}`);
      return {
        e: EVENT.OVERPENETRATION,
        to: readU32(fields[1], "event.overpenetration.to"),
      };
    case EVENT.MISS:
      requireLength(fields, 2, `miss event ${index}`);
      return {
        e: EVENT.MISS,
        to: readU32(fields[1], "event.miss.to"),
      };
    case EVENT.DEATH:
      requireLength(fields, 5, `death event ${index}`);
      return {
        e: EVENT.DEATH,
        id: readU32(fields[1], "event.id"),
        x: readNumber(fields[2], "event.x"),
        y: readNumber(fields[3], "event.y"),
        kind: readCode(fields[4], KIND_BY_CODE, "event.kind"),
      };
    case EVENT.BUILD:
      requireLength(fields, 3, `build event ${index}`);
      return {
        e: EVENT.BUILD,
        id: readU32(fields[1], "event.id"),
        kind: readCode(fields[2], KIND_BY_CODE, "event.kind"),
      };
    case EVENT.NOTICE:
      if (fields.length !== 2 && fields.length !== 3 && fields.length !== 5) {
        throw new Error(`notice event ${index} field count mismatch`);
      }
      if (typeof fields[1] !== "string") throw new Error(`notice event ${index} msg must be string`);
      return decodeCompactNotice(fields, index);
    case EVENT.SMOKE_LAUNCH: {
      requireLength(fields, 4, `smoke launch event ${index}`);
      const from = decodeCompactPoint(fields[1], "event.smokeLaunch.from");
      const to = decodeCompactPoint(fields[2], "event.smokeLaunch.to");
      return {
        e: EVENT.SMOKE_LAUNCH,
        fromX: from[0],
        fromY: from[1],
        toX: to[0],
        toY: to[1],
        delayTicks: readU32(fields[3], "event.smokeLaunch.delayTicks"),
      };
    }
    case EVENT.MORTAR_LAUNCH: {
      requireLength(fields, 6, `mortar launch event ${index}`);
      const fromPoint = decodeCompactPoint(fields[2], "event.mortarLaunch.from");
      const to = decodeCompactPoint(fields[3], "event.mortarLaunch.to");
      return {
        e: EVENT.MORTAR_LAUNCH,
        from: readU32(fields[1], "event.mortarLaunch.from"),
        fromX: fromPoint[0],
        fromY: fromPoint[1],
        toX: to[0],
        toY: to[1],
        radiusTiles: readNumber(fields[4], "event.mortarLaunch.radiusTiles"),
        delayTicks: readU32(fields[5], "event.mortarLaunch.delayTicks"),
      };
    }
    case EVENT.MORTAR_IMPACT:
      if (fields.length !== 4 && fields.length !== 5 && fields.length !== 6) {
        throw new Error(`mortar impact event ${index} field count mismatch`);
      }
      {
        const ev = {
          e: EVENT.MORTAR_IMPACT,
          x: readNumber(fields[1], "event.mortarImpact.x"),
          y: readNumber(fields[2], "event.mortarImpact.y"),
          radiusTiles: readNumber(fields[3], "event.mortarImpact.radiusTiles"),
        };
        if (fields.length > 4 && fields[4] != null) {
          ev.from = readU32(fields[4], "event.mortarImpact.from");
        }
        if (fields.length > 5 && fields[5] != null) {
          ev.reveal = decodeCompactAttackReveal(fields[5], index);
        }
        return ev;
      }
    case EVENT.ARTILLERY_TARGET: {
      requireLength(fields, 5, `artillery target event ${index}`);
      const target = decodeCompactPoint(fields[2], "event.artilleryTarget.target");
      return {
        e: EVENT.ARTILLERY_TARGET,
        from: readU32(fields[1], "event.artilleryTarget.from"),
        x: target[0],
        y: target[1],
        radiusTiles: readNumber(fields[3], "event.artilleryTarget.radiusTiles"),
        delayTicks: readU32(fields[4], "event.artilleryTarget.delayTicks"),
      };
    }
    case EVENT.ARTILLERY_FIRING:
      requireLength(fields, 5, `artillery firing event ${index}`);
      return {
        e: EVENT.ARTILLERY_FIRING,
        owner: readU32(fields[1], "event.artilleryFiring.owner"),
        x: readNumber(fields[2], "event.artilleryFiring.x"),
        y: readNumber(fields[3], "event.artilleryFiring.y"),
        facing: readNumber(fields[4], "event.artilleryFiring.facing"),
      };
    case EVENT.ARTILLERY_IMPACT:
      requireLength(fields, 4, `artillery impact event ${index}`);
      return {
        e: EVENT.ARTILLERY_IMPACT,
        x: readNumber(fields[1], "event.artilleryImpact.x"),
        y: readNumber(fields[2], "event.artilleryImpact.y"),
        radiusTiles: readNumber(fields[3], "event.artilleryImpact.radiusTiles"),
      };
    case EVENT.PANZERFAUST_LAUNCH: {
      requireLength(fields, 5, `panzerfaust launch event ${index}`);
      const from = decodeCompactPoint(fields[2], "event.panzerfaustLaunch.from");
      const to = decodeCompactPoint(fields[3], "event.panzerfaustLaunch.to");
      return {
        e: EVENT.PANZERFAUST_LAUNCH,
        from: readU32(fields[1], "event.panzerfaustLaunch.from"),
        fromX: from[0],
        fromY: from[1],
        toX: to[0],
        toY: to[1],
        delayTicks: readU32(fields[4], "event.panzerfaustLaunch.delayTicks"),
      };
    }
    case EVENT.PANZERFAUST_IMPACT:
      requireLength(fields, 3, `panzerfaust impact event ${index}`);
      return {
        e: EVENT.PANZERFAUST_IMPACT,
        x: readNumber(fields[1], "event.panzerfaustImpact.x"),
        y: readNumber(fields[2], "event.panzerfaustImpact.y"),
      };
    case EVENT.PANZERFAUST_CONVERSION:
      requireLength(fields, 3, `panzerfaust conversion event ${index}`);
      return {
        e: EVENT.PANZERFAUST_CONVERSION,
        id: readU32(fields[1], "event.panzerfaustConversion.id"),
        toKind: readCode(fields[2], KIND_BY_CODE, "event.panzerfaustConversion.toKind"),
      };
    default:
      throw new Error(`unknown compact event kind ${eventKind}`);
  }
}

function decodeCompactAttackReveal(record, index) {
  const fields = readArray(record, `attack reveal ${index}`, 7);
  if (fields.length < 4) throw new Error(`attack reveal ${index} is too short`);
  const reveal = {
    owner: readU32(fields[0], "attackReveal.owner"),
    kind: readCode(fields[1], KIND_BY_CODE, "attackReveal.kind"),
    x: readNumber(fields[2], "attackReveal.x"),
    y: readNumber(fields[3], "attackReveal.y"),
  };
  assignOptional(reveal, "facing", fields, 4, readNumber);
  assignOptional(reveal, "weaponFacing", fields, 5, readNumber);
  assignOptionalCode(reveal, "setupState", fields, 6, SETUP_BY_CODE);
  return reveal;
}

function decodeCompactNotice(fields, index) {
  const ev = {
    e: EVENT.NOTICE,
    msg: fields[1],
    severity: NOTICE_SEVERITY.INFO,
  };
  if (fields.length > 2) {
    ev.severity = readCode(fields[2], NOTICE_SEVERITY_BY_CODE, `notice event ${index}.severity`);
  }
  if (fields.length > 3) {
    ev.x = readNumber(fields[3], `notice event ${index}.x`);
    ev.y = readNumber(fields[4], `notice event ${index}.y`);
  }
  return ev;
}

function readOptionalWeaponKind(value, name) {
  if (value == null) return undefined;
  const code = readU32(value, name);
  return Object.prototype.hasOwnProperty.call(WEAPON_KIND_BY_CODE, code)
    ? WEAPON_KIND_BY_CODE[code]
    : undefined;
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

function readCode(value, table, name) {
  const code = readU32(value, name);
  if (!Object.prototype.hasOwnProperty.call(table, code)) {
    throw new Error(`${name} has unknown code ${code}`);
  }
  return table[code];
}

function requireLength(fields, expected, name) {
  if (fields.length !== expected) throw new Error(`${name} field count mismatch`);
}

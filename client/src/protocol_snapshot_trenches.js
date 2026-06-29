import { MAX_COMPACT_TRENCHES } from "./protocol_constants.js";

export function decodeCompactTrenches(record) {
  if (record == null) return [];
  return readArray(record, "trenches", MAX_COMPACT_TRENCHES).map(decodeCompactTrench);
}

function decodeCompactTrench(record, index) {
  const fields = readArray(record, `trench ${index}`, 4);
  if (fields.length !== 4) throw new Error(`trench ${index} field count mismatch`);
  return {
    id: readU32(fields[0], "trench.id"),
    x: readNumber(fields[1], "trench.x"),
    y: readNumber(fields[2], "trench.y"),
    radiusTiles: readNumber(fields[3], "trench.radiusTiles"),
  };
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

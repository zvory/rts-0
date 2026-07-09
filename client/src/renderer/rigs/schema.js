import { UNIT_KINDS, BUILDING_KINDS } from "../../protocol.js";

export const RIG_SCHEMA_VERSION = 1;

export const REQUIRED_ANCHORS = Object.freeze(["origin", "selection", "hp"]);
export const TINT_SLOTS = Object.freeze([
  "team",
  "team-light",
  "team-light-soft",
  "team-light-strong",
  "team-light-08",
  "team-light-10",
  "team-light-14",
  "team-light-24",
  "team-stroke",
  "team-fill-stroke",
  "neutral",
  "fixed",
]);
export const GEOMETRY_TYPES = Object.freeze(["rect", "circle", "ellipse", "line", "polygon", "polyline", "path"]);
export const ANIMATION_INPUTS = Object.freeze([
  "now",
  "teamColor",
  "recoilProgress",
  "recoilPx",
  "recoilKickX",
  "recoilKickY",
  "setupVisual",
  "vehicleMotion",
  "selected",
  "damaged",
  "shotRevealAlpha",
  "visibility",
  "mapTileSize",
  "facing",
  "weaponFacing",
  "weaponFacingCos",
  "weaponFacingSin",
  "weaponVisualFacing",
  "carriageVisualFacing",
  "weaponVisualDoubleCos",
  "weaponVisualDoubleSin",
  "weaponRecoilX",
  "weaponRecoilY",
  "scoutGunnerX",
  "scoutGunnerY",
  "scoutMountX",
  "scoutMountY",
  "setupVisible",
  "setupMostlyDeployed",
  "setupBarrelVisible",
  "busy",
  "breakthroughTicks",
  "lowOil",
  "oilStarved",
  "fuelCueVisible",
  "panzerfaustLoaded",
]);
export const ANIMATION_PROPERTIES = Object.freeze([
  "transform.x",
  "transform.y",
  "transform.rotation",
  "transform.scaleX",
  "transform.scaleY",
  "transform.localX",
  "transform.localY",
  "geometry.scaleX",
  "geometry.scaleY",
  "alpha",
  "visible",
  "tintSlot",
]);

const UNIT_KIND_SET = new Set([...UNIT_KINDS, ...BUILDING_KINDS]);
const TINT_SLOT_SET = new Set(TINT_SLOTS);
const GEOMETRY_TYPE_SET = new Set(GEOMETRY_TYPES);
const ANIMATION_INPUT_SET = new Set(ANIMATION_INPUTS);
const ANIMATION_PROPERTY_SET = new Set(ANIMATION_PROPERTIES);

/**
 * @typedef {{x: number, y: number}} RigPoint
 * @typedef {{x: number, y: number, rotation: number, scaleX: number, scaleY: number}} RigTransform
 * @typedef {{fill: string | null, stroke: string | null, strokeWidth: number | null, opacity: number, fillOpacity: number, strokeOpacity: number}} RigPaint
 * @typedef {{id: string, drawOrder: number, geometry: object, transform: RigTransform, pivot: RigPoint, tintSlot: string, paint: RigPaint}} RigPart
 * @typedef {{id: string, kind: string, schemaVersion: number, parts: RigPart[], anchors: Record<string, RigPoint>, bounds: object, animations: object[], requiredRuntimeInputs: string[]}} RigDefinition
 */

export function validateRigDefinition(definition, options = {}) {
  const errors = [];
  const source = isPlainObject(definition) ? definition : {};

  if (!isPlainObject(definition)) {
    errors.push(error("rig.invalidDefinition", "", "Rig definition must be a plain object."));
  }

  const id = readRequiredString(source, "id", errors);
  const kind = readRequiredString(source, "kind", errors);
  if (kind && !UNIT_KIND_SET.has(kind)) {
    errors.push(error("rig.invalidUnitKind", "kind", `Unknown unit kind ${JSON.stringify(kind)}.`));
  }
  if (typeof options.expectedKind === "string" && kind && kind !== options.expectedKind) {
    errors.push(error("rig.unitKindMismatch", "kind", `Rig kind ${JSON.stringify(kind)} does not match expected kind ${JSON.stringify(options.expectedKind)}.`));
  }
  if (typeof source.authoredKind === "string" && kind && source.authoredKind !== kind) {
    errors.push(error("rig.unitKindMismatch", "authoredKind", `Authored kind ${JSON.stringify(source.authoredKind)} does not match rig kind ${JSON.stringify(kind)}.`));
  }

  const schemaVersion = source.schemaVersion ?? RIG_SCHEMA_VERSION;
  if (!Number.isInteger(schemaVersion) || schemaVersion !== RIG_SCHEMA_VERSION) {
    errors.push(error("rig.unsupportedSchemaVersion", "schemaVersion", `Rig schemaVersion must be ${RIG_SCHEMA_VERSION}.`));
  }

  const requiredRuntimeInputs = normalizeStringArray(source.requiredRuntimeInputs, "requiredRuntimeInputs", ANIMATION_INPUT_SET, "rig.invalidRuntimeInput", errors);
  const parts = normalizeParts(source.parts, errors);
  const anchors = normalizeAnchors(source.anchors, errors);
  const bounds = normalizeBounds(source.bounds, errors);
  const animations = normalizeAnimations(source.animations, parts.ids, errors);

  for (const anchor of REQUIRED_ANCHORS) {
    if (!anchors.value[anchor]) {
      errors.push(error("rig.missingRequiredAnchor", `anchors.${anchor}`, `Rig is missing required ${anchor} anchor.`));
    }
  }

  if (errors.length > 0) {
    return { ok: false, errors };
  }

  return {
    ok: true,
    definition: {
      id,
      kind,
      schemaVersion,
      parts: parts.value,
      anchors: anchors.value,
      bounds,
      animations,
      requiredRuntimeInputs,
    },
    errors: [],
  };
}

function normalizeParts(parts, errors) {
  const ids = new Set();
  if (!Array.isArray(parts) || parts.length === 0) {
    errors.push(error("rig.missingParts", "parts", "Rig must define at least one part."));
    return { value: [], ids };
  }

  const normalized = [];
  parts.forEach((part, index) => {
    const path = `parts.${index}`;
    if (!isPlainObject(part)) {
      errors.push(error("rig.invalidPart", path, "Rig part must be a plain object."));
      return;
    }

    const id = readRequiredString(part, `${path}.id`, errors, "id");
    if (id) {
      if (ids.has(id)) {
        errors.push(error("rig.duplicatePartId", `${path}.id`, `Duplicate part id ${JSON.stringify(id)}.`));
      }
      ids.add(id);
    }

    const drawOrder = readFiniteNumber(part.drawOrder, `${path}.drawOrder`, errors, { integer: true });
    const geometry = normalizeGeometry(part.geometry, `${path}.geometry`, errors);
    const transform = normalizeTransform(part.transform, `${path}.transform`, errors);
    const pivot = normalizePoint(part.pivot ?? { x: 0, y: 0 }, `${path}.pivot`, errors);
    const paint = normalizePaint(part.paint, `${path}.paint`, errors);
    const tintSlot = part.tintSlot ?? "fixed";
    if (!TINT_SLOT_SET.has(tintSlot)) {
      errors.push(error("rig.invalidTintSlot", `${path}.tintSlot`, `Unsupported tint slot ${JSON.stringify(tintSlot)}.`));
    }

    if (id && Number.isFinite(drawOrder) && geometry && transform && pivot && paint && TINT_SLOT_SET.has(tintSlot)) {
      normalized.push({ id, drawOrder, geometry, transform, pivot, tintSlot, paint });
    }
  });

  normalized.sort((a, b) => a.drawOrder - b.drawOrder || a.id.localeCompare(b.id));
  return { value: normalized, ids };
}

function normalizePaint(paint, path, errors) {
  const source = paint ?? {};
  if (!isPlainObject(source)) {
    errors.push(error("rig.invalidPaint", path, "Paint must be a plain object."));
    return null;
  }
  const fill = normalizePaintColor(source.fill ?? null, `${path}.fill`, errors);
  const stroke = normalizePaintColor(source.stroke ?? null, `${path}.stroke`, errors);
  const strokeWidth = source.strokeWidth == null ? null : readPositiveNumber(source.strokeWidth, `${path}.strokeWidth`, errors);
  const opacity = readUnitNumber(source.opacity ?? 1, `${path}.opacity`, errors);
  const fillOpacity = readUnitNumber(source.fillOpacity ?? 1, `${path}.fillOpacity`, errors);
  const strokeOpacity = readUnitNumber(source.strokeOpacity ?? 1, `${path}.strokeOpacity`, errors);
  if ((fill === null || typeof fill === "string") && (stroke === null || typeof stroke === "string") && (strokeWidth === null || Number.isFinite(strokeWidth)) && Number.isFinite(opacity) && Number.isFinite(fillOpacity) && Number.isFinite(strokeOpacity)) {
    return { fill, stroke, strokeWidth, opacity, fillOpacity, strokeOpacity };
  }
  return null;
}

function normalizePaintColor(value, path, errors) {
  if (value === null) return null;
  if (typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value)) {
    return value.toLowerCase();
  }
  errors.push(error("rig.invalidPaintColor", path, `${path} must be null or a six-digit hex color.`));
  return undefined;
}

function readUnitNumber(value, path, errors) {
  const number = readFiniteNumber(value, path, errors);
  if (Number.isFinite(number) && (number < 0 || number > 1)) {
    errors.push(error("rig.outOfRangeNumber", path, `${path} must be between zero and one.`));
    return NaN;
  }
  return number;
}

function normalizeGeometry(geometry, path, errors) {
  if (!isPlainObject(geometry)) {
    errors.push(error("rig.invalidGeometry", path, "Geometry must be a plain object."));
    return null;
  }
  const type = geometry.type;
  if (!GEOMETRY_TYPE_SET.has(type)) {
    errors.push(error("rig.unsupportedGeometry", `${path}.type`, `Unsupported geometry type ${JSON.stringify(type)}.`));
    return null;
  }

  if (type === "rect") {
    const x = readFiniteNumber(geometry.x ?? 0, `${path}.x`, errors);
    const y = readFiniteNumber(geometry.y ?? 0, `${path}.y`, errors);
    const width = readPositiveNumber(geometry.width, `${path}.width`, errors);
    const height = readPositiveNumber(geometry.height, `${path}.height`, errors);
    return numbersAreFinite([x, y, width, height]) ? { type, x, y, width, height } : null;
  }
  if (type === "circle") {
    const cx = readFiniteNumber(geometry.cx ?? 0, `${path}.cx`, errors);
    const cy = readFiniteNumber(geometry.cy ?? 0, `${path}.cy`, errors);
    const r = readPositiveNumber(geometry.r, `${path}.r`, errors);
    return numbersAreFinite([cx, cy, r]) ? { type, cx, cy, r } : null;
  }
  if (type === "ellipse") {
    const cx = readFiniteNumber(geometry.cx ?? 0, `${path}.cx`, errors);
    const cy = readFiniteNumber(geometry.cy ?? 0, `${path}.cy`, errors);
    const rx = readPositiveNumber(geometry.rx, `${path}.rx`, errors);
    const ry = readPositiveNumber(geometry.ry, `${path}.ry`, errors);
    return numbersAreFinite([cx, cy, rx, ry]) ? { type, cx, cy, rx, ry } : null;
  }
  if (type === "line") {
    const from = normalizePoint(geometry.from, `${path}.from`, errors);
    const to = normalizePoint(geometry.to, `${path}.to`, errors);
    const strokeWidth = readPositiveNumber(geometry.strokeWidth ?? 1, `${path}.strokeWidth`, errors);
    return from && to && Number.isFinite(strokeWidth) ? { type, from, to, strokeWidth } : null;
  }
  if (type === "polygon" || type === "polyline") {
    const points = normalizePoints(geometry.points, `${path}.points`, errors);
    const minimum = type === "polygon" ? 3 : 2;
    if (points.length < minimum) {
      errors.push(error("rig.invalidGeometry", `${path}.points`, `${type} geometry must include at least ${minimum} points.`));
      return null;
    }
    return { type, points };
  }

  const commands = normalizePathCommands(geometry.commands, `${path}.commands`, errors);
  return commands.length > 0 ? { type, commands } : null;
}

function normalizeTransform(transform, path, errors) {
  const source = transform ?? {};
  if (!isPlainObject(source)) {
    errors.push(error("rig.unsupportedTransform", path, "Transform must be a plain object."));
    return null;
  }
  const allowedKeys = new Set(["x", "y", "rotation", "scaleX", "scaleY"]);
  for (const key of Object.keys(source)) {
    if (!allowedKeys.has(key)) {
      errors.push(error("rig.unsupportedTransform", `${path}.${key}`, `Unsupported transform component ${JSON.stringify(key)}.`));
    }
  }
  const x = readFiniteNumber(source.x ?? 0, `${path}.x`, errors);
  const y = readFiniteNumber(source.y ?? 0, `${path}.y`, errors);
  const rotation = readFiniteNumber(source.rotation ?? 0, `${path}.rotation`, errors);
  const scaleX = readFiniteNumber(source.scaleX ?? 1, `${path}.scaleX`, errors);
  const scaleY = readFiniteNumber(source.scaleY ?? 1, `${path}.scaleY`, errors);
  return numbersAreFinite([x, y, rotation, scaleX, scaleY]) ? { x, y, rotation, scaleX, scaleY } : null;
}

function normalizeAnchors(anchors, errors) {
  const value = {};
  if (!isPlainObject(anchors)) {
    errors.push(error("rig.invalidAnchors", "anchors", "Anchors must be a plain object."));
    return { value };
  }
  for (const [name, point] of Object.entries(anchors)) {
    if (!isIdentifier(name)) {
      errors.push(error("rig.invalidAnchorName", `anchors.${name}`, `Invalid anchor name ${JSON.stringify(name)}.`));
      continue;
    }
    const normalized = normalizePoint(point, `anchors.${name}`, errors);
    if (normalized) value[name] = normalized;
  }
  return { value };
}

function normalizeBounds(bounds, errors) {
  const source = isPlainObject(bounds) ? bounds : {};
  if (!isPlainObject(bounds)) {
    errors.push(error("rig.invalidBounds", "bounds", "Bounds must be a plain object."));
  }
  const value = {};
  for (const [name, rect] of Object.entries(source)) {
    if (!isIdentifier(name)) {
      errors.push(error("rig.invalidBoundsName", `bounds.${name}`, `Invalid bounds name ${JSON.stringify(name)}.`));
      continue;
    }
    const normalized = normalizeRect(rect, `bounds.${name}`, errors);
    if (normalized) value[name] = normalized;
  }
  return value;
}

function normalizeAnimations(animations, partIds, errors) {
  if (animations == null) return [];
  if (!Array.isArray(animations)) {
    errors.push(error("rig.invalidAnimations", "animations", "Animations must be an array."));
    return [];
  }

  const normalized = [];
  animations.forEach((binding, index) => {
    const path = `animations.${index}`;
    if (!isPlainObject(binding)) {
      errors.push(error("rig.invalidAnimation", path, "Animation binding must be a plain object."));
      return;
    }
    const partId = readRequiredString(binding, `${path}.partId`, errors, "partId");
    if (partId && !partIds.has(partId)) {
      errors.push(error("rig.invalidAnimationReference", `${path}.partId`, `Animation references unknown part ${JSON.stringify(partId)}.`));
    }
    const input = readRequiredString(binding, `${path}.input`, errors, "input");
    if (input && !ANIMATION_INPUT_SET.has(input)) {
      errors.push(error("rig.invalidAnimationInput", `${path}.input`, `Unsupported animation input ${JSON.stringify(input)}.`));
    }
    const property = readRequiredString(binding, `${path}.property`, errors, "property");
    if (property && !ANIMATION_PROPERTY_SET.has(property)) {
      errors.push(error("rig.invalidAnimationProperty", `${path}.property`, `Unsupported animation property ${JSON.stringify(property)}.`));
    }
    const factor = readFiniteNumber(binding.factor ?? 1, `${path}.factor`, errors);
    const offset = readFiniteNumber(binding.offset ?? 0, `${path}.offset`, errors);

    if (partId && partIds.has(partId) && input && ANIMATION_INPUT_SET.has(input) && property && ANIMATION_PROPERTY_SET.has(property) && Number.isFinite(factor) && Number.isFinite(offset)) {
      normalized.push({ partId, input, property, factor, offset });
    }
  });
  return normalized;
}

function normalizeStringArray(value, path, allowed, code, errors) {
  if (value == null) return [];
  if (!Array.isArray(value)) {
    errors.push(error("rig.invalidStringArray", path, `${path} must be an array.`));
    return [];
  }
  const out = [];
  const seen = new Set();
  value.forEach((item, index) => {
    if (typeof item !== "string" || !allowed.has(item)) {
      errors.push(error(code, `${path}.${index}`, `Unsupported value ${JSON.stringify(item)}.`));
      return;
    }
    if (!seen.has(item)) {
      seen.add(item);
      out.push(item);
    }
  });
  return out;
}

function normalizePoint(point, path, errors) {
  if (!isPlainObject(point)) {
    errors.push(error("rig.invalidPoint", path, "Point must be a plain object."));
    return null;
  }
  const x = readFiniteNumber(point.x, `${path}.x`, errors);
  const y = readFiniteNumber(point.y, `${path}.y`, errors);
  return numbersAreFinite([x, y]) ? { x, y } : null;
}

function normalizePoints(points, path, errors) {
  if (!Array.isArray(points)) {
    errors.push(error("rig.invalidPoints", path, "Points must be an array."));
    return [];
  }
  const normalized = [];
  points.forEach((point, index) => {
    const normalizedPoint = Array.isArray(point)
      ? normalizePoint({ x: point[0], y: point[1] }, `${path}.${index}`, errors)
      : normalizePoint(point, `${path}.${index}`, errors);
    if (normalizedPoint) normalized.push(normalizedPoint);
  });
  return normalized;
}

function normalizeRect(rect, path, errors) {
  if (!isPlainObject(rect)) {
    errors.push(error("rig.invalidBounds", path, "Bounds entry must be a plain object."));
    return null;
  }
  const x = readFiniteNumber(rect.x, `${path}.x`, errors);
  const y = readFiniteNumber(rect.y, `${path}.y`, errors);
  const width = readPositiveNumber(rect.width, `${path}.width`, errors);
  const height = readPositiveNumber(rect.height, `${path}.height`, errors);
  return numbersAreFinite([x, y, width, height]) ? { x, y, width, height } : null;
}

function normalizePathCommands(commands, path, errors) {
  if (!Array.isArray(commands)) {
    errors.push(error("rig.invalidPathCommands", path, "Path commands must be an array."));
    return [];
  }
  const allowed = new Set(["M", "L", "C", "Q", "Z"]);
  const normalized = [];
  commands.forEach((command, index) => {
    const commandPath = `${path}.${index}`;
    if (!isPlainObject(command) || !allowed.has(command.command) || !Array.isArray(command.values)) {
      errors.push(error("rig.invalidPathCommand", commandPath, "Path command must have an allowed command and numeric values."));
      return;
    }
    const values = [];
    command.values.forEach((value, valueIndex) => {
      const normalizedValue = readFiniteNumber(value, `${commandPath}.values.${valueIndex}`, errors);
      if (Number.isFinite(normalizedValue)) values.push(normalizedValue);
    });
    normalized.push({ command: command.command, values });
  });
  return normalized;
}

function readRequiredString(source, path, errors, key = path) {
  const value = source[key];
  if (typeof value !== "string" || value.trim() === "") {
    errors.push(error("rig.invalidString", path, `${path} must be a non-empty string.`));
    return null;
  }
  return value;
}

function readFiniteNumber(value, path, errors, options = {}) {
  if (typeof value !== "number" || !Number.isFinite(value) || (options.integer && !Number.isInteger(value))) {
    errors.push(error("rig.nonFiniteNumber", path, `${path} must be a finite${options.integer ? " integer" : ""} number.`));
    return NaN;
  }
  return value;
}

function readPositiveNumber(value, path, errors) {
  const number = readFiniteNumber(value, path, errors);
  if (Number.isFinite(number) && number <= 0) {
    errors.push(error("rig.nonPositiveNumber", path, `${path} must be greater than zero.`));
    return NaN;
  }
  return number;
}

function numbersAreFinite(values) {
  return values.every((value) => Number.isFinite(value));
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isIdentifier(value) {
  return typeof value === "string" && /^[a-z][a-zA-Z0-9]*(?:[._-][a-zA-Z0-9]+)*$/.test(value);
}

function error(code, path, message) {
  return { code, path, message };
}

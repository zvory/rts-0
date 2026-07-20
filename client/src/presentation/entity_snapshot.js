export const PRESENTATION_ENTITY_FIELDS = Object.freeze([
  "id", "kind", "owner", "x", "y", "facing", "weaponFacing", "state",
  "hp", "maxHp", "remaining", "latchedNode", "occupiedTrenchId",
  "buildProgress", "deconstructProgress", "prodProgress", "prodQueue", "prodRepeatKinds",
  "setupState", "setupFacing", "recoilPhase", "recoilProgress",
  "panzerfaustLoaded", "breakthroughTicks", "breakthroughAuraTicks", "abilities",
  "extractorActive",
  "orderPlan", "rally", "rallyPlan", "optimisticRally", "debugPath",
  "attackArcRad", "attackMinRangePx", "attackMinRangeTiles", "attackRangePx",
  "attackRangeTiles", "attackRangeProfile", "firingArcRad", "firingMinRangePx",
  "firingMinRangeTiles", "firingRangePx", "firingRangeTiles", "firingRangeProfile",
  "weaponArcRad", "weaponMinRangePx", "weaponMinRangeTiles", "weaponRangePx",
  "weaponRangeTiles", "weaponRangeProfile", "aboveFogReveal", "shotRevealCreatedAt", "shotRevealExpiresAt",
]);

const ENTITY_FIELDS = new Set(PRESENTATION_ENTITY_FIELDS);
const PREPARED_ENTRIES = new WeakSet();

export function prepareEntitySnapshots(entities) {
  const entries = [];
  const debug = { interactionObjects: 0, interactionArrays: 0, admittedNestedReuses: 0 };
  for (const source of Array.isArray(entities) ? entities : []) {
    const state = {
      seen: new WeakMap(),
      presentationError: null,
      presentation: { valid: true, depth: 0, ancestors: new Set() },
      debug,
    };
    let interaction = null;
    try {
      interaction = cloneRoot(source, state);
    } catch (error) {
      state.presentationError = error instanceof Error
        ? error.message
        : "Prepared entity could not be detached.";
    }
    const entry = Object.freeze({
      source,
      interaction,
      presentationError: state.presentationError,
    });
    PREPARED_ENTRIES.add(entry);
    entries.push(entry);
  }
  return Object.freeze({
    entries: Object.freeze(entries),
    debug: Object.freeze({ ...debug }),
  });
}

export function preparedPresentationEntityRecord(prepared, derived) {
  if (!PREPARED_ENTRIES.has(prepared) || prepared.presentationError) {
    throw new TypeError(prepared?.presentationError || "Prepared presentation entity is invalid.");
  }
  const record = { type: derived.type };
  for (const field of PRESENTATION_ENTITY_FIELDS) {
    if (prepared.interaction[field] !== undefined) record[field] = prepared.interaction[field];
  }
  record.x = derived.x;
  record.y = derived.y;
  record.owner = derived.owner;
  record.relationship = derived.relationship;
  record.teamColor = derived.teamColor;
  record.selected = derived.selected;
  record.visualBounds = freezeDerived(derived.visualBounds);
  record.anchors = freezeDerived(derived.anchors);
  return Object.freeze(record);
}

function cloneRoot(source, state) {
  if (source == null || typeof source !== "object") return source;
  const interaction = Array.isArray(source) ? [] : {};
  state.seen.set(source, interaction);
  if (Array.isArray(source)) state.debug.interactionArrays += 1;
  else state.debug.interactionObjects += 1;
  for (const [key, value] of Object.entries(source)) {
    const admittedField = ENTITY_FIELDS.has(key);
    const presentation = state.presentation;
    presentation.valid = true;
    presentation.depth = 1;
    presentation.ancestors.clear();
    const cloned = cloneValue(value, state, admittedField);
    interaction[key] = cloned;
    if (admittedField) {
      if (!presentation.valid && !state.presentationError) {
        state.presentationError = "Prepared entity contains unsupported admitted presentation data.";
      } else if (presentation.valid && value !== undefined && value && typeof value === "object") {
        state.debug.admittedNestedReuses += 1;
      }
    }
  }
  return Object.freeze(interaction);
}

function cloneValue(value, state, validatePresentation) {
  const presentation = state.presentation;
  if (validatePresentation) validatePresentationValue(value, presentation);
  if (value == null || typeof value !== "object") return value;
  if (state.seen.has(value)) {
    if (validatePresentation && presentation.valid) validatePresentationGraph(value, presentation);
    return state.seen.get(value);
  }
  const out = Array.isArray(value) ? [] : {};
  state.seen.set(value, out);
  if (Array.isArray(value)) state.debug.interactionArrays += 1;
  else state.debug.interactionObjects += 1;
  const trackingPresentationPath = validatePresentation && presentation.valid;
  if (trackingPresentationPath) {
    presentation.ancestors.add(value);
    presentation.depth += 1;
  }
  if (Array.isArray(value)) {
    for (const item of value) out.push(cloneValue(item, state, validatePresentation));
  } else {
    for (const [key, item] of Object.entries(value)) {
      out[key] = cloneValue(item, state, validatePresentation);
    }
  }
  if (trackingPresentationPath) {
    presentation.depth -= 1;
    presentation.ancestors.delete(value);
  }
  return Object.freeze(out);
}

function validatePresentationGraph(value, presentation) {
  validatePresentationValue(value, presentation);
  if (!presentation.valid || value == null || typeof value !== "object") return;
  presentation.ancestors.add(value);
  presentation.depth += 1;
  for (const item of Array.isArray(value) ? value : Object.values(value)) {
    validatePresentationGraph(item, presentation);
    if (!presentation.valid) break;
  }
  presentation.depth -= 1;
  presentation.ancestors.delete(value);
}

function validatePresentationValue(value, presentation) {
  if (!presentation?.valid) return;
  if (value == null || typeof value === "string" || typeof value === "boolean") return;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) presentation.valid = false;
    return;
  }
  if (typeof value !== "object" || presentation.depth > 16 || presentation.ancestors.has(value)) {
    presentation.valid = false;
    return;
  }
  if (ArrayBuffer.isView(value) || value instanceof Map || value instanceof Set) {
    presentation.valid = false;
    return;
  }
  const prototype = Object.getPrototypeOf(value);
  if (!Array.isArray(value) && prototype !== Object.prototype && prototype !== null) {
    presentation.valid = false;
  }
}

function freezeDerived(value) {
  if (value == null || typeof value !== "object") return value;
  for (const key of Object.keys(value)) freezeDerived(value[key]);
  return Object.freeze(value);
}

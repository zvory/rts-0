import { isUnit } from "../protocol.js";

const SAFE_ID_RE = /^[A-Za-z0-9_-]{1,64}$/;
const NEAREST_TIE_EPSILON = 0.001;

export function resolveVisualUnitOverrides(rules, entities, candidateDefinitions, options = {}) {
  const overrides = new Map();
  const errors = [];
  const list = Array.isArray(rules) ? rules : [];
  if (list.length === 0) {
    return {
      overrides,
      errors,
      diagnostics: Object.freeze({ rules: 0, activeOverrides: 0, errors: 0 }),
    };
  }

  const unitEntities = (Array.isArray(entities) ? entities : [])
    .filter((entity) => isUnit(entity?.kind));
  const candidateErrors = options.candidateErrors || new Map();

  for (let index = 0; index < list.length; index += 1) {
    const rule = list[index];
    const ruleId = safeRuleId(rule, index);
    const candidateId = typeof rule?.candidateId === "string" ? rule.candidateId : "";
    if (!SAFE_ID_RE.test(candidateId)) {
      errors.push(ruleError("invalid-candidate-id", ruleId, index, candidateId, "Visual unit override candidate id is invalid."));
      continue;
    }
    const candidate = candidateDefinitions?.get?.(candidateId) || null;
    if (!candidate) {
      const invalid = candidateErrors?.get?.(candidateId);
      errors.push(ruleError(
        invalid ? "invalid-candidate" : "unknown-candidate",
        ruleId,
        index,
        candidateId,
        invalid
          ? `Visual unit override candidate "${candidateId}" failed SVG rig validation.`
          : `Visual unit override candidate "${candidateId}" is not registered.`,
        { candidateErrors: invalid || undefined },
      ));
      continue;
    }

    const selected = selectVisualUnitOverrideEntity(rule?.selector, unitEntities);
    if (!selected.ok) {
      errors.push(ruleError(selected.reason, ruleId, index, candidateId, selected.message, {
        selector: rule?.selector || null,
        matches: selected.matches || undefined,
      }));
      continue;
    }
    if (candidate.kind !== selected.entity.kind) {
      errors.push(ruleError(
        "candidate-kind-mismatch",
        ruleId,
        index,
        candidateId,
        `Visual unit override candidate "${candidateId}" is for ${candidate.kind}, not ${selected.entity.kind}.`,
        { entityId: selected.entity.id },
      ));
      continue;
    }
    if (overrides.has(selected.entity.id)) {
      errors.push(ruleError(
        "duplicate-target",
        ruleId,
        index,
        candidateId,
        `Visual unit override target ${selected.entity.id} is already assigned by another rule.`,
        { entityId: selected.entity.id },
      ));
      continue;
    }

    overrides.set(selected.entity.id, Object.freeze({
      ruleId,
      candidateId,
      label: candidate.label || candidateId,
      kind: candidate.kind,
      definition: candidate.definition,
    }));
  }

  return {
    overrides,
    errors,
    diagnostics: Object.freeze({
      rules: list.length,
      activeOverrides: overrides.size,
      errors: errors.length,
    }),
  };
}

export function selectVisualUnitOverrideEntity(selector, entities) {
  if (!selector || typeof selector !== "object") {
    return failed("selector-invalid", "Visual unit override selector is missing.");
  }

  let candidates = Array.isArray(entities) ? [...entities] : [];
  if (selector.entityId != null) {
    const entityId = Number(selector.entityId);
    if (!Number.isInteger(entityId) || entityId <= 0) {
      return failed("selector-invalid", "Visual unit override entityId must be a positive integer.");
    }
    candidates = candidates.filter((entity) => entity.id === entityId);
  }
  if (selector.kind != null) {
    if (typeof selector.kind !== "string" || selector.kind === "") {
      return failed("selector-invalid", "Visual unit override kind selector must be a unit kind string.");
    }
    candidates = candidates.filter((entity) => entity.kind === selector.kind);
  }
  if (selector.owner != null) {
    const owner = Number(selector.owner);
    if (!Number.isInteger(owner)) {
      return failed("selector-invalid", "Visual unit override owner selector must be an integer.");
    }
    candidates = candidates.filter((entity) => entity.owner === owner);
  }

  if (selector.ordinal != null) {
    const ordinal = Number(selector.ordinal);
    if (!Number.isInteger(ordinal) || ordinal < 1) {
      return failed("selector-invalid", "Visual unit override ordinal selector must be one-based.");
    }
    if (!selector.kind) {
      return failed("selector-invalid", "Visual unit override ordinal selectors must include a unit kind.");
    }
    const sorted = candidates.slice().sort(compareEntityIds);
    const entity = sorted[ordinal - 1] || null;
    return entity
      ? { ok: true, entity }
      : failed("selector-no-match", `No unit matched ordinal ${ordinal} for kind ${selector.kind}.`);
  }

  if (selector.nearest != null) {
    if (!selector.kind) {
      return failed("selector-invalid", "Visual unit override nearest selectors must include a unit kind.");
    }
    const point = selector.nearest;
    const x = Number(point?.x);
    const y = Number(point?.y);
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      return failed("selector-invalid", "Visual unit override nearest selector needs finite x and y.");
    }
    if (candidates.length === 0) {
      return failed("selector-no-match", `No units of kind ${selector.kind} are available for nearest selector.`);
    }
    const byDistance = candidates
      .map((entity) => ({ entity, distance: Math.hypot((entity.x ?? 0) - x, (entity.y ?? 0) - y) }))
      .sort((a, b) => a.distance - b.distance || compareEntityIds(a.entity, b.entity));
    const maxDistance = Number.isFinite(Number(selector.maxDistance)) ? Number(selector.maxDistance) : Infinity;
    if (byDistance[0].distance > maxDistance) {
      return failed(
        "selector-no-match",
        `Nearest ${selector.kind} was ${byDistance[0].distance.toFixed(2)}px away, beyond maxDistance ${maxDistance}.`,
      );
    }
    const tied = byDistance.filter((item) => Math.abs(item.distance - byDistance[0].distance) <= NEAREST_TIE_EPSILON);
    if (tied.length > 1) {
      return failed(
        "selector-ambiguous",
        "Nearest visual unit override selector matched multiple units at the same distance.",
        tied.map((item) => item.entity.id),
      );
    }
    return { ok: true, entity: byDistance[0].entity };
  }

  if (candidates.length === 0) {
    return failed("selector-no-match", "No unit matched visual override selector.");
  }
  if (candidates.length > 1) {
    return failed(
      "selector-ambiguous",
      "Visual unit override selector matched multiple units; add entityId, ordinal, or nearest.",
      candidates.map((entity) => entity.id),
    );
  }
  return { ok: true, entity: candidates[0] };
}

export function publishVisualUnitOverrideDiagnostics(result) {
  const errors = result?.errors || [];
  if (!errors.length) {
    if (globalThis.__rtsVisualUnitOverrideErrors) delete globalThis.__rtsVisualUnitOverrideErrors;
    return;
  }
  globalThis.__rtsVisualUnitOverrideErrors = {
    total: errors.length,
    latest: errors[errors.length - 1],
    errors,
  };
}

function safeRuleId(rule, index) {
  return typeof rule?.id === "string" && SAFE_ID_RE.test(rule.id) ? rule.id : `rule-${index}`;
}

function ruleError(reason, ruleId, index, candidateId, message, extra = {}) {
  return Object.freeze({
    reason,
    ruleId,
    index,
    candidateId,
    message,
    ...extra,
  });
}

function failed(reason, message, matches = null) {
  return { ok: false, reason, message, matches };
}

function compareEntityIds(a, b) {
  return (Number(a?.id) || 0) - (Number(b?.id) || 0);
}

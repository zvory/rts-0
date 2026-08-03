import { compileSvgRig } from "./svg_importer.js";

const CANDIDATE_ID_RE = /^[A-Za-z0-9_-]{1,64}$/;

// Raster units no longer ship duplicate SVG candidates. Keep the generic compiler for local
// experiments supplied by tests or future profiles that target an SVG-authored unit.
export const VISUAL_UNIT_RIG_CANDIDATE_SOURCES = Object.freeze([]);

export function visualUnitRigCandidateIds() {
  return VISUAL_UNIT_RIG_CANDIDATE_SOURCES.map((candidate) => candidate.id);
}

export function compileVisualUnitRigCandidates(entries = VISUAL_UNIT_RIG_CANDIDATE_SOURCES) {
  const definitions = new Map();
  const errors = new Map();
  const metadata = new Map();
  const list = Array.isArray(entries) ? entries : [];
  for (let index = 0; index < list.length; index += 1) {
    const entry = list[index];
    const id = typeof entry?.id === "string" ? entry.id : `candidate-${index}`;
    const label = typeof entry?.label === "string" ? entry.label : id;
    const kind = typeof entry?.kind === "string" ? entry.kind : "";
    metadata.set(id, Object.freeze({ id, label, kind }));
    if (!CANDIDATE_ID_RE.test(id)) {
      errors.set(id, [candidateError("candidate.invalidId", id, "Candidate ids must be allowlisted local identifiers.")]);
      continue;
    }
    if (!kind) {
      errors.set(id, [candidateError("candidate.invalidKind", id, "Candidate kind is required.")]);
      continue;
    }
    const compiled = compileSvgRig(entry?.svgText, { id, expectedKind: kind });
    if (!compiled.ok) {
      errors.set(id, compiled.errors);
      continue;
    }
    definitions.set(id, Object.freeze({
      id,
      label,
      kind,
      definition: compiled.definition,
    }));
  }
  return { definitions, errors, metadata };
}

function candidateError(code, path, message) {
  return Object.freeze({ code, path, message });
}

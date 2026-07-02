import { KIND } from "../../protocol.js";
import { TANK_RIG_SVG } from "./tank_svg.js";
import { compileSvgRig } from "./svg_importer.js";

const CANDIDATE_ID_RE = /^[A-Za-z0-9_-]{1,64}$/;

export const VISUAL_UNIT_RIG_CANDIDATE_SOURCES = Object.freeze([
  Object.freeze({
    id: "tank-low-profile",
    label: "Low profile tank",
    kind: KIND.TANK,
    svgText: tankLowProfileSvg(),
  }),
  Object.freeze({
    id: "tank-wide-turret",
    label: "Wide turret tank",
    kind: KIND.TANK,
    svgText: tankWideTurretSvg(),
  }),
  Object.freeze({
    id: "tank-long-cannon",
    label: "Long cannon tank",
    kind: KIND.TANK,
    svgText: tankLongCannonSvg(),
  }),
]);

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

function tankLowProfileSvg() {
  let svg = TANK_RIG_SVG;
  svg = replaceOnce(
    svg,
    'points="-23.2,-11.4 19.2,-11.4 25.2,-7.4 25.2,7.4 19.2,11.4 -23.2,11.4 -25.2,7.4 -25.2,-7.4"',
    'points="-26,-9.2 18,-9.2 27,-5.8 27,5.8 18,9.2 -26,9.2 -29,5.8 -29,-5.8"',
  );
  svg = replaceOnce(
    svg,
    'x="-8.072" y="-6.48" width="18.144" height="12.96"',
    'x="-10.5" y="-5.4" width="22" height="10.8"',
  );
  svg = replaceOnce(
    svg,
    'x="14.7" y="-9.72" width="7" height="19.44"',
    'x="17.8" y="-7.8" width="7.4" height="15.6"',
  );
  return svg;
}

function tankWideTurretSvg() {
  let svg = TANK_RIG_SVG;
  svg = replaceOnce(
    svg,
    'x="-8.072" y="-6.48" width="18.144" height="12.96"',
    'x="-12.6" y="-7.5" width="26.4" height="15"',
  );
  svg = replaceOnce(
    svg,
    'x="-16.49" y="-5.904" width="28.98" height="11.808"',
    'x="-19" y="-6.9" width="34" height="13.8"',
  );
  svg = replaceOnce(
    svg,
    'stroke="#241d17" stroke-width="5"',
    'stroke="#241d17" stroke-width="6.2"',
  );
  return svg;
}

function tankLongCannonSvg() {
  return TANK_RIG_SVG
    .replaceAll("33.2", "39.2")
    .replaceAll("weaponFacingCos:transform.x:42:0", "weaponFacingCos:transform.x:48:0")
    .replaceAll("weaponFacingSin:transform.y:42:0", "weaponFacingSin:transform.y:48:0");
}

function replaceOnce(source, needle, replacement) {
  return source.includes(needle) ? source.replace(needle, replacement) : source;
}

function candidateError(code, path, message) {
  return Object.freeze({ code, path, message });
}

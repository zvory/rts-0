export const AUTHORING_GROUND_CHARACTERS = new Set([".", ..."0123456789"]);
export const AUTHORING_FEATURE_CHARACTERS = new Set(["#", "~", "=", "-", "|", "\\", "/"]);
export const AUTHORING_NO_FEATURE = ".";
export const AUTHORING_DEFAULT_GROUND = ".";

export function isGroundCharacter(character) {
  return AUTHORING_GROUND_CHARACTERS.has(character);
}

export function isFeatureCharacter(character) {
  return AUTHORING_FEATURE_CHARACTERS.has(character);
}

/** Split the legacy one-character terrain grid without changing its composed appearance. */
export function splitTerrainRows(rows, width, height) {
  validateRows(rows, width, height, "terrain");
  const ground = [];
  const features = [];
  for (const row of rows) {
    const groundRow = [];
    const featureRow = [];
    for (const source of row) {
      if (isGroundCharacter(source)) {
        groundRow.push(source);
        featureRow.push(AUTHORING_NO_FEATURE);
      } else if (isFeatureCharacter(source)) {
        groundRow.push(AUTHORING_DEFAULT_GROUND);
        featureRow.push(source);
      } else {
        groundRow.push(AUTHORING_DEFAULT_GROUND);
        featureRow.push(AUTHORING_NO_FEATURE);
      }
    }
    ground.push(groundRow.join(""));
    features.push(featureRow.join(""));
  }
  return { ground, features };
}

/** Recompose the editor-only split into the authoritative legacy terrain contract. */
export function composeTerrainRows(draft) {
  if (Array.isArray(draft?.terrain)) return [...draft.terrain];
  const width = Number(draft?.width);
  const height = Number(draft?.height);
  validateRows(draft?.ground, width, height, "ground");
  validateRows(draft?.features, width, height, "features");
  return draft.ground.map((groundRow, y) => [...groundRow].map((ground, x) => {
    const feature = draft.features[y][x];
    return isFeatureCharacter(feature) ? feature : ground;
  }).join(""));
}

export function terrainCharacterAt(draft, x, y) {
  if (Array.isArray(draft?.terrain)) return draft.terrain[y]?.[x];
  const feature = draft?.features?.[y]?.[x];
  return isFeatureCharacter(feature) ? feature : draft?.ground?.[y]?.[x];
}

/**
 * Mutate one editor terrain layer. Legacy authoring callers retain replacement semantics;
 * split Map Editor drafts keep cosmetic ground separate from meaningful features.
 */
export function setTerrainCharacter(draft, x, y, character, { eraseFeature = false } = {}) {
  if (Array.isArray(draft?.terrain)) {
    const row = [...draft.terrain[y]];
    if (row[x] === character) return null;
    row[x] = character;
    draft.terrain[y] = row.join("");
    return character;
  }
  if (eraseFeature) {
    const row = [...draft.features[y]];
    if (!isFeatureCharacter(row[x])) return null;
    row[x] = AUTHORING_NO_FEATURE;
    draft.features[y] = row.join("");
    return terrainCharacterAt(draft, x, y);
  }
  if (isGroundCharacter(character)) {
    if (isFeatureCharacter(draft.features[y][x])) return null;
    const row = [...draft.ground[y]];
    if (row[x] === character) return null;
    row[x] = character;
    draft.ground[y] = row.join("");
    return terrainCharacterAt(draft, x, y);
  }
  if (isFeatureCharacter(character)) {
    const featureRow = [...draft.features[y]];
    if (featureRow[x] === character) return null;
    featureRow[x] = character;
    draft.features[y] = featureRow.join("");
    // Legacy authored maps cannot persist a hidden cosmetic underlay. Reset it now so export and
    // re-import cannot silently lose editor state; a future layered schema can lift this boundary.
    const groundRow = [...draft.ground[y]];
    groundRow[x] = AUTHORING_DEFAULT_GROUND;
    draft.ground[y] = groundRow.join("");
    return character;
  }
  return null;
}

export function validateTerrainLayers(draft, width, height) {
  validateRows(draft?.ground, width, height, "ground");
  validateRows(draft?.features, width, height, "features");
  draft.ground = draft.ground.map((row) => [...row]
    .map((character) => isGroundCharacter(character) ? character : AUTHORING_DEFAULT_GROUND)
    .join(""));
  draft.features = draft.features.map((row) => [...row]
    .map((character) => isFeatureCharacter(character) ? character : AUTHORING_NO_FEATURE)
    .join(""));
}

function validateRows(rows, width, height, label) {
  if (!Number.isInteger(width) || !Number.isInteger(height) || !Array.isArray(rows)
    || rows.length !== height
    || rows.some((row) => typeof row !== "string" || [...row].length !== width)) {
    throw new Error(`Map ${label} rows must match its width and height.`);
  }
}

export const LAB_SCENARIO_AUTHORING_LIMITS = Object.freeze({
  slug: 48,
  name: 80,
  title: 96,
  description: 320,
  tags: 8,
  tag: 32,
});

export function createLabScenarioAuthoringState({ defaultName = "Untitled lab setup" } = {}) {
  const name = cleanAuthoringText(defaultName) || "Untitled lab setup";
  return {
    name,
    title: name,
    slug: slugifyLabScenario(name),
    description: "",
    tags: "",
    scenarioJson: "",
  };
}

export function slugifyLabScenario(value) {
  const slug = String(value || "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, LAB_SCENARIO_AUTHORING_LIMITS.slug)
    .replace(/-+$/g, "");
  return slug || "lab-setup";
}

export function validateLabScenarioAuthoringState(state) {
  const name = cleanAuthoringText(state?.name);
  const title = cleanAuthoringText(state?.title);
  const slug = cleanAuthoringText(state?.slug);
  const description = cleanAuthoringText(state?.description);
  const { tags, errors: tagErrors } = parseLabScenarioTags(state?.tags);
  const errors = [];

  if (!isSafeCatalogId(slug)) {
    errors.push(`Slug must be 1-${LAB_SCENARIO_AUTHORING_LIMITS.slug} ASCII letters, numbers, hyphens, or underscores.`);
  }
  if (!name || name.length > LAB_SCENARIO_AUTHORING_LIMITS.name) {
    errors.push(`Name must be 1-${LAB_SCENARIO_AUTHORING_LIMITS.name} bytes.`);
  }
  if (!title || title.length > LAB_SCENARIO_AUTHORING_LIMITS.title) {
    errors.push(`Title must be 1-${LAB_SCENARIO_AUTHORING_LIMITS.title} bytes.`);
  }
  if (!description || description.length > LAB_SCENARIO_AUTHORING_LIMITS.description) {
    errors.push(`Description must be 1-${LAB_SCENARIO_AUTHORING_LIMITS.description} bytes.`);
  }
  errors.push(...tagErrors);

  const metadata = {
    slug,
    name,
    title,
    description,
    tags,
  };
  return { ok: errors.length === 0, errors, metadata };
}

export function parseLabScenarioTags(value) {
  const raw = String(value || "");
  const tags = raw.split(",").map(cleanAuthoringText).filter(Boolean);
  const errors = [];
  if (tags.length > LAB_SCENARIO_AUTHORING_LIMITS.tags) {
    errors.push(`Use at most ${LAB_SCENARIO_AUTHORING_LIMITS.tags} tags.`);
  }
  for (const tag of tags) {
    if (!isSafeCatalogTag(tag)) {
      errors.push(`Tag "${tag}" must be 1-${LAB_SCENARIO_AUTHORING_LIMITS.tag} ASCII letters, numbers, hyphens, or underscores.`);
    }
  }
  return { tags, errors };
}

export function labScenarioPreviewLabel(preview) {
  const scenarioPath = cleanAuthoringText(preview?.scenarioPath);
  const entry = preview?.manifestEntry || {};
  const id = cleanAuthoringText(entry.id || preview?.slug);
  const title = cleanAuthoringText(entry.title);
  if (!scenarioPath) return "";
  return title ? `${title} (${id}) -> ${scenarioPath}` : `${id} -> ${scenarioPath}`;
}

function cleanAuthoringText(value) {
  return String(value || "").trim();
}

function isSafeCatalogId(value) {
  const text = String(value || "");
  return text.length > 0 &&
    text.length <= LAB_SCENARIO_AUTHORING_LIMITS.slug &&
    /^[A-Za-z0-9_-]+$/.test(text);
}

function isSafeCatalogTag(value) {
  const text = String(value || "");
  return text.length > 0 &&
    text.length <= LAB_SCENARIO_AUTHORING_LIMITS.tag &&
    /^[A-Za-z0-9_-]+$/.test(text);
}

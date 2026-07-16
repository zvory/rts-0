import {
  LAB_SCENARIO_AUTHORING_LIMITS,
  labScenarioPreviewLabel,
  slugifyLabScenario,
  validateLabScenarioAuthoringState,
} from "./lab_scenario_authoring.js";

const AUTHORING_FIELD_IDS = Object.freeze([
  ["name", "scenario-name"],
  ["title", "scenario-title"],
  ["slug", "scenario-slug"],
  ["tags", "scenario-tags"],
  ["description", "scenario-description"],
  ["scenarioJson", "scenario-json"],
]);

export function updateLabScenarioTitle(panel, value) {
  panel.authoring.title = value;
  if (panel.authoringSlugEdited) return;
  panel.authoring.slug = slugifyLabScenario(value);
  const slugField = panel.fields.get("scenario-slug");
  if (slugField) slugField.value = panel.authoring.slug;
}

export function captureLabScenarioAuthoringFields(panel) {
  for (const [key, id] of AUTHORING_FIELD_IDS) panel.authoring[key] = panel.value(id);
}

export function renderLabScenarioAuthoringFeedback(panel) {
  const errors = panel.authoringValidation.errors || [];
  const label = errors.length ? errors.join(" ") : labScenarioPreviewLabel(panel.authoringValidation.preview);
  const node = panel.readout(label);
  node.className = "lab-readout lab-authoring-feedback";
  node.dataset.state = errors.length ? "error" : (label ? "ok" : "idle");
  return node;
}

export function renderLabScenarioOptions(panel) {
  const limits = LAB_SCENARIO_AUTHORING_LIMITS;
  const setAuthoring = (key) => (value) => {
    panel.authoring[key] = value;
  };
  return [
    panel.fieldset("Checkpoint Setup", [
      panel.inputField("scenario-name", "Name", "text", panel.authoring.name, {
        maxLength: limits.name,
        onChange: (value) => { panel.authoring.name = value; if (!panel.authoring.title) panel.authoring.title = value; },
      }),
      panel.inputField("scenario-title", "Title", "text", panel.authoring.title, {
        maxLength: limits.title,
        onChange: (value) => updateLabScenarioTitle(panel, value),
      }),
      panel.inputField("scenario-slug", "Slug", "text", panel.authoring.slug, {
        maxLength: limits.slug,
        onChange: (value) => { panel.authoring.slug = value; panel.authoringSlugEdited = true; },
      }),
      panel.inputField("scenario-tags", "Tags", "text", panel.authoring.tags, {
        maxLength: (limits.tag + 1) * limits.tags,
        onChange: setAuthoring("tags"),
      }),
      panel.textAreaField("scenario-description", "Description", panel.authoring.description, {
        maxLength: limits.description,
        rows: 3,
        wide: true,
        onChange: setAuthoring("description"),
      }),
      panel.textAreaField("scenario-json", "Setup JSON", panel.authoring.scenarioJson, {
        rows: 7,
        wide: true,
        onChange: setAuthoring("scenarioJson"),
      }),
      renderLabScenarioAuthoringFeedback(panel),
      panel.button("Validate setup", () => validateLabScenario(panel)),
      panel.button("Export setup JSON", () => panel.exportScenario()),
      panel.button("Import setup JSON", () => panel.importScenario()),
      panel.button("Reset setup", () => panel.resetScenario()),
    ]),
    renderLabReplayOptions(panel),
  ];
}

export function renderLabReplayOptions(panel) {
  return panel.fieldset("Lab Replay", [
    panel.button("Save lab replay", () => panel.saveLabReplay(), {
      disabled: true,
      title: "Lab replay save uses the bounded replay-artifact path, not the setup JSON wire request.",
      dataset: { labReplayAction: "save" },
    }),
    panel.button("Open lab replay", () => panel.openLabReplay(), {
      disabled: true,
      title: "Lab replay open uses the bounded replay-artifact path, not the setup JSON wire request.",
      dataset: { labReplayAction: "open" },
    }),
  ]);
}

export async function validateLabScenario(panel) {
  captureLabScenarioAuthoringFields(panel);
  const validation = validateLabScenarioAuthoringState(panel.authoring);
  if (!validation.ok) {
    panel.authoringValidation = { errors: validation.errors, preview: null };
    return panel.publishLocalResult("validateScenario", false, validation.errors.join(" "));
  }
  panel.authoringValidation = { errors: [], preview: null };
  const result = await panel.labClient.validateScenario(validation.metadata);
  const preview = result?.ok ? (result.outcome?.preview || null) : null;
  panel.authoringValidation = result?.ok
    ? { errors: [], preview }
    : { errors: [result?.error || "Setup validation failed."], preview: null };
  applyScenarioPreview(panel, preview);
  panel.render();
  return result;
}

function applyScenarioPreview(panel, preview) {
  if (typeof preview?.scenarioJson !== "string") return;
  panel.authoring.scenarioJson = preview.scenarioJson;
  const field = panel.fields.get("scenario-json");
  if (field) field.value = preview.scenarioJson;
}

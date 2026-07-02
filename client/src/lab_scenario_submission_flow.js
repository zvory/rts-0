import {
  LAB_SCENARIO_AUTHORING_LIMITS,
  labScenarioPreviewLabel,
  slugifyLabScenario,
  validateLabScenarioAuthoringState,
} from "./lab_scenario_authoring.js";

const LAB_SCENARIO_SUBMISSION_TIMEOUT_MS = 120000;
const AUTHORING_FIELD_IDS = Object.freeze([
  ["name", "scenario-name"],
  ["title", "scenario-title"],
  ["slug", "scenario-slug"],
  ["tags", "scenario-tags"],
  ["description", "scenario-description"],
  ["reviewNotes", "scenario-review-notes"],
  ["scenarioJson", "scenario-json"],
]);
const DEFAULT_SUBMISSION_CAPABILITY = Object.freeze({
  available: false,
  unavailableCode: "unavailable",
  unavailableReason: "Setup PR submission is unavailable.",
  branchPrefix: "zvorygin/lab-scenario-",
  scenarioPathPrefix: "server/assets/lab-scenarios/",
  manifestPath: "server/assets/lab-scenarios/manifest.json",
});

export function createLabScenarioSubmissionState() {
  return {
    capability: null,
    capabilityPending: false,
    pending: false,
    pendingPromise: null,
    message: "",
    result: null,
  };
}

export function defaultLabScenarioSubmissionWindow(url) {
  const opener = globalThis.window?.open || globalThis.open;
  if (typeof opener !== "function") return null;
  return opener.call(globalThis.window || globalThis, url, "_blank", "noopener,noreferrer");
}

export function setLabScenarioSubmissionCapability(panel, source) {
  if (source && typeof source.then === "function") {
    panel.submission.capability = null;
    panel.submission.capabilityPending = true;
    source.then((capability) => {
      if (panel.destroyed) return;
      panel.submission.capability = normalizeSubmissionCapability(capability);
      panel.submission.capabilityPending = false;
      panel.render();
    }).catch((err) => {
      if (panel.destroyed) return;
      panel.submission.capability = normalizeSubmissionCapability({
        available: false,
        unavailableCode: "capabilityCheckFailed",
        unavailableReason: `Setup PR submission availability could not be checked: ${err?.message || err}`,
      });
      panel.submission.capabilityPending = false;
      panel.render();
    });
    return;
  }
  panel.submission.capability = normalizeSubmissionCapability(source);
  panel.submission.capabilityPending = false;
}

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
      panel.textAreaField("scenario-review-notes", "Review notes", panel.authoring.reviewNotes, {
        maxLength: limits.reviewNotes,
        rows: 3,
        wide: true,
        onChange: setAuthoring("reviewNotes"),
      }),
      panel.textAreaField("scenario-json", "Setup JSON", panel.authoring.scenarioJson, {
        rows: 7,
        wide: true,
        onChange: setAuthoring("scenarioJson"),
      }),
      renderLabScenarioAuthoringFeedback(panel),
      renderLabScenarioSubmissionFeedback(panel),
      panel.button("Validate setup", () => validateLabScenario(panel)),
      panel.button("Submit setup PR", () => submitLabScenario(panel), {
        disabled: !!labScenarioSubmissionDisabledReason(panel),
        title: labScenarioSubmissionDisabledReason(panel) || "Validate and submit this checkpoint-backed lab setup as a draft pull request",
        dataset: {
          scenarioSubmit: "true",
          pending: panel.submission.pending ? "true" : "false",
        },
      }),
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

export function renderLabScenarioSubmissionFeedback(panel) {
  const wrap = document.createElement("div");
  wrap.className = "lab-submission-feedback";
  wrap.dataset.state = submissionFeedbackState(panel);

  const node = panel.readout(submissionFeedbackText(panel));
  node.className = "lab-readout lab-submission-status";
  wrap.appendChild(node);

  const prUrl = panel.submission.result?.ok ? panel.submission.result.prUrl : "";
  if (prUrl) {
    const linkRow = document.createElement("div");
    linkRow.className = "lab-submission-link";
    const link = document.createElement("a");
    link.href = prUrl;
    link.target = "_blank";
    link.rel = "noopener noreferrer";
    link.textContent = prUrl;
    const copy = document.createElement("input");
    copy.type = "text";
    copy.readOnly = true;
    copy.value = prUrl;
    copy.setAttribute("aria-label", "Draft PR link");
    panel.fields.set("scenario-pr-link", copy);
    linkRow.append(link, copy);
    wrap.appendChild(linkRow);
  }

  return wrap;
}

export function labScenarioSubmissionDisabledReason(panel) {
  if (panel.submission.pending) return "Setup PR submission is already running.";
  if (panel.submission.capabilityPending) return "Setup PR submission availability is still loading.";
  const capability = submissionCapability(panel);
  if (!capability.available) return capability.unavailableReason || "Setup PR submission is not configured.";
  return "";
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

export function submitLabScenario(panel) {
  if (panel.submission.pending) return panel.submission.pendingPromise || Promise.resolve(panel.lastResult);
  captureLabScenarioAuthoringFields(panel);
  const validation = validateLabScenarioAuthoringState(panel.authoring);
  if (!validation.ok) {
    panel.authoringValidation = { errors: validation.errors, preview: null };
    panel.submission.result = { ok: false, error: validation.errors.join(" ") };
    return panel.publishLocalResult("submitScenario", false, validation.errors.join(" "));
  }
  const disabledReason = labScenarioSubmissionDisabledReason(panel);
  if (disabledReason) {
    panel.submission.result = {
      ok: false,
      error: `Setup PR submission unavailable: ${disabledReason}`,
    };
    return panel.publishLocalResult("submitScenario", false, panel.submission.result.error);
  }

  panel.submission.pending = true;
  panel.submission.pendingPromise = runLabScenarioSubmission(panel, validation.metadata);
  panel.submission.message = "Validating setup before submission...";
  panel.submission.result = null;
  panel.authoringValidation = { errors: [], preview: null };
  panel.render();
  return panel.submission.pendingPromise;
}

export function destroyLabScenarioSubmission(panel) {
  panel.submission.pending = false;
  panel.submission.pendingPromise = null;
}

async function runLabScenarioSubmission(panel, metadata) {
  try {
    const validationResult = await panel.labClient.validateScenario(metadata);
    if (panel.destroyed) return validationResult;
    const preview = validationResult?.ok ? (validationResult.outcome?.preview || null) : null;
    if (!validationResult?.ok) {
      const error = validationResult?.error || "Setup validation failed.";
      panel.authoringValidation = { errors: [error], preview: null };
      panel.submission.result = { ok: false, error };
      return validationResult;
    }
    panel.authoringValidation = { errors: [], preview };
    applyScenarioPreview(panel, preview);
    panel.submission.message = "Submitting draft PR...";
    panel.render();

    const result = await panel.labClient.submitScenario(metadata, {
      timeoutMs: LAB_SCENARIO_SUBMISSION_TIMEOUT_MS,
    });
    if (panel.destroyed) return result;
    if (result?.ok) {
      const prUrl = String(result.outcome?.prUrl || "");
      panel.submission.result = {
        ok: true,
        prUrl,
        branchName: result.outcome?.branchName || "",
        scenarioPath: result.outcome?.scenarioPath || "",
        manifestPath: result.outcome?.manifestPath || "",
      };
      if (prUrl) openSubmissionPr(panel, prUrl);
    } else {
      const code = result?.outcome?.code ? `[${result.outcome.code}] ` : "";
      panel.submission.result = {
        ok: false,
        error: `${code}${result?.error || "Setup PR submission failed."}`,
      };
    }
    return result;
  } finally {
    if (!panel.destroyed) {
      panel.submission.pending = false;
      panel.submission.pendingPromise = null;
      panel.submission.message = "";
      panel.render();
    }
  }
}

function applyScenarioPreview(panel, preview) {
  if (typeof preview?.scenarioJson !== "string") return;
  panel.authoring.scenarioJson = preview.scenarioJson;
  const field = panel.fields.get("scenario-json");
  if (field) field.value = preview.scenarioJson;
}

function submissionCapability(panel) {
  return panel.submission.capability || DEFAULT_SUBMISSION_CAPABILITY;
}

function submissionFeedbackState(panel) {
  if (panel.submission.pending) return "pending";
  if (panel.submission.result?.ok) return "ok";
  if (panel.submission.result && !panel.submission.result.ok) return "error";
  if (panel.submission.capabilityPending) return "pending";
  return submissionCapability(panel).available ? "ready" : "disabled";
}

function submissionFeedbackText(panel) {
  if (panel.submission.pending) return panel.submission.message || "Submitting setup PR...";
  if (panel.submission.result?.ok) return "Draft PR created. The link is available below.";
  if (panel.submission.result && !panel.submission.result.ok) {
    const error = panel.submission.result.error || "Setup PR submission failed.";
    return `${error} Export setup JSON remains available.`;
  }
  if (panel.submission.capabilityPending) return "Checking setup PR submission availability...";
  const capability = submissionCapability(panel);
  if (capability.available) return `Setup PR submission enabled for ${capability.scenarioPathPrefix}`;
  const reason = capability.unavailableReason || "backend is not configured";
  return `Setup PR submission disabled: ${reason} Export setup JSON remains available.`;
}

function openSubmissionPr(panel, prUrl) {
  try {
    panel.openWindow?.(prUrl);
  } catch (err) {
    panel.submission.result = {
      ...(panel.submission.result || {}),
      ok: true,
      prUrl,
      openError: String(err?.message || err),
    };
  }
}

function normalizeSubmissionCapability(value) {
  const source = value && typeof value === "object" ? value : {};
  return {
    available: !!source.available,
    unavailableCode: cleanString(source.unavailableCode ?? source.unavailable_code) ||
      DEFAULT_SUBMISSION_CAPABILITY.unavailableCode,
    unavailableReason: cleanString(source.unavailableReason ?? source.unavailable_reason) ||
      (source.available ? "" : DEFAULT_SUBMISSION_CAPABILITY.unavailableReason),
    branchPrefix: cleanString(source.branchPrefix ?? source.branch_prefix) ||
      DEFAULT_SUBMISSION_CAPABILITY.branchPrefix,
    scenarioPathPrefix: cleanString(source.scenarioPathPrefix ?? source.scenario_path_prefix) ||
      DEFAULT_SUBMISSION_CAPABILITY.scenarioPathPrefix,
    manifestPath: cleanString(source.manifestPath ?? source.manifest_path) ||
      DEFAULT_SUBMISSION_CAPABILITY.manifestPath,
  };
}

function cleanString(value) {
  return String(value || "").trim();
}

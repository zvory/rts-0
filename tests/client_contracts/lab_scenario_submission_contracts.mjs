// Lab scenario authoring and submission-capability contracts.

import {
  assert,
  assertDeepEqual,
} from "./assertions.mjs";
import {
  slugifyLabScenario,
  validateLabScenarioAuthoringState,
} from "../../client/src/lab_scenario_authoring.js";
import {
  LAB_SCENARIO_SUBMISSION_CAPABILITY_PATH,
  fetchLabScenarioSubmissionCapability,
} from "../../client/src/lab_scenario_submission_capability.js";

{
  assert(slugifyLabScenario("Two Player Test!") === "two-player-test", "lab setup authoring generates stable slugs from titles");
  const valid = validateLabScenarioAuthoringState({
    slug: "two-player-test",
    name: "Two Player Test",
    title: "Two Player Test",
    description: "Small deterministic setup.",
    tags: "two-player, test",
    reviewNotes: "ready for review",
  });
  assert(valid.ok && valid.metadata.tags.length === 2, "lab setup authoring accepts catalog-ready metadata");
  const invalid = validateLabScenarioAuthoringState({
    slug: "bad slug",
    name: "",
    title: "Bad",
    description: "",
    tags: "bad tag",
  });
  assert(
    !invalid.ok &&
      invalid.errors.some((error) => error.includes("Slug")) &&
      invalid.errors.some((error) => error.includes("Name")) &&
      invalid.errors.some((error) => error.includes("Tag")),
    "lab setup authoring reports blocking metadata errors before server validation",
  );
}

{
  const requests = [];
  const sleeps = [];
  const result = await fetchLabScenarioSubmissionCapability({
    retryDelaysMs: [7, 11],
    sleep: async (ms) => { sleeps.push(ms); },
    fetchImpl: async (url, options) => {
      requests.push({ url, options });
      if (requests.length < 3) return { ok: false, status: 502 };
      return {
        ok: true,
        async json() {
          return {
            available: true,
            branchPrefix: "zvorygin/lab-scenario-",
            scenarioPathPrefix: "server/assets/lab-scenarios/",
            manifestPath: "server/assets/lab-scenarios/manifest.json",
          };
        },
      };
    },
  });
  assert(
    result.available &&
      requests.length === 3 &&
      requests.every((request) => (
        request.url === LAB_SCENARIO_SUBMISSION_CAPABILITY_PATH &&
        request.options.cache === "no-store"
      )),
    "lab setup submission capability probe retries transient 502s before disabling PR submission",
  );
  assertDeepEqual(
    sleeps,
    [7, 11],
    "lab setup submission capability probe uses configured retry delays",
  );
}

{
  let requests = 0;
  const result = await fetchLabScenarioSubmissionCapability({
    retryDelaysMs: [7, 11],
    sleep: async () => { throw new Error("404 must not be retried"); },
    fetchImpl: async () => {
      requests += 1;
      return { ok: false, status: 404 };
    },
  });
  assert(
    requests === 1 &&
      !result.available &&
      result.unavailableCode === "capabilityCheckFailed" &&
      result.unavailableReason.includes("(404)"),
    "lab setup submission capability probe does not retry permanent HTTP failures",
  );
}

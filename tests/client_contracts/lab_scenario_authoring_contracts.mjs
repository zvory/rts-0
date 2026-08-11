// Lab scenario authoring contracts.

import { assert } from "./assertions.mjs";
import {
  slugifyLabScenario,
  validateLabScenarioAuthoringState,
} from "../../client/src/lab_scenario_authoring.js";

assert(
  slugifyLabScenario("Two Player Test!") === "two-player-test",
  "lab setup authoring generates stable slugs from titles",
);

const valid = validateLabScenarioAuthoringState({
  slug: "two-player-test",
  name: "Two Player Test",
  title: "Two Player Test",
  description: "Small deterministic setup.",
});
assert(valid.ok && !("tags" in valid.metadata), "lab setup authoring metadata has no tag concept");

const invalid = validateLabScenarioAuthoringState({
  slug: "bad slug",
  name: "",
  title: "Bad",
  description: "",
});
assert(
  !invalid.ok &&
    invalid.errors.some((error) => error.includes("Slug")) &&
    invalid.errors.some((error) => error.includes("Name")),
  "lab setup authoring reports blocking metadata errors before server validation",
);

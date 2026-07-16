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
  tags: "two-player, test",
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

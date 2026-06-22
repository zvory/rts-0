// tests/client_contracts/map_editor_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import fs from "node:fs";
import { assert } from "./assertions.mjs";

{
  const editorHtml = fs.readFileSync(new URL("../../client/map-editor.html", import.meta.url), "utf8");
  assert(!editorHtml.includes('data-view="atlas"'), "map editor does not expose an Atlas tab");
  assert(!editorHtml.includes('MAP_ATLAS_URL'), "map editor does not request atlas diagnostics");
  assert(!editorHtml.includes("atlas-readout"), "map editor does not include atlas controls");
}

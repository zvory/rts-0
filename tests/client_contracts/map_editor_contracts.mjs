// tests/client_contracts/map_editor_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import fs from "node:fs";
import vm from "node:vm";
import { assert } from "./assertions.mjs";

{
  const editorHtml = fs.readFileSync(new URL("../../client/map-editor.html", import.meta.url), "utf8");
  assert(!editorHtml.includes('data-view="atlas"'), "map editor does not expose an Atlas tab");
  assert(!editorHtml.includes('MAP_ATLAS_URL'), "map editor does not request atlas diagnostics");
  assert(!editorHtml.includes("atlas-readout"), "map editor does not include atlas controls");
  assert(editorHtml.includes('data-symmetry="left-right"'), "map editor exposes left-right symmetry");
  assert(editorHtml.includes('data-symmetry="top-bottom"'), "map editor exposes top-bottom symmetry");
  assert(editorHtml.includes('data-symmetry="radial"'), "map editor exposes 180-degree rotational symmetry");
  assert(editorHtml.includes('const NATURALS_PER_SLOT = 3'), "map editor allows a main plus three naturals per player slot");
  assert(editorHtml.includes("function applyTerrain(tiles, ch)"), "map editor mirrors terrain changes through one terrain application path");
  assert(editorHtml.includes("function syncMirroredSlotsForSite(siteId)"), "map editor keeps mirrored base slots in sync");
}

function mapEditorRuntime() {
  const editorHtml = fs.readFileSync(new URL("../../client/map-editor.html", import.meta.url), "utf8");
  const script = editorHtml.match(/<script>\s*([\s\S]*?)<\/script>/)?.[1] || "";
  const eventWiring = script.indexOf('      document.getElementById("tools").addEventListener');
  assert(eventWiring > 0, "map editor script has a testable pre-event initialization section");

  const elements = new Map();
  const context = vm.createContext({
    document: {
      getElementById(id) {
        if (!elements.has(id)) {
          elements.set(id, {
            addEventListener() {},
            appendChild() {},
            classList: { toggle() {} },
            getContext() {
              return new Proxy({}, { get: () => () => {} });
            },
            replaceChildren() {},
          });
        }
        return elements.get(id);
      },
      querySelectorAll() {
        return [];
      },
    },
  });
  vm.runInContext(
    script.slice(0, eventWiring) + `
      globalThis.__mapEditor = {
        applyTerrain,
        currentMap: () => map,
        replaceMap: (next) => { map = next; },
        selectSiteId: (id) => { selectedSiteId = id; },
        setMap: (next) => { map = normalizeMap(next); },
        setSelectedSite,
        setSymmetry: (mode) => { selectedSymmetry = mode; },
      };
    `,
    context,
  );
  return context.__mapEditor;
}

function blankTerrainMap(size = 40) {
  return {
    terrain: Array.from({ length: size }, () => ".".repeat(size)),
    sites: [],
  };
}

{
  const editor = mapEditorRuntime();
  const terrainChecks = [
    ["left-right", { x: 4, y: 5 }, { x: 35, y: 5 }],
    ["top-bottom", { x: 4, y: 5 }, { x: 4, y: 34 }],
    ["radial", { x: 4, y: 5 }, { x: 35, y: 34 }],
  ];
  for (const [mode, source, mirrored] of terrainChecks) {
    editor.replaceMap(blankTerrainMap());
    editor.setSymmetry(mode);
    assert(editor.applyTerrain([source], "#"), mode + " symmetry accepts an unprotected terrain edit");
    const map = editor.currentMap();
    assert(map.terrain[source.y][source.x] === "#", mode + " symmetry keeps the drawn terrain tile");
    assert(map.terrain[mirrored.y][mirrored.x] === "#", mode + " symmetry writes the reflected terrain tile");
  }
}

{
  const editor = mapEditorRuntime();
  const baseModes = [
    ["left-right", { x: 7, y: 8 }, { x: 48, y: 8 }],
    ["top-bottom", { x: 8, y: 7 }, { x: 8, y: 48 }],
    ["radial", { x: 7, y: 8 }, { x: 48, y: 47 }],
  ];
  for (const [mode, destination, mirroredDestination] of baseModes) {
    editor.setMap({
      version: 2,
      name: "symmetric-slot",
      description: "test",
      _design: "test",
      terrain: Array.from({ length: 56 }, () => ".".repeat(56)),
      sites: [
        { id: "main_left", kind: "main", x: 8, y: 8 },
        { id: "natural_a", kind: "natural", x: 16, y: 8 },
        { id: "natural_b", kind: "natural", x: 16, y: 20 },
        { id: "natural_c", kind: "natural", x: 16, y: 32 },
      ],
      layouts: [
        { id: "one", playerCount: 1, slots: [{ main: "main_left", naturals: ["natural_a", "natural_b", "natural_c"] }] },
      ],
    });
    editor.setSymmetry(mode);
    editor.selectSiteId("main_left");
    assert(editor.setSelectedSite(destination), mode + " symmetric base placement succeeds");

    const map = editor.currentMap();
    const mirrorMain = map.sites.find((site) => site.id === "main_left_mirror");
    const layout = map.layouts[0];
    assert(
      mirrorMain?.x === mirroredDestination.x && mirrorMain.y === mirroredDestination.y,
      mode + " symmetric base placement creates the reflected main",
    );
    assert(layout.playerCount === 2 && layout.slots.length === 2, mode + " symmetric base placement creates the matching player slot");
    assert(
      layout.slots.some((slot) => (
        slot.main === "main_left_mirror" &&
        slot.naturals.length === 3 &&
        slot.naturals.join(",") === "natural_a_mirror,natural_b_mirror,natural_c_mirror"
      )),
      mode + " matching player slot keeps all four mirrored bases together",
    );
  }
}

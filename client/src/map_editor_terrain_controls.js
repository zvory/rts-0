import { MAP_EDITOR_FEATURE_PALETTE, MAP_EDITOR_GROUND_PALETTE } from "./map_editor_terrain_palette.js";

export function createMapEditorTerrainPalettes(panel, { button, terrainName }) {
  const groundPalette = document.createElement("div");
  groundPalette.className = "map-editor-palette";
  const featurePalette = document.createElement("div");
  featurePalette.className = "map-editor-palette";
  for (const [layer, palette, entries] of [
    ["ground", groundPalette, MAP_EDITOR_GROUND_PALETTE],
    ["feature", featurePalette, MAP_EDITOR_FEATURE_PALETTE],
  ]) {
    for (const [code, label] of entries) {
      const control = button(label, () => panel.selectTerrainMaterial(code, layer), {
        active: panel.terrainContent === "material"
          && panel.selectedTerrain === code
          && panel.selectedTerrainLayer === layer
          && !(layer === "feature" && panel.semanticFeatureEraseSelected()),
      });
      control.dataset.terrain = terrainName(code);
      control.dataset.terrainLayer = layer;
      control.classList.add("map-editor-terrain-button");
      const preview = panel.viewport.createTerrainPreview?.(code);
      if (preview) {
        preview.className = "map-editor-terrain-icon";
        preview.setAttribute("aria-hidden", "true");
        control.prepend(preview);
      }
      palette.appendChild(control);
    }
  }
  const eraseControl = button("Erase", () => panel.selectSemanticFeatureErase(), {
    active: panel.semanticFeatureEraseSelected(),
    title: "Remove semantic terrain features to reveal the cosmetic ground underneath.",
  });
  eraseControl.dataset.terrain = "erase";
  eraseControl.dataset.terrainLayer = "feature";
  eraseControl.classList.add("map-editor-terrain-button");
  const eraseIcon = document.createElement("span");
  eraseIcon.className = "map-editor-erase-icon";
  eraseIcon.setAttribute("aria-hidden", "true");
  eraseControl.prepend(eraseIcon);
  featurePalette.insertBefore(eraseControl, featurePalette.children[2] || null);
  return { groundPalette, featurePalette };
}

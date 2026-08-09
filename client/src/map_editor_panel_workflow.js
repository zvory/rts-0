import { TERRAIN } from "./protocol.js";
import {
  MAP_EDITOR_DOODAD_CATALOG,
  isTreeDoodadType,
  isWildflowerDoodadType,
} from "./map_editor_doodads.js";
import {
  MAP_EDITOR_MAX_BASE_SITES,
  MAP_EDITOR_MAX_START_LOCATIONS,
} from "./map_editor_session.js";

export const MAP_EDITOR_CATEGORIES = Object.freeze([
  ["terrain", "Textures"],
  ["elevation", "Elevation"],
  ["objects", "Objects"],
  ["zones", "Zones"],
  ["locations", "Locations"],
]);

export const MAP_EDITOR_OPERATIONS = Object.freeze([
  ["brush", "Brush"],
  ["box", "Box"],
  ["path", "Path"],
  ["place", "Place"],
  ["spray", "Spray"],
  ["erase", "Erase"],
  ["add", "Add"],
  ["move", "Move"],
  ["remove", "Remove"],
]);

export function selectMapEditorCategoryState(panel, category) {
  panel.activeCategory = category;
  if (category === "elevation") panel.terrainContent = "elevation";
  else if (category === "terrain" && panel.terrainContent === "elevation") panel.terrainContent = "material";
  return panel.lastOperation[category === "elevation" ? "terrain" : category];
}

export function availableMapEditorOperations(panel) {
  if (panel.activeCategory === "elevation") return new Set(["brush", "box", "erase"]);
  if (panel.activeCategory === "objects") {
    const sprayable = isTreeDoodadType(panel.selectedDoodadType) || isWildflowerDoodadType(panel.selectedDoodadType);
    return new Set(sprayable ? ["place", "spray", "erase"] : ["place", "erase"]);
  }
  if (panel.activeCategory === "zones") {
    return new Set(panel.selectedOverlayEffects?.size ? ["brush", "box", "erase"] : []);
  }
  if (panel.activeCategory === "locations") {
    const selectionCount = panel.locationContent === "start"
      ? panel.session.draft?.startLocations?.length || 0
      : panel.session.mapOverlay()?.bases?.length || 0;
    const totalBaseCount = panel.session.draft?.baseSites?.length || 0;
    const selectionMax = panel.locationContent === "start" ? MAP_EDITOR_MAX_START_LOCATIONS : MAP_EDITOR_MAX_BASE_SITES;
    const operations = [];
    const canAdd = panel.locationContent === "start"
      ? selectionCount < selectionMax
      : selectionCount < selectionMax && totalBaseCount < MAP_EDITOR_MAX_BASE_SITES;
    if (canAdd) operations.push("add");
    if (selectionCount > 0) operations.push("move", "remove");
    return new Set(operations);
  }
  if (panel.terrainContent === "road") return new Set(["path", "erase"]);
  if (panel.terrainContent === "forest") return new Set(["brush", "erase"]);
  return new Set(["brush", "box", "erase"]);
}

export function activeMapEditorOperation(panel) {
  const tool = panel.viewport.tool;
  if (panel.activeCategory === "objects" && tool?.kind === "doodad") return tool.mode;
  if (panel.activeCategory === "zones" && tool?.kind === "overlay") {
    if (panel.overlayMode === "erase") return "erase";
    return tool.shape === "box" ? "box" : "brush";
  }
  if (panel.activeCategory === "locations" && ["start", "base"].includes(tool?.kind)) {
    return tool.add ? "add" : "move";
  }
  if (["terrain", "elevation"].includes(panel.activeCategory)) {
    if (tool?.kind === "road") return "path";
    if (tool?.kind === "forest") return tool.paint ? "brush" : "erase";
    if (tool?.kind === "elevation") {
      if (tool.level === 0 && panel.lastOperation.terrain === "erase") return "erase";
      return tool.shape === "box" ? "box" : "brush";
    }
    if (tool?.kind === "terrain") {
      if (tool.terrain === TERRAIN.GRASS && panel.lastOperation.terrain === "erase") return "erase";
      return tool.shape === "box" ? "box" : "brush";
    }
  }
  return null;
}

export function mapEditorOperationHelp(operation, category) {
  if (operation === "box") return "Drag to fill a rectangular area.";
  if (operation === "path") return "Drag a snapped road path.";
  if (operation === "spray") return "Hold and drag to scatter the selected objects.";
  if (operation === "erase") return "Remove editable content under the eraser.";
  if (operation === "place") return "Click to place one selected object.";
  if (operation === "add") return "Click to add a map location.";
  if (operation === "move") return "Click to move the selected map location.";
  if (operation === "remove") return "Remove the selected map location.";
  return category === "zones"
    ? "Hold and drag to paint the selected zone effects."
    : "Hold and drag to paint the selected content.";
}

export function mapEditorContentLabel(panel, overlayEffectName, terrainName) {
  const operation = activeMapEditorOperation(panel);
  if (operation === "erase" && panel.activeCategory === "objects") return "All objects";
  if (panel.activeCategory === "elevation") {
    return `Elevation level ${operation === "erase" ? 0 : panel.selectedElevation}`;
  }
  if (operation === "erase" && panel.activeCategory === "terrain" && panel.terrainContent !== "forest") {
    return "Terrain to grass";
  }
  if (panel.activeCategory === "objects") {
    return MAP_EDITOR_DOODAD_CATALOG.find(({ typeId }) => typeId === panel.selectedDoodadType)?.label || "Object";
  }
  if (panel.activeCategory === "zones") {
    return [...panel.selectedOverlayEffects].map(overlayEffectName).join(", ") || "No zones selected";
  }
  if (panel.activeCategory === "locations") return panel.locationContent === "base" ? "Neutral base" : "Player start";
  if (panel.terrainContent === "road") return "Automatic road";
  if (panel.terrainContent === "forest") return "Forest tile";
  return terrainName(panel.selectedTerrain).replaceAll("-", " ");
}

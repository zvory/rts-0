import {
  MAP_EDITOR_MAX_OIL_PATCHES,
  MAP_EDITOR_MAX_STEEL_PATCHES,
  updateSymmetricDraftBasePatchCount,
} from "./map_editor_session.js";

export function mapEditorLocationResourceState(panel, startBase, neutralBase) {
  const adding = panel.viewport.tool?.add === true && panel.viewport.tool?.kind === panel.locationContent;
  return {
    adding,
    resourceBase: adding
      ? { steelPatches: panel.pendingBaseSteelPatches, oilPatches: panel.pendingBaseOilPatches }
      : panel.locationContent === "start" ? startBase : neutralBase,
  };
}

export function armMapEditorLocation(panel, kind, locationIndex, add = false) {
  panel.viewport.armTool({
    kind,
    locationIndex,
    add,
    symmetry: panel.symmetry,
    ...(add ? {
      steelPatches: panel.pendingBaseSteelPatches,
      oilPatches: panel.pendingBaseOilPatches,
    } : {}),
  });
  panel.setStatus(`Click the map to ${add ? "add" : "move"} this ${kind === "start" ? "start location" : "base site"}.`);
}

export function updateMapEditorBasePatchCount(panel, baseIndex, fieldName, value) {
  let result = null;
  const changed = panel.session.mutate("Updated base resources", (draft) => {
    result = updateSymmetricDraftBasePatchCount(draft, {
      baseIndex,
      fieldName,
      value,
      symmetry: panel.symmetry,
    });
  });
  panel.setStatus(
    changed ? "Base resource counts updated." : result?.error || "Base resource count unchanged.",
    !changed && Boolean(result?.error),
  );
}

export function updateMapEditorPendingBasePatchCount(panel, fieldName, value) {
  const max = fieldName === "oilPatches" ? MAP_EDITOR_MAX_OIL_PATCHES : MAP_EDITOR_MAX_STEEL_PATCHES;
  const count = Math.max(0, Math.min(max, Math.trunc(Number(value)) || 0));
  if (fieldName === "oilPatches") panel.pendingBaseOilPatches = count;
  else panel.pendingBaseSteelPatches = count;
  if (panel.viewport.tool?.add && ["start", "base"].includes(panel.viewport.tool.kind)) {
    panel.viewport.armTool({ ...panel.viewport.tool, [fieldName]: count });
  }
  panel.setStatus("New base resource counts updated.");
}

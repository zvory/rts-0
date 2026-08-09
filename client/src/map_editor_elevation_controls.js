export function createMapEditorElevationTool(panel) {
  const label = document.createElement("label");
  label.className = "map-editor-field";
  const title = document.createElement("span");
  title.textContent = "Elevation";
  const controls = document.createElement("div");
  controls.className = "map-editor-palette";
  for (let level = 0; level <= 9; level += 1) {
    const control = document.createElement("button");
    control.type = "button";
    control.className = "map-editor-button";
    control.textContent = `Level ${level}`;
    control.dataset.active = panel.terrainContent === "elevation" && panel.selectedElevation === level ? "true" : "false";
    control.addEventListener("click", () => {
      selectMapEditorElevationLevel(panel, level);
    });
    controls.appendChild(control);
  }
  label.append(title, controls);
  return label;
}

export function selectMapEditorElevationLevel(panel, level) {
  panel.terrainContent = "elevation";
  panel.selectedElevation = level;
  const operation = level === 0
    ? "erase"
    : panel.lastOperation.terrain === "box" ? "box" : "brush";
  panel.selectOperation(operation);
}

export function armMapEditorElevation(panel, level = panel.selectedElevation) {
  panel.viewport.armTool({
    kind: "elevation",
    level,
    shape: panel.paintShape,
    symmetry: panel.symmetry,
  });
  panel.setStatus(`${panel.paintShape === "box" ? "Drag to fill a box" : "Paint"} with elevation level ${level}.`);
}

export function selectMapEditorElevationOperation(panel, operation) {
  panel.paintShape = operation === "box" ? "box" : "brush";
  armMapEditorElevation(panel, operation === "erase" ? 0 : panel.selectedElevation);
}

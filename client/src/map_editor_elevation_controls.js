export function createMapEditorElevationTool(panel) {
  const label = document.createElement("label");
  label.className = "map-editor-field";
  const title = document.createElement("span");
  title.textContent = "Paint level";
  const controls = document.createElement("div");
  controls.className = "map-editor-elevation-control";
  const range = elevationInput("range", panel.selectedElevation, "Elevation paint level slider");
  const number = elevationInput("number", panel.selectedElevation, "Elevation paint level");
  const select = (value) => {
    const level = boundedMapEditorElevationLevel(value);
    range.value = String(level);
    number.value = String(level);
    selectMapEditorElevationLevel(panel, level);
  };
  range.addEventListener("input", () => { number.value = range.value; });
  range.addEventListener("change", () => select(range.value));
  number.addEventListener("input", () => {
    const value = Number(number.value);
    if (Number.isFinite(value)) range.value = String(boundedMapEditorElevationLevel(value));
  });
  number.addEventListener("change", () => select(number.value));
  controls.append(range, number);
  label.append(title, controls);
  return label;
}

export function boundedMapEditorElevationLevel(value) {
  return Math.max(0, Math.min(9, Math.trunc(Number(value)) || 0));
}

export function selectMapEditorElevationLevel(panel, level) {
  panel.terrainContent = "elevation";
  panel.selectedElevation = boundedMapEditorElevationLevel(level);
  const operation = panel.selectedElevation === 0
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

function elevationInput(type, value, ariaLabel) {
  const input = document.createElement("input");
  input.type = type;
  input.min = "0";
  input.max = "9";
  input.step = "1";
  input.value = String(value);
  input.setAttribute("aria-label", ariaLabel);
  return input;
}

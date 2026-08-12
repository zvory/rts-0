const MAP_EDITOR_MIN_BRUSH_WIDTH = 1;
const MAP_EDITOR_MAX_BRUSH_WIDTH = 31;

export function createMapEditorBrushWidthInput(value, onChange, ariaLabel) {
  const input = document.createElement("input");
  input.type = "number";
  input.min = String(MAP_EDITOR_MIN_BRUSH_WIDTH);
  input.max = String(MAP_EDITOR_MAX_BRUSH_WIDTH);
  input.step = "1";
  input.value = String(value);
  input.setAttribute("aria-label", ariaLabel);
  input.addEventListener("change", () => {
    const numericValue = Math.trunc(Number(input.value)) || MAP_EDITOR_MIN_BRUSH_WIDTH;
    const next = Math.max(MAP_EDITOR_MIN_BRUSH_WIDTH, Math.min(MAP_EDITOR_MAX_BRUSH_WIDTH, numericValue));
    input.value = String(next);
    onChange(next);
  });
  return input;
}

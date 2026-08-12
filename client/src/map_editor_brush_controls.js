const MAP_EDITOR_MIN_BRUSH_WIDTH = 1;
const MAP_EDITOR_MAX_BRUSH_WIDTH = 31;

export function createMapEditorBrushWidthInput(value, onChange, ariaLabel) {
  return createMapEditorNumericInput(
    value,
    MAP_EDITOR_MIN_BRUSH_WIDTH,
    MAP_EDITOR_MAX_BRUSH_WIDTH,
    onChange,
    ariaLabel,
  );
}

export function createMapEditorNumericInput(value, min, max, onChange, ariaLabel) {
  const input = document.createElement("input");
  input.type = "number";
  input.min = String(min);
  input.max = String(max);
  input.step = "1";
  input.value = String(value);
  input.setAttribute("aria-label", ariaLabel);
  input.addEventListener("change", () => {
    const numericValue = Math.trunc(Number(input.value)) || min;
    const next = Math.max(min, Math.min(max, numericValue));
    input.value = String(next);
    onChange(next);
  });
  return input;
}

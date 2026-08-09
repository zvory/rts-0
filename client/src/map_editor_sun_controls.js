import { MAP_EDITOR_DEFAULT_SUN } from "./map_editor_session.js";

export function createMapEditorSunSettings(session, viewport) {
  const section = group("Sun & atmosphere");
  const sun = session.draft.sun;
  if (!sun) {
    section.append(
      readout("Add directional sunlight to this map. Flat maps can use sunlight and projected unit shadows without elevation relief."),
      actionButton("Enable sunlight", () => enableMapEditorSun(session)),
    );
    return section;
  }
  section.append(
    sunRangeField("Direction", sun.azimuthDegrees, 0, 359, "°", (value) => {
      previewMapEditorSunField(session, viewport, "azimuthDegrees", value);
    }, (value) => commitMapEditorSunField(session, "azimuthDegrees", value)),
    sunRangeField("Height", sun.elevationDegrees, 1, 89, "°", (value) => {
      previewMapEditorSunField(session, viewport, "elevationDegrees", value);
    }, (value) => commitMapEditorSunField(session, "elevationDegrees", value)),
    sunRangeField("Color temperature", sun.warmth, 0, 100, "% warm", (value) => {
      previewMapEditorSunField(session, viewport, "warmth", value);
    }, (value) => commitMapEditorSunField(session, "warmth", value)),
    readout("Drag any control to preview lighting, shadow direction, and atmosphere live. Changes are saved in the map and can be undone."),
  );
  if (!hasElevationRelief(session.draft)) {
    section.appendChild(actionButton("Remove sunlight", () => disableMapEditorSun(session)));
  }
  return section;
}

export function enableMapEditorSun(session) {
  return session.mutate("Enabled sunlight", (draft) => { draft.sun = { ...MAP_EDITOR_DEFAULT_SUN }; });
}

export function disableMapEditorSun(session) {
  if (hasElevationRelief(session.draft)) return false;
  return session.mutate("Disabled sunlight", (draft) => { draft.sun = null; });
}

function hasElevationRelief(draft) {
  const levels = draft?.elevation?.flatMap((row) => [...row]) || [];
  return levels.length > 0 && levels.some((level) => level !== levels[0]);
}

export function previewMapEditorSunField(session, viewport, fieldName, value) {
  const sun = session.draft?.sun;
  if (!sun || !Object.prototype.hasOwnProperty.call(sun, fieldName)) return false;
  return viewport.previewSunConditions({ ...sun, [fieldName]: boundedSunField(fieldName, value) });
}

export function commitMapEditorSunField(session, fieldName, value) {
  const sun = session.draft?.sun;
  if (!sun || !Object.prototype.hasOwnProperty.call(sun, fieldName)) return false;
  const next = boundedSunField(fieldName, value);
  return session.mutate("Changed sun conditions", (draft) => { draft.sun[fieldName] = next; });
}

function sunRangeField(labelText, value, min, max, suffix, onInput, onChange) {
  const wrapper = document.createElement("div");
  wrapper.className = "map-editor-sun-control";
  const input = document.createElement("input");
  input.type = "range";
  input.min = String(min);
  input.max = String(max);
  input.step = "1";
  input.value = String(value);
  input.setAttribute("aria-label", `Sun ${labelText.toLowerCase()}`);
  const output = document.createElement("output");
  output.value = `${value}${suffix}`;
  output.textContent = output.value;
  input.addEventListener("input", () => {
    const next = Math.max(min, Math.min(max, Math.trunc(Number(input.value)) || min));
    output.value = `${next}${suffix}`;
    output.textContent = output.value;
    onInput(next);
  });
  input.addEventListener("change", () => onChange(input.value));
  wrapper.append(input, output);
  return field(labelText, wrapper);
}

function boundedSunField(fieldName, value) {
  const [min, max] = fieldName === "azimuthDegrees" ? [0, 359]
    : fieldName === "elevationDegrees" ? [1, 89]
      : fieldName === "warmth" ? [0, 100] : [0, 0];
  return Math.max(min, Math.min(max, Math.trunc(Number(value)) || min));
}

function group(title) {
  const section = document.createElement("fieldset");
  section.className = "map-editor-group";
  const legend = document.createElement("legend");
  legend.textContent = title;
  section.appendChild(legend);
  return section;
}

function field(labelText, control) {
  const label = document.createElement("label");
  label.className = "map-editor-field";
  const text = document.createElement("span");
  text.textContent = labelText;
  label.append(text, control);
  return label;
}

function readout(text) {
  const node = document.createElement("p");
  node.className = "map-editor-readout";
  node.dataset.state = "ok";
  node.textContent = text;
  return node;
}

function actionButton(label, onClick) {
  const control = document.createElement("button");
  control.type = "button";
  control.className = "map-editor-button";
  control.textContent = label;
  control.addEventListener("click", onClick);
  return control;
}

import { MAP_EDITOR_DEFAULT_SUN } from "./map_editor_session.js";
import {
  boundedStalingradTime,
  formatStalingradTime,
  STALINGRAD_SUN_PRESET,
  stalingradSunAtTime,
  stalingradTimeFromSun,
} from "./map_editor_stalingrad_sun.js";

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
  const stalingradTime = stalingradTimeFromSun(sun);
  section.append(
    stalingradTimeField(stalingradTime, (value) => {
      previewMapEditorStalingradTime(session, viewport, value);
    }, (value) => {
      commitMapEditorStalingradTime(session, value);
      viewport.clearSunDirectionPreview?.();
    }, {
      onBegin: () => viewport.previewSunDirection?.(
        stalingradSunAtTime(stalingradTime).azimuthDegrees,
      ),
      onEnd: () => viewport.clearSunDirectionPreview?.(),
      onCancel: () => restoreMapEditorSunPreview(session, viewport),
    }),
    sunRangeField("Direction", sun.azimuthDegrees, 0, 359, "°", (value) => {
      previewMapEditorSunDirectionField(session, viewport, value);
    }, (value) => {
      commitMapEditorSunField(session, "azimuthDegrees", value);
      viewport.clearSunDirectionPreview?.();
    }, {
      onBegin: () => viewport.previewSunDirection?.(sun.azimuthDegrees),
      onEnd: () => viewport.clearSunDirectionPreview?.(),
    }),
    sunRangeField("Height", sun.elevationDegrees, 1, 89, "°", (value) => {
      previewMapEditorSunField(session, viewport, "elevationDegrees", value);
    }, (value) => commitMapEditorSunField(session, "elevationDegrees", value)),
    sunRangeField("Color temperature", sun.warmth, 0, 100, "% warm", (value) => {
      previewMapEditorSunField(session, viewport, "warmth", value);
    }, (value) => commitMapEditorSunField(session, "warmth", value)),
    readout("Historical daylight: 23 Aug 1942 on the steppe west of Stalingrad (48.7°N, 44.3°E). Local solar time sets direction, height, and warmth together."),
    readout("Direction uses compass degrees: 0° north, 90° east, 180° south, 270° west. Drag it to show the sun-source arrow on the map."),
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

export function previewMapEditorSunDirectionField(session, viewport, value) {
  const direction = boundedSunField("azimuthDegrees", value);
  viewport.previewSunDirection?.(direction);
  return previewMapEditorSunField(session, viewport, "azimuthDegrees", direction);
}

export function commitMapEditorSunField(session, fieldName, value) {
  const sun = session.draft?.sun;
  if (!sun || !Object.prototype.hasOwnProperty.call(sun, fieldName)) return false;
  const next = boundedSunField(fieldName, value);
  return session.mutate("Changed sun conditions", (draft) => { draft.sun[fieldName] = next; });
}

export function previewMapEditorStalingradTime(session, viewport, value) {
  if (!session.draft?.sun) return false;
  const conditions = stalingradSunAtTime(value);
  viewport.previewSunDirection?.(conditions.azimuthDegrees);
  return viewport.previewSunConditions(conditions);
}

export function commitMapEditorStalingradTime(session, value) {
  if (!session.draft?.sun) return false;
  const conditions = stalingradSunAtTime(value);
  return session.mutate("Set Stalingrad time of day", (draft) => {
    draft.sun = { ...conditions };
  });
}

export function restoreMapEditorSunPreview(session, viewport) {
  const sun = session.draft?.sun;
  if (!sun) return false;
  viewport.clearSunDirectionPreview?.();
  return viewport.previewSunConditions({ ...sun });
}

function stalingradTimeField(value, onInput, onChange, {
  onBegin, onEnd, onCancel,
} = {}) {
  const wrapper = document.createElement("div");
  wrapper.className = "map-editor-sun-control";
  const input = document.createElement("input");
  input.type = "range";
  input.min = String(STALINGRAD_SUN_PRESET.minTimeHours);
  input.max = String(STALINGRAD_SUN_PRESET.maxTimeHours);
  input.step = String(STALINGRAD_SUN_PRESET.timeStepHours);
  input.value = String(value);
  input.setAttribute("aria-label", "Stalingrad daylight time");
  const output = document.createElement("output");
  const updateOutput = (next) => {
    output.value = formatStalingradTime(next);
    output.textContent = output.value;
  };
  updateOutput(value);
  input.addEventListener("input", () => {
    const next = boundedStalingradTime(input.value);
    updateOutput(next);
    onInput(next);
  });
  input.addEventListener("change", () => onChange(input.value));
  input.addEventListener("pointerdown", () => onBegin?.());
  input.addEventListener("pointerup", () => onEnd?.());
  input.addEventListener("pointercancel", () => (onCancel || onEnd)?.());
  input.addEventListener("blur", () => onEnd?.());
  wrapper.append(input, output);
  return field("Time of day · Stalingrad steppe", wrapper);
}

function sunRangeField(labelText, value, min, max, suffix, onInput, onChange, { onBegin, onEnd } = {}) {
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
  input.addEventListener("pointerdown", () => onBegin?.());
  input.addEventListener("pointerup", () => onEnd?.());
  input.addEventListener("pointercancel", () => onEnd?.());
  input.addEventListener("blur", () => onEnd?.());
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

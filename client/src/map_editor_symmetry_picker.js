const PREVIEW_SIZE = 100;
const PREVIEW_MARKER = Object.freeze([
  { x: 59, y: 15 }, { x: 74, y: 15 }, { x: 74, y: 10 },
  { x: 83, y: 20 }, { x: 74, y: 30 }, { x: 74, y: 25 }, { x: 59, y: 25 },
]);

export function createMapEditorSymmetryPicker({
  symmetry,
  symmetryValues,
  dimensions,
  isSupported,
  onSelect,
  transforms,
  transformPoint,
}) {
  const options = symmetryOptions(symmetryValues);
  const preview = (value) => createPreview(value, symmetryValues, transforms, transformPoint);
  const controls = document.createElement("div");
  controls.className = "map-editor-symmetry-controls";
  controls.title = "Symmetry applies to terrain, zones, objects, forests, roads, and locations.";
  const selected = options.find(([value]) => value === symmetry) || options[0];
  const picker = document.createElement("div");
  picker.className = "map-editor-symmetry-picker";
  const trigger = document.createElement("button");
  trigger.type = "button";
  trigger.className = "map-editor-symmetry-trigger";
  trigger.setAttribute("aria-label", `Symmetry: ${selected[1]}`);
  trigger.setAttribute("aria-haspopup", "listbox");
  trigger.setAttribute("aria-expanded", "false");
  trigger.append(preview(symmetry), pickerLabel(selected[1]), pickerChevron());
  const menu = document.createElement("div");
  menu.className = "map-editor-symmetry-menu";
  menu.setAttribute("role", "listbox");
  menu.setAttribute("aria-label", "Symmetry options");
  menu.setAttribute("popover", "auto");
  for (const [value, label] of options) {
    const option = document.createElement("button");
    option.type = "button";
    option.className = "map-editor-symmetry-option";
    option.dataset.symmetry = value;
    option.setAttribute("role", "option");
    option.setAttribute("aria-selected", String(value === symmetry));
    option.disabled = !isSupported(dimensions, value);
    option.append(preview(value), pickerLabel(label));
    option.addEventListener("click", () => onSelect(value));
    menu.appendChild(option);
  }
  connectMenu(picker, trigger, menu);
  picker.append(trigger, menu);
  controls.appendChild(picker);
  return controls;
}

function symmetryOptions(values) {
  return [
    [values.NONE, "None"],
    [values.HORIZONTAL, "Horizontal"],
    [values.VERTICAL, "Vertical"],
    [values.HALF_TURN, "Half-turn (180°)"],
    [values.THREE_WAY, "3-way rotation (120°, square-grid approximation)"],
    [values.RADIAL, "Radial (4-way)"],
    [values.QUADRANT_MIRROR, "Quadrant mirror (4-way)"],
    [values.DIAGONAL_MAIN, "Diagonal ↘ (top-left ↔ bottom-right)"],
    [values.DIAGONAL_ANTI, "Diagonal ↙ (top-right ↔ bottom-left)"],
    [values.DIAGONAL_MAIN_FLIP, "Diagonal ↘ · flipped copy"],
    [values.DIAGONAL_ANTI_FLIP, "Diagonal ↙ · flipped copy"],
  ];
}

function connectMenu(picker, trigger, menu) {
  const setOpen = (open) => {
    if (open === menu.matches(":popover-open")) return;
    if (open) {
      positionMenu(menu, trigger);
      menu.showPopover();
    } else {
      menu.hidePopover();
    }
    trigger.setAttribute("aria-expanded", String(open));
    picker.dataset.open = open ? "true" : "false";
    if (open) menu.querySelector('[aria-selected="true"]:not(:disabled)')?.focus();
  };
  trigger.addEventListener("click", () => setOpen(!menu.matches(":popover-open")));
  trigger.addEventListener("keydown", (event) => {
    if (!["ArrowDown", "ArrowUp"].includes(event.key)) return;
    event.preventDefault();
    setOpen(true);
  });
  menu.addEventListener("keydown", (event) => handleMenuKey(event, menu, trigger, setOpen));
  menu.addEventListener("toggle", (event) => {
    const open = event.newState === "open";
    trigger.setAttribute("aria-expanded", String(open));
    picker.dataset.open = open ? "true" : "false";
  });
  picker.addEventListener("focusout", (event) => {
    if (!picker.contains(event.relatedTarget)) setOpen(false);
  });
}

function pickerLabel(text) {
  const label = document.createElement("span");
  label.className = "map-editor-symmetry-option-label";
  label.textContent = text;
  return label;
}

function pickerChevron() {
  const chevron = document.createElement("span");
  chevron.className = "map-editor-symmetry-chevron";
  chevron.textContent = "▾";
  chevron.setAttribute("aria-hidden", "true");
  return chevron;
}

function handleMenuKey(event, menu, trigger, setOpen) {
  if (event.key === "Escape") {
    event.preventDefault();
    setOpen(false);
    trigger.focus();
    return;
  }
  if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  const options = [...menu.querySelectorAll(".map-editor-symmetry-option:not(:disabled)")];
  if (!options.length) return;
  const current = Math.max(0, options.indexOf(document.activeElement));
  const next = event.key === "Home" ? 0
    : event.key === "End" ? options.length - 1
      : event.key === "ArrowDown" ? (current + 1) % options.length
        : (current - 1 + options.length) % options.length;
  options[next].focus();
}

function positionMenu(menu, trigger) {
  const triggerRect = trigger.getBoundingClientRect();
  const margin = 8;
  const gap = 5;
  const spaceAbove = Math.max(0, triggerRect.top - margin - gap);
  const spaceBelow = Math.max(0, window.innerHeight - triggerRect.bottom - margin - gap);
  const openAbove = spaceAbove > spaceBelow;
  const available = openAbove ? spaceAbove : spaceBelow;
  menu.style.left = `${Math.max(margin, Math.min(triggerRect.left, window.innerWidth - triggerRect.width - margin))}px`;
  menu.style.width = `${triggerRect.width}px`;
  menu.style.maxHeight = `${Math.max(72, Math.min(620, available))}px`;
  menu.style.top = openAbove ? "auto" : `${triggerRect.bottom + gap}px`;
  menu.style.bottom = openAbove ? `${window.innerHeight - triggerRect.top + gap}px` : "auto";
}

function createPreview(symmetry, values, transforms, transformPoint) {
  const namespace = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(namespace, "svg");
  svg.classList.add("map-editor-symmetry-preview");
  svg.setAttribute("viewBox", `0 0 ${PREVIEW_SIZE} ${PREVIEW_SIZE}`);
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", previewLabel(symmetry, values));

  const background = document.createElementNS(namespace, "rect");
  background.setAttribute("x", "1");
  background.setAttribute("y", "1");
  background.setAttribute("width", "98");
  background.setAttribute("height", "98");
  background.setAttribute("rx", "7");
  background.classList.add("map-editor-symmetry-preview-background");
  svg.appendChild(background);

  for (const [x0, y0, x1, y1] of previewGuides(symmetry, values)) {
    const guide = document.createElementNS(namespace, "line");
    guide.setAttribute("x1", String(x0));
    guide.setAttribute("y1", String(y0));
    guide.setAttribute("x2", String(x1));
    guide.setAttribute("y2", String(y1));
    guide.classList.add("map-editor-symmetry-preview-guide");
    svg.appendChild(guide);
  }

  const dimensions = { width: PREVIEW_SIZE, height: PREVIEW_SIZE };
  const points = previewMarker(symmetry, values);
  for (const transform of transforms(dimensions, symmetry)) {
    const marker = document.createElementNS(namespace, "polygon");
    marker.setAttribute("points", points.map((point) => {
      const transformed = transformPoint(point, dimensions, transform);
      return `${transformed.x},${transformed.y}`;
    }).join(" "));
    marker.classList.add("map-editor-symmetry-preview-copy");
    if (transform === "identity") marker.classList.add("map-editor-symmetry-preview-source");
    svg.appendChild(marker);
  }

  const centre = document.createElementNS(namespace, "circle");
  centre.setAttribute("cx", "50");
  centre.setAttribute("cy", "50");
  centre.setAttribute("r", "2");
  centre.classList.add("map-editor-symmetry-preview-centre");
  svg.appendChild(centre);
  return svg;
}

function previewMarker(symmetry, values) {
  let offsetX = 0;
  let offsetY = 0;
  if ([values.DIAGONAL_MAIN, values.DIAGONAL_MAIN_FLIP].includes(symmetry)) {
    offsetX = -21;
  } else if ([values.DIAGONAL_ANTI, values.DIAGONAL_ANTI_FLIP].includes(symmetry)) {
    offsetX = -51;
    offsetY = 30;
  }
  return PREVIEW_MARKER.map(({ x, y }) => ({ x: x + offsetX, y: y + offsetY }));
}

function previewGuides(symmetry, values) {
  const horizontal = [3, 50, 97, 50];
  const vertical = [50, 3, 50, 97];
  const main = [3, 3, 97, 97];
  const anti = [3, 97, 97, 3];
  if (symmetry === values.HORIZONTAL) return [horizontal];
  if (symmetry === values.VERTICAL) return [vertical];
  if ([values.RADIAL, values.QUADRANT_MIRROR].includes(symmetry)) return [horizontal, vertical];
  if ([values.DIAGONAL_MAIN, values.DIAGONAL_MAIN_FLIP].includes(symmetry)) return [main];
  if ([values.DIAGONAL_ANTI, values.DIAGONAL_ANTI_FLIP].includes(symmetry)) return [anti];
  if (symmetry === values.THREE_WAY) {
    return [[50, 50, 50, 3], [50, 50, 91, 74], [50, 50, 9, 74]];
  }
  return [];
}

function previewLabel(symmetry, values) {
  const labels = {
    [values.NONE]: "No symmetry preview",
    [values.HORIZONTAL]: "Horizontal reflection preview with two copies",
    [values.VERTICAL]: "Vertical reflection preview with two copies",
    [values.HALF_TURN]: "Half-turn preview with two copies",
    [values.THREE_WAY]: "Approximate three-way rotation preview with three copies",
    [values.RADIAL]: "Quarter-turn rotation preview with four copies",
    [values.QUADRANT_MIRROR]: "Quadrant mirror preview with four copies",
    [values.DIAGONAL_MAIN_FLIP]: "Main diagonal with flipped partner preview with two copies",
    [values.DIAGONAL_ANTI_FLIP]: "Anti-diagonal with flipped partner preview with two copies",
    [values.DIAGONAL_MAIN]: "Main diagonal reflection preview with two copies",
    [values.DIAGONAL_ANTI]: "Anti-diagonal reflection preview with two copies",
  };
  return labels[symmetry] || labels[values.NONE];
}

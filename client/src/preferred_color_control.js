import { PLAYER_PALETTE } from "./config.js";

const COLOR_NAMES = ["Blue", "Orange", "Green", "Pink", "Light blue", "Amber", "Yellow", "Purple"];

export function renderPreferredColorControl(root, preference) {
  const row = document.createElement("div");
  row.className = "preferred-color-setting";
  const label = document.createElement("span");
  label.id = "preferred-color-label";
  label.textContent = "Preferred color";
  const wrap = document.createElement("div");
  wrap.className = "preferred-color-wrap";
  const trigger = document.createElement("button");
  trigger.type = "button";
  trigger.id = "preferred-color-trigger";
  trigger.className = "preferred-color-trigger";
  trigger.setAttribute("aria-labelledby", label.id + " preferred-color-value");
  trigger.setAttribute("aria-haspopup", "listbox");
  trigger.setAttribute("aria-expanded", "false");
  trigger.setAttribute("aria-controls", "preferred-color-options");
  const list = document.createElement("div");
  list.id = "preferred-color-options";
  list.className = "preferred-color-options";
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-labelledby", label.id);
  list.hidden = true;
  const choices = [[null, "No preference"], ...PLAYER_PALETTE.map((color, i) => [color, COLOR_NAMES[i]])];
  const buttons = [];
  function swatch(color) {
    const element = document.createElement("span");
    element.className = "preferred-color-swatch";
    if (color) element.style.backgroundColor = color;
    else element.classList.add("preferred-color-auto");
    element.setAttribute("aria-hidden", "true");
    return element;
  }
  function sync() {
    const color = preference.read();
    const name = choices.find(([value]) => value === color)?.[1] || "No preference";
    const value = document.createElement("span");
    value.id = "preferred-color-value";
    value.textContent = name;
    const arrow = document.createElement("span");
    arrow.className = "preferred-color-arrow";
    arrow.textContent = "▾";
    arrow.setAttribute("aria-hidden", "true");
    trigger.replaceChildren(swatch(color), value, arrow);
    buttons.forEach((button, i) => {
      const selected = choices[i][0] === color;
      button.setAttribute("aria-selected", String(selected));
      button.lastChild.textContent = selected ? "✓" : "";
    });
  }
  function close(focus = false) {
    list.hidden = true;
    trigger.setAttribute("aria-expanded", "false");
    if (focus) trigger.focus();
  }
  function open() {
    list.hidden = false;
    // Fixed positioning escapes the gear panel’s scrolling clip.
    const bounds = trigger.getBoundingClientRect();
    const height = Math.min(340, window.innerHeight - 16);
    list.style.maxHeight = `${height}px`;
    list.style.width = `${bounds.width}px`;
    list.style.left = `${bounds.left}px`;
    const below = bounds.bottom + 4;
    list.style.top = `${below + list.offsetHeight <= window.innerHeight - 8
      ? below : Math.max(8, bounds.top - list.offsetHeight - 4)}px`;
    trigger.setAttribute("aria-expanded", "true");
    buttons[choices.findIndex(([color]) => color === preference.read())]?.focus();
  }
  for (const [color, name] of choices) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "preferred-color-option";
    button.setAttribute("role", "option");
    button.tabIndex = -1;
    const text = document.createElement("span");
    text.textContent = name;
    const check = document.createElement("span");
    check.className = "preferred-color-check";
    check.setAttribute("aria-hidden", "true");
    button.append(swatch(color), text, check);
    button.addEventListener("click", () => { preference.write(color); sync(); close(true); });
    buttons.push(button);
    list.append(button);
  }
  trigger.addEventListener("click", () => list.hidden ? open() : close());
  row.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !list.hidden) { event.preventDefault(); close(true); }
    if (event.key === "Tab") close();
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      if (list.hidden) { open(); return; }
      const index = buttons.indexOf(document.activeElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1
        : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length;
      buttons[next].focus();
    }
  });
  const outside = (event) => { if (!row.contains(event.target)) close(); };
  const reposition = () => close();
  document.addEventListener("pointerdown", outside);
  window.addEventListener("resize", reposition);
  row.addEventListener("focusout", (event) => { if (!row.contains(event.relatedTarget)) close(); });
  wrap.append(trigger, list);
  row.append(label, wrap);
  root.append(row);
  sync();
  return () => {
    document.removeEventListener("pointerdown", outside);
    window.removeEventListener("resize", reposition);
  };
}

import { FloatingRoomTimePanel } from "../../client/src/room_time_panel.js";
import { createImmediateTouchButtonActivation } from "../../client/src/panel_touch_activation.js";
import { assert } from "./assertions.mjs";

const priorDocument = globalThis.document;
const priorWindow = globalThis.window;
const localStorageValues = new Map();
const windowListeners = new Map();

globalThis.document = {
  documentElement: { clientWidth: 1000, clientHeight: 800 },
  createElement(tagName) {
    return fakeEl(tagName);
  },
};
globalThis.window = {
  innerWidth: 1000,
  innerHeight: 800,
  localStorage: {
    getItem(key) {
      return localStorageValues.get(key) ?? null;
    },
    setItem(key, value) {
      localStorageValues.set(key, String(value));
    },
    removeItem(key) {
      localStorageValues.delete(key);
    },
  },
  addEventListener(type, handler) {
    windowListeners.set(type, handler);
  },
  removeEventListener(type, handler) {
    if (windowListeners.get(type) === handler) windowListeners.delete(type);
  },
};

try {
  const root = fakeEl("div");
  const speed = fakeEl("button");
  speed.className = "spd-btn";
  root.appendChild(speed);

  const panel = new FloatingRoomTimePanel({ root, label: "Room time" });
  panel.mount();
  const collapse = root.querySelector(".room-time-panel-collapse");
  const body = root.querySelector(".room-time-panel-body");

  collapse._listeners.get("pointerdown")({
    button: 0,
    isPrimary: true,
    pointerId: 8,
    pointerType: "touch",
  });
  collapse._listeners.get("pointerup")({
    pointerId: 8,
    pointerType: "touch",
    preventDefault() {},
    stopPropagation() {},
  });
  assert(
    root.dataset.collapsed === "true" && body.hidden === true,
    "FloatingRoomTimePanel touch release toggles collapse without waiting for a synthesized click",
  );

  collapse._listeners.get("click")({
    preventDefault() {},
    stopPropagation() {},
  });
  assert(root.dataset.collapsed === "true", "FloatingRoomTimePanel ignores the click synthesized after touch collapse");

  collapse._listeners.get("pointerdown")({
    button: 0,
    isPrimary: true,
    pointerId: 10,
    pointerType: "touch",
  });
  collapse._listeners.get("pointerleave")({
    pointerId: 10,
    pointerType: "touch",
  });
  collapse._listeners.get("pointerup")({
    pointerId: 10,
    pointerType: "touch",
    preventDefault() {},
    stopPropagation() {},
  });
  assert(root.dataset.collapsed === "true", "FloatingRoomTimePanel cancels touch collapse when the pointer leaves");
  panel.destroy();

  let activations = 0;
  let now = 100;
  const activation = createImmediateTouchButtonActivation(() => { activations += 1; }, { now: () => now });
  activation.pointerdown({ button: 0, isPrimary: true, pointerId: 1, pointerType: "pen" });
  activation.pointerup({ pointerId: 1, pointerType: "pen", preventDefault() {}, stopPropagation() {} });
  assert(activations === 1, "touch activation accepts a primary pen release");
  activation.click({ preventDefault() {}, stopPropagation() {} });
  assert(activations === 1, "touch activation suppresses the pen's synthesized duplicate click");

  now += 1_000;
  activation.pointerdown({ button: 0, isPrimary: true, pointerId: 2, pointerType: "touch" });
  activation.pointerup({ pointerId: 3, pointerType: "touch", preventDefault() {}, stopPropagation() {} });
  activation.pointercancel({ pointerId: 2, pointerType: "touch" });
  assert(activations === 1, "touch activation ignores a mismatched pointer and cancellation");

  now += 1_000;
  const capturedButton = {
    contains: () => true,
    getBoundingClientRect: () => ({ left: 10, top: 10, right: 50, bottom: 50 }),
  };
  activation.pointerdown({ button: 0, isPrimary: true, pointerId: 4, pointerType: "touch" });
  activation.pointerup({
    pointerId: 4,
    pointerType: "touch",
    currentTarget: capturedButton,
    target: capturedButton,
    clientX: 70,
    clientY: 30,
    preventDefault() {},
    stopPropagation() {},
  });
  assert(
    activations === 1,
    "touch activation rejects an outside release even when pointer capture keeps the button targeted",
  );
  activation.click({ preventDefault() {}, stopPropagation() {} });
  assert(activations === 1, "touch activation suppresses the synthetic click after an outside release");

  now += 1_000;
  activation.click({});
  assert(activations === 2, "touch activation preserves the native mouse and keyboard click path");
} finally {
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
}

function fakeEl(tagName = "div") {
  const el = {
    tagName: String(tagName).toUpperCase(),
    children: [],
    className: "",
    dataset: {},
    hidden: false,
    textContent: "",
    type: "",
    _listeners: new Map(),
    classList: {
      add(value) {
        setClass(el, value, true);
      },
      remove(value) {
        setClass(el, value, false);
      },
      contains(value) {
        return classNames(el).includes(value);
      },
    },
    style: {
      left: "",
      top: "",
      right: "",
      bottom: "",
      transform: "",
      width: "",
      height: "",
      setProperty(name, value) {
        this[toCamelCase(name)] = String(value);
      },
      removeProperty(name) {
        this[toCamelCase(name)] = "";
      },
    },
    setAttribute(name, value) {
      this[name] = String(value);
    },
    append(...children) {
      for (const child of children) this.appendChild(child);
    },
    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
      return child;
    },
    replaceChildren(...children) {
      for (const child of this.children) child.parentNode = null;
      this.children = [];
      this.append(...children);
    },
    addEventListener(type, handler) {
      this._listeners.set(type, handler);
    },
    removeEventListener(type, handler) {
      if (this._listeners.get(type) === handler) this._listeners.delete(type);
    },
    getBoundingClientRect() {
      return { left: 0, top: 0, width: 240, height: 96 };
    },
    querySelector(selector) {
      return this.querySelectorAll(selector)[0] || null;
    },
    querySelectorAll(selector) {
      const results = [];
      const visit = (node) => {
        if (matches(node, selector)) results.push(node);
        for (const child of node.children || []) visit(child);
      };
      visit(this);
      return results;
    },
  };
  return el;
}

function matches(node, selector) {
  if (selector.startsWith(".")) return classNames(node).includes(selector.slice(1));
  return node.tagName === selector.toUpperCase();
}

function setClass(el, value, enabled) {
  const next = new Set(classNames(el));
  if (enabled) next.add(value);
  else next.delete(value);
  el.className = Array.from(next).join(" ");
}

function classNames(el) {
  return String(el?.className || "").split(/\s+/).filter(Boolean);
}

function toCamelCase(property) {
  return property.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
}

export function withFakeDocument(fn) {
  const priorDocument = globalThis.document;
  const created = [];
  const restore = () => {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
  };
  const docListeners = {};
  globalThis.document = {
    activeElement: null,
    listeners: docListeners,
    addEventListener(type, handler) {
      docListeners[type] = handler;
    },
    removeEventListener(type, handler) {
      if (docListeners[type] === handler) delete docListeners[type];
    },
    createElement(tagName) {
      const el = {
        tagName: String(tagName).toUpperCase(),
        className: "",
        classList: fakeClassList(),
        children: [],
        dataset: {},
        disabled: false,
        hidden: false,
        title: "",
        type: "",
        value: "",
        innerHTML: "",
        listeners: {},
        style: { setProperty() {} },
        addEventListener(type, handler) {
          this.listeners[type] = handler;
        },
        removeEventListener(type, handler) {
          if (this.listeners[type] === handler) delete this.listeners[type];
        },
        append(...children) {
          this.children.push(...children);
        },
        appendChild(child) {
          this.children.push(child);
          return child;
        },
        replaceChildren(...children) {
          this.children = [...children];
        },
        setAttribute(name, value) {
          this[name] = String(value);
        },
        remove() {
          this.removed = true;
        },
        focus() {
          globalThis.document.activeElement = this;
        },
        click() {
          this.listeners.click?.({ target: this, preventDefault() {}, stopPropagation() {} });
        },
        querySelectorAll() {
          return [];
        },
      };
      created.push(el);
      return el;
    },
    createDocumentFragment() {
      return { children: [], appendChild(child) { this.children.push(child); } };
    },
  };
  try {
    const result = fn(created);
    if (result && typeof result.finally === "function") return result.finally(restore);
    restore();
    return result;
  } catch (err) {
    restore();
    throw err;
  }
}

export function fakeClassList() {
  const values = new Set();
  return {
    add(value) { values.add(value); },
    remove(value) { values.delete(value); },
    contains(value) { return values.has(value); },
    toggle(value, enabled) {
      if (enabled) values.add(value);
      else values.delete(value);
    },
  };
}

export function withFakeHudDocument(fn) {
  const priorDocument = globalThis.document;
  class FakeElement {
    constructor(tagName) {
      this.tagName = String(tagName).toUpperCase();
      this.type = "";
      this.className = "";
      this.textContent = "";
      this.title = "";
      this.children = [];
      this.parentNode = null;
      this.dataset = {};
      this.listeners = {};
      this.attributes = new Map();
      this.style = {
        values: new Map(),
        setProperty: (name, value) => {
          this.style.values.set(name, String(value));
        },
      };
      this._innerHTML = "";
    }
    set innerHTML(value) {
      this._innerHTML = String(value);
      if (value === "") {
        for (const child of this.children || []) child.parentNode = null;
        this.children = [];
      }
    }
    get innerHTML() {
      return this._innerHTML;
    }
    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
      return child;
    }
    setAttribute(name, value) {
      this.attributes.set(name, String(value));
    }
    getAttribute(name) {
      return this.attributes.get(name) || null;
    }
    addEventListener(type, handler) {
      this.listeners[type] = handler;
    }
    removeEventListener(type, handler) {
      if (this.listeners[type] === handler) delete this.listeners[type];
    }
    contains(node) {
      for (let cur = node; cur; cur = cur.parentNode) {
        if (cur === this) return true;
      }
      return false;
    }
    closest(selector) {
      for (let cur = this; cur; cur = cur.parentNode) {
        if (matches(cur, selector)) return cur;
      }
      return null;
    }
    querySelectorAll(selector) {
      const results = [];
      const visit = (node) => {
        if (matches(node, selector)) results.push(node);
        for (const child of node.children || []) visit(child);
      };
      visit(this);
      return results;
    }
    querySelector(selector) {
      return this.querySelectorAll(selector)[0] || null;
    }
  }
  function matches(node, selector) {
    if (!node) return false;
    if (selector.startsWith(".")) return node.className.split(/\s+/).includes(selector.slice(1));
    if (selector.startsWith("[")) return node.attributes.has(selector.slice(1, -1));
    return node.tagName === selector.toUpperCase();
  }
  globalThis.document = {
    createElement(tagName) {
      return new FakeElement(tagName);
    },
    createDocumentFragment() {
      return new FakeElement("fragment");
    },
  };
  try {
    return fn({ FakeElement });
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
  }
}

export function withFakeSettingsDocument(fn) {
  const priorDocument = globalThis.document;
  const priorHTMLElement = globalThis.HTMLElement;
  const priorWindow = globalThis.window;
  const windowListeners = {};
  class FakeElement {
    constructor(tagName) {
      this.tagName = String(tagName).toUpperCase();
      this.id = "";
      this.type = "";
      this.className = "";
      this.textContent = "";
      this.innerHTML = "";
      this.hidden = false;
      this.disabled = false;
      this.value = "";
      this.dataset = {};
      this.children = [];
      this.attributes = new Map();
      this.listeners = {};
      this.classList = {
        add: (value) => {
          this.className = this.className ? `${this.className} ${value}` : value;
        },
      };
    }
    append(...children) {
      this.children.push(...children);
    }
    appendChild(child) {
      this.children.push(child);
      return child;
    }
    setAttribute(name, value) {
      this.attributes.set(name, String(value));
    }
    getAttribute(name) {
      return this.attributes.get(name) || null;
    }
    addEventListener(type, handler) {
      this.listeners[type] = handler;
    }
    replaceChildren(...children) {
      this.children = [...children];
    }
    click(init = {}) {
      this.listeners.click?.({ preventDefault() {}, ...init });
    }
  }
  globalThis.HTMLElement = FakeElement;
  globalThis.document = {
    createElement(tagName) {
      return new FakeElement(tagName);
    },
  };
  globalThis.window = {
    addEventListener(type, handler) {
      windowListeners[type] = handler;
    },
    removeEventListener(type, handler) {
      if (windowListeners[type] === handler) delete windowListeners[type];
    },
    listeners: windowListeners,
  };
  try {
    return fn(windowListeners);
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorHTMLElement === undefined) delete globalThis.HTMLElement;
    else globalThis.HTMLElement = priorHTMLElement;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  }
}

export function withFakeOverlayDocument(fn) {
  const priorDocument = globalThis.document;
  const priorElement = globalThis.Element;

  class FakeElement {
    constructor(tagName) {
      this.tagName = String(tagName).toUpperCase();
      this.id = "";
      this.type = "";
      this.className = "";
      this.textContent = "";
      this.title = "";
      this.hidden = false;
      this.tabIndex = 0;
      this.dataset = {};
      this.children = [];
      this.parentNode = null;
      this.focused = false;
      this.replaceChildrenCount = 0;
      this.listeners = {};
      this.attributes = new Map();
      this.classList = {
        add: (value) => this.setClass(value, true),
        remove: (value) => this.setClass(value, false),
        toggle: (value, enabled) => this.setClass(value, !!enabled),
        contains: (value) => this.className.split(/\s+/).includes(value),
      };
    }
    setClass(value, enabled) {
      const classes = new Set(this.className.split(/\s+/).filter(Boolean));
      if (enabled) classes.add(value);
      else classes.delete(value);
      this.className = [...classes].join(" ");
    }
    append(...children) {
      for (const child of children) this.appendChild(child);
    }
    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
      return child;
    }
    replaceChildren(...children) {
      this.replaceChildrenCount += 1;
      for (const child of this.children) child.parentNode = null;
      this.children = [];
      this.append(...children);
    }
    remove() {
      if (!this.parentNode) return;
      const siblings = this.parentNode.children;
      const index = siblings.indexOf(this);
      if (index >= 0) siblings.splice(index, 1);
      this.parentNode = null;
    }
    addEventListener(type, handler) {
      this.listeners[type] = handler;
    }
    removeEventListener(type, handler) {
      if (this.listeners[type] === handler) delete this.listeners[type];
    }
    focus() {
      this.focused = true;
    }
    setAttribute(name, value) {
      this.attributes.set(name, String(value));
    }
    getAttribute(name) {
      return this.attributes.get(name) || null;
    }
    contains(node) {
      for (let cur = node; cur; cur = cur.parentNode) {
        if (cur === this) return true;
      }
      return false;
    }
    closest(selector) {
      for (let cur = this; cur; cur = cur.parentNode) {
        if (matchesSelector(cur, selector)) return cur;
      }
      return null;
    }
    querySelector(selector) {
      return this.querySelectorAll(selector)[0] || null;
    }
    querySelectorAll(selector) {
      const results = [];
      const visit = (node) => {
        if (matchesSelector(node, selector)) results.push(node);
        for (const child of node.children) visit(child);
      };
      for (const child of this.children) visit(child);
      return results;
    }
  }

  function matchesSelector(node, selector) {
    if (!node) return false;
    if (selector === "button") return node.tagName === "BUTTON";
    if (selector.startsWith(".")) return node.classList.contains(selector.slice(1));
    if (selector.startsWith("#")) return node.id === selector.slice(1);
    return false;
  }

  globalThis.Element = FakeElement;
  globalThis.document = {
    createElement(tagName) {
      return new FakeElement(tagName);
    },
  };

  try {
    return fn({ FakeElement });
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorElement === undefined) delete globalThis.Element;
    else globalThis.Element = priorElement;
  }
}

export function fakeStorage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem(key) {
      return values.has(key) ? values.get(key) : null;
    },
    setItem(key, value) {
      values.set(key, String(value));
    },
    removeItem(key) {
      values.delete(key);
    },
    values,
  };
}

export function findFakeById(root, id) {
  if (root.id === id) return root;
  for (const child of root.children || []) {
    const found = findFakeById(child, id);
    if (found) return found;
  }
  return null;
}

export function findFakes(root, predicate, out = []) {
  if (predicate(root)) out.push(root);
  for (const child of root.children || []) findFakes(child, predicate, out);
  return out;
}

export function memoryStorage(seed = {}) {
  const data = new Map(Object.entries(seed));
  return {
    getItem(key) {
      return data.has(key) ? data.get(key) : null;
    },
    setItem(key, value) {
      data.set(key, String(value));
    },
    data,
  };
}

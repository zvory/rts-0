const LAB_CATALOG_ENDPOINT = "/api/lab-scenarios";
const MAP_CATALOG_ENDPOINT = "/maps/catalog";
const DEFAULT_LAB_MAP = "1v1";

export function normalizeLabScenarioEntry(entry) {
  const id = safeCatalogText(entry?.id, "");
  const title = safeCatalogText(entry?.title, id || "Setup");
  const description = safeCatalogText(entry?.description, "");
  const map = safeCatalogText(entry?.map, DEFAULT_LAB_MAP);
  const playerCount = Math.max(0, Math.trunc(Number(entry?.playerCount) || 0));
  return {
    id,
    title,
    description,
    map,
    playerCount,
  };
}

export function normalizeLabMapEntry(entry) {
  const name = safeCatalogText(entry?.name, "");
  return name ? { name } : null;
}

export class LabCatalogScreen {
  constructor({
    root,
    fetchImpl = globalThis.fetch?.bind(globalThis),
    initialRoom = "default",
    onStart,
  }) {
    this.root = root;
    this.fetchImpl = fetchImpl;
    this.initialRoom = initialRoom;
    this.onStart = onStart;
    this.entries = [];
    this.maps = [];
    this.status = "";
    this.error = "";
    this.connected = false;
    this.loading = false;
    this.roomInput = null;
    this.mapSelect = null;
    this.starting = false;
  }

  mount() {
    this.render();
    void this.loadCatalog();
  }

  setConnected(connected) {
    this.connected = !!connected;
    if (!this.connected) this.starting = false;
    this.render();
  }

  setStatus(status, { error = false } = {}) {
    this.status = String(status || "");
    this.error = error ? this.status : "";
    if (error) this.starting = false;
    this.render();
  }

  async loadCatalog() {
    if (!this.fetchImpl || this.loading) return;
    this.loading = true;
    this.error = "";
    this.render();
    try {
      const [scenarioResponse, mapResponse] = await Promise.all([
        this.fetchImpl(LAB_CATALOG_ENDPOINT, { cache: "no-store" }),
        this.fetchImpl(MAP_CATALOG_ENDPOINT, { cache: "no-store" }),
      ]);
      if (!scenarioResponse?.ok) {
        throw new Error(`setup catalog request failed: ${scenarioResponse?.status || "network"}`);
      }
      if (!mapResponse?.ok) {
        throw new Error(`map catalog request failed: ${mapResponse?.status || "network"}`);
      }
      const rows = await scenarioResponse.json();
      const mapCatalog = await mapResponse.json();
      this.entries = Array.isArray(rows)
        ? rows.map((entry) => normalizeLabScenarioEntry(entry)).filter((entry) => entry.id)
        : [];
      this.maps = Array.isArray(mapCatalog?.maps)
        ? mapCatalog.maps.map((entry) => normalizeLabMapEntry(entry)).filter(Boolean)
        : [];
      if (this.maps.length === 0) throw new Error("map catalog is empty");
      this.status = "";
    } catch (_) {
      this.entries = [];
      this.maps = [];
      this.error = "Setup catalog unavailable.";
      this.status = this.error;
    } finally {
      this.loading = false;
      this.render();
    }
  }

  render() {
    if (!this.root) return;
    const roomValue = this.currentRoom();
    this.root.replaceChildren();

    const shell = document.createElement("div");
    shell.className = "lab-entry-shell";

    const header = document.createElement("header");
    header.className = "lab-entry-header";
    const titleGroup = document.createElement("div");
    const kicker = document.createElement("span");
    kicker.className = "lobby-kicker";
    kicker.textContent = "Shared Lab";
    const title = document.createElement("h1");
    title.textContent = "Setup Catalog";
    titleGroup.append(kicker, title);
    const status = document.createElement("p");
    status.className = "lab-entry-status";
    status.dataset.state = this.error ? "error" : this.connected ? "ready" : "pending";
    status.textContent = this.loading ? "Loading" : this.status || (this.connected ? "Ready" : "Connecting");
    header.append(titleGroup, status);

    const controls = document.createElement("section");
    controls.className = "lab-entry-controls";
    controls.setAttribute("aria-label", "Lab launch settings");
    const roomLabel = document.createElement("label");
    roomLabel.textContent = "Room";
    this.roomInput = document.createElement("input");
    this.roomInput.type = "text";
    this.roomInput.maxLength = 40;
    this.roomInput.value = roomValue;
    this.roomInput.autocomplete = "off";
    roomLabel.appendChild(this.roomInput);
    controls.appendChild(roomLabel);

    const catalog = document.createElement("section");
    catalog.className = "lab-entry-list";
    catalog.setAttribute("aria-label", "Lab checkpoint setups");
    catalog.appendChild(this.renderNewLabRow());
    for (const entry of this.entries) catalog.appendChild(this.renderScenarioRow(entry));
    if (this.loading) catalog.appendChild(this.renderStateRow("Loading setups"));
    else if (this.error && this.entries.length === 0) {
      catalog.appendChild(this.renderStateRow(this.error));
    }

    shell.append(header, controls, catalog);
    this.root.appendChild(shell);
  }

  renderNewLabRow() {
    const selectedMap = this.currentMap();
    const row = this.renderRow({
      title: "New Lab",
      description: "Start an empty Lab on a bundled map.",
      action: "Start lab",
      disabled: this.maps.length === 0,
      onStart: () => this.start({ scenario: "blank", map: this.currentMap() }),
    });
    const picker = document.createElement("label");
    picker.className = "lab-entry-map-picker";
    const label = document.createElement("span");
    label.textContent = "Map";
    this.mapSelect = document.createElement("select");
    this.mapSelect.setAttribute("aria-label", "New Lab map");
    for (const entry of this.maps) {
      const option = document.createElement("option");
      option.value = entry.name;
      option.textContent = entry.name;
      option.selected = entry.name === selectedMap;
      this.mapSelect.appendChild(option);
    }
    this.mapSelect.disabled = this.maps.length === 0 || this.starting;
    picker.append(label, this.mapSelect);
    row.children[0]?.appendChild(picker);
    return row;
  }

  renderScenarioRow(entry) {
    return this.renderRow({
      title: entry.title,
      description: entry.description,
      action: "Start setup",
      onStart: () => this.start({ scenario: entry.id, map: entry.map }),
    });
  }

  renderRow({ title, description, action, disabled = false, onStart }) {
    const row = document.createElement("article");
    row.className = "lab-entry-row";
    const copy = document.createElement("div");
    copy.className = "lab-entry-copy";
    const heading = document.createElement("h2");
    heading.textContent = title;
    const body = document.createElement("p");
    body.textContent = description;
    copy.append(heading, body);

    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn primary";
    button.textContent = action;
    button.disabled = disabled || !this.connected || this.starting;
    button.addEventListener("click", onStart);
    row.append(copy, button);
    return row;
  }

  renderStateRow(text) {
    const row = document.createElement("p");
    row.className = "lab-entry-message";
    row.textContent = text;
    return row;
  }

  start({ scenario, map }) {
    if (!this.connected || this.starting) return;
    const selection = {
      room: this.currentRoom(),
      map,
      scenario,
    };
    this.starting = true;
    this.status = "Starting lab";
    this.error = "";
    this.render();
    this.onStart?.(selection);
  }

  currentRoom() {
    return this.roomInput?.value || this.initialRoom || "default";
  }

  currentMap() {
    const selected = this.mapSelect?.value;
    if (this.maps.some((entry) => entry.name === selected)) return selected;
    if (this.maps.some((entry) => entry.name === DEFAULT_LAB_MAP)) return DEFAULT_LAB_MAP;
    return this.maps[0]?.name || DEFAULT_LAB_MAP;
  }
}

function safeCatalogText(value, fallback) {
  const text = String(value || "").trim();
  return text || fallback;
}

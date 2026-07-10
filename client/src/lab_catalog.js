const LAB_CATALOG_ENDPOINT = "/api/lab-scenarios";
const DEFAULT_LAB_MAP = "Default";
const BLANK_LAB_MAP = "No Terrain";

export function normalizeLabScenarioEntry(entry) {
  const id = safeCatalogText(entry?.id, "");
  const title = safeCatalogText(entry?.title, id || "Setup");
  const description = safeCatalogText(entry?.description, "");
  const map = safeCatalogText(entry?.map, DEFAULT_LAB_MAP);
  const playerCount = Math.max(0, Math.trunc(Number(entry?.playerCount) || 0));
  const tags = Array.isArray(entry?.tags)
    ? entry.tags.map((tag) => safeCatalogText(tag, "")).filter(Boolean).slice(0, 8)
    : [];
  return {
    id,
    title,
    description,
    map,
    playerCount,
    tags,
  };
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
    this.status = "";
    this.error = "";
    this.connected = false;
    this.loading = false;
    this.roomInput = null;
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
      const response = await this.fetchImpl(LAB_CATALOG_ENDPOINT, { cache: "no-store" });
      if (!response?.ok) throw new Error(`catalog request failed: ${response?.status || "network"}`);
      const rows = await response.json();
      this.entries = Array.isArray(rows)
        ? rows.map((entry) => normalizeLabScenarioEntry(entry)).filter((entry) => entry.id)
        : [];
      this.status = "";
    } catch (_) {
      this.entries = [];
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
    catalog.appendChild(this.renderBlankRow());
    for (const entry of this.entries) catalog.appendChild(this.renderScenarioRow(entry));
    if (this.loading) catalog.appendChild(this.renderStateRow("Loading setups"));
    else if (this.error && this.entries.length === 0) {
      catalog.appendChild(this.renderStateRow(this.error));
    }

    shell.append(header, controls, catalog);
    this.root.appendChild(shell);
  }

  renderBlankRow() {
    return this.renderRow({
      title: "Blank Lab",
      description: "No-terrain map with the normal two-player lab setup.",
      map: BLANK_LAB_MAP,
      playerCount: 2,
      tags: ["blank"],
      action: "Start blank",
      onStart: () => this.start({ scenario: "blank", map: BLANK_LAB_MAP }),
    });
  }

  renderScenarioRow(entry) {
    return this.renderRow({
      title: entry.title,
      description: entry.description,
      map: entry.map,
      playerCount: entry.playerCount,
      tags: entry.tags,
      action: "Start setup",
      onStart: () => this.start({ scenario: entry.id, map: entry.map }),
    });
  }

  renderRow({ title, description, map, playerCount, tags, action, onStart }) {
    const row = document.createElement("article");
    row.className = "lab-entry-row";
    const copy = document.createElement("div");
    copy.className = "lab-entry-copy";
    const heading = document.createElement("h2");
    heading.textContent = title;
    const body = document.createElement("p");
    body.textContent = description;
    const meta = document.createElement("div");
    meta.className = "lab-entry-meta";
    meta.append(
      this.metaChip(map || DEFAULT_LAB_MAP),
      this.metaChip(`${playerCount || 2} players`),
      ...tags.map((tag) => this.metaChip(tag)),
    );
    copy.append(heading, body, meta);

    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn primary";
    button.textContent = action;
    button.disabled = !this.connected || this.starting;
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

  metaChip(text) {
    const chip = document.createElement("span");
    chip.textContent = text;
    return chip;
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
}

function safeCatalogText(value, fallback) {
  const text = String(value || "").trim();
  return text || fallback;
}

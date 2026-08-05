// Custom lobby map selector with bundled minimap previews.
// Lobby owns authoritative selection state; guests can browse previews but only hosts may select.

export const LOBBY_MAP_PRESENTATION = Object.freeze({
  "1v1 No Terrain": Object.freeze({
    author: "Alex",
    preview: "/assets/map-previews/1v1-no-terrain.jpg",
  }),
  "1v1": Object.freeze({
    author: "Alex",
    preview: "/assets/map-previews/1v1.jpg",
  }),
  "3 Player Map": Object.freeze({
    author: "Alex",
    preview: "/assets/map-previews/3-player-map.jpg",
  }),
  "4 Player Map": Object.freeze({
    author: "Alex",
    preview: "/assets/map-previews/4-player-map.jpg",
  }),
  Chokes: Object.freeze({
    author: "Alex",
    preview: "/assets/map-previews/chokes.jpg",
  }),
  Crossroads: Object.freeze({
    author: "Alex",
    preview: "/assets/map-previews/crossroads.jpg",
  }),
  "Open Basin": Object.freeze({
    author: "Alex",
    preview: "/assets/map-previews/open-basin.jpg",
  }),
  "Schone Tage": Object.freeze({
    author: "oti",
    preview: "/assets/map-previews/schone-tage.jpg",
  }),
});

export function lobbyMapPresentation(name) {
  return LOBBY_MAP_PRESENTATION[String(name || "")] || Object.freeze({
    author: "Unknown",
    preview: "",
  });
}

export class LobbyMapSelector {
  constructor(root, { onSelect = () => {} } = {}) {
    this.root = root;
    this.onSelect = onSelect;
    this.maps = [];
    this.selectedMap = "";
    this.disabled = true;
    this.readOnly = false;
    this.optionButtons = [];
    this.catalogKey = "";

    this._onDocumentPointerDown = (event) => {
      if (!this.root?.contains?.(event.target)) this.close();
    };
    this._onRootKeyDown = (event) => this._handleKeyDown(event);

    if (!this.root || typeof document === "undefined") return;
    this._build();
    document.addEventListener("pointerdown", this._onDocumentPointerDown);
    this.root.addEventListener("keydown", this._onRootKeyDown);
  }

  _build() {
    this.root.className = "lobby-map-selector";

    this.trigger = document.createElement("button");
    this.trigger.type = "button";
    this.trigger.id = "lobby-map-trigger";
    this.trigger.className = "lobby-map-trigger";
    this.trigger.setAttribute("aria-haspopup", "listbox");
    this.trigger.setAttribute("aria-controls", "lobby-map-options");
    this.trigger.setAttribute("aria-expanded", "false");
    this.trigger.addEventListener("click", () => {
      if (this.disabled) return;
      if (this.popover.hidden) this.open();
      else this.close();
    });

    this.triggerLabel = document.createElement("span");
    this.triggerLabel.className = "lobby-map-trigger-label";
    this.chevron = document.createElement("span");
    this.chevron.className = "lobby-map-chevron";
    this.chevron.setAttribute("aria-hidden", "true");
    this.trigger.append(this.triggerLabel, this.chevron);

    this.control = document.createElement("div");
    this.control.className = "lobby-map-control";

    this.popover = document.createElement("div");
    this.popover.className = "lobby-map-popover";
    this.popover.hidden = true;

    this.optionList = document.createElement("div");
    this.optionList.id = "lobby-map-options";
    this.optionList.className = "lobby-map-options";
    this.optionList.setAttribute("role", "listbox");
    this.optionList.setAttribute("aria-label", "Available maps");

    this.previewFigure = document.createElement("figure");
    this.previewFigure.className = "lobby-map-preview";
    this.previewImage = document.createElement("img");
    this.previewImage.width = 512;
    this.previewImage.height = 512;
    this.previewImage.alt = "";
    this.previewImage.decoding = "async";
    this.previewImage.addEventListener("error", () => {
      this.previewImage.hidden = true;
      this.previewFallback.hidden = false;
    });
    this.previewFallback = document.createElement("div");
    this.previewFallback.className = "lobby-map-preview-fallback";
    this.previewFallback.textContent = "Preview unavailable";
    this.previewFallback.hidden = true;
    this.previewCaption = document.createElement("figcaption");
    this.previewName = document.createElement("strong");
    this.previewAuthor = document.createElement("span");
    this.previewCaption.append(this.previewName, this.previewAuthor);
    this.previewFigure.append(
      this.previewImage,
      this.previewFallback,
      this.previewCaption,
    );

    this.popover.appendChild(this.optionList);
    this.control.append(this.trigger, this.popover);
    this.root.replaceChildren(this.previewFigure, this.control);
  }

  render({ maps = [], selectedMap = "", visible = false, disabled = false, readOnly = false } = {}) {
    if (!this.root || !this.trigger) return;
    this.maps = Array.isArray(maps) ? maps.filter((entry) => String(entry?.name || "")) : [];
    this.selectedMap = String(selectedMap || this.maps[0]?.name || "");
    this.disabled = !!disabled;
    this.readOnly = !!readOnly;
    this.root.hidden = !visible;
    this.root.classList.toggle("is-readonly", this.readOnly);
    this.trigger.disabled = this.disabled;
    this.trigger.setAttribute("aria-readonly", this.readOnly ? "true" : "false");
    this.trigger.title = this.readOnly ? "Browse maps — only the host can change the selection" : "";
    this.triggerLabel.textContent = this.selectedMap || "Select map";

    const nextCatalogKey = this.maps.map((entry) => entry.name).join("\u0000");
    if (nextCatalogKey !== this.catalogKey) {
      this.catalogKey = nextCatalogKey;
      this._rebuildOptions();
    }
    this._reflectSelection();
    this.preview(this.selectedMap);
    if (!visible || this.disabled) this.close();
  }

  _rebuildOptions() {
    this.optionButtons = [];
    const fragment = document.createDocumentFragment();
    for (const entry of this.maps) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "lobby-map-option";
      button.textContent = entry.name;
      button.dataset.mapName = entry.name;
      button.setAttribute("role", "option");
      button.setAttribute("tabindex", "-1");
      button.addEventListener("mouseenter", () => this.preview(entry.name));
      button.addEventListener("focus", () => this.preview(entry.name));
      button.addEventListener("click", () => this._select(entry.name));
      this.optionButtons.push(button);
      fragment.appendChild(button);
    }
    this.optionList.replaceChildren(fragment);
  }

  _reflectSelection() {
    for (const button of this.optionButtons) {
      const selected = button.dataset.mapName === this.selectedMap;
      button.classList.toggle("is-selected", selected);
      button.classList.toggle("is-unavailable", this.readOnly);
      button.setAttribute("aria-selected", selected ? "true" : "false");
      button.setAttribute("aria-disabled", this.readOnly ? "true" : "false");
      button.title = this.readOnly ? "Only the host can select this map" : "";
    }
  }

  preview(name) {
    if (!this.previewImage) return;
    const mapName = String(name || this.selectedMap || "Map");
    const presentation = lobbyMapPresentation(mapName);
    this.previewName.textContent = mapName;
    this.previewAuthor.textContent = `Created by ${presentation.author}`;
    this.previewImage.alt = `${mapName} minimap preview`;
    if (presentation.preview) {
      this.previewFallback.hidden = true;
      this.previewImage.hidden = false;
      this.previewImage.src = presentation.preview;
    } else {
      if (typeof this.previewImage.removeAttribute === "function") {
        this.previewImage.removeAttribute("src");
      } else {
        this.previewImage.src = "";
      }
      this.previewImage.hidden = true;
      this.previewFallback.hidden = false;
    }
  }

  open({ focus = false } = {}) {
    if (!this.popover || this.disabled || !this.root || this.root.hidden) return;
    this.popover.hidden = false;
    this.trigger.setAttribute("aria-expanded", "true");
    this.preview(this.selectedMap);
    if (focus) this._focusOption(this._selectedIndex());
  }

  close({ restoreFocus = false } = {}) {
    if (!this.popover) return;
    this.popover.hidden = true;
    this.trigger?.setAttribute("aria-expanded", "false");
    this.preview(this.selectedMap);
    if (restoreFocus) this.trigger?.focus();
  }

  clear() {
    this.maps = [];
    this.selectedMap = "";
    this.catalogKey = "";
    this.optionButtons = [];
    this.optionList?.replaceChildren();
    this.close();
  }

  _select(name) {
    if (this.disabled || this.readOnly || !this.maps.some((entry) => entry.name === name)) return;
    this.selectedMap = name;
    this.triggerLabel.textContent = name;
    this._reflectSelection();
    this.preview(name);
    this.onSelect(name);
    this.close({ restoreFocus: true });
  }

  _handleKeyDown(event) {
    if (this.disabled) return;
    const key = event.key;
    if (key === "Escape" && !this.popover.hidden) {
      event.preventDefault();
      this.close({ restoreFocus: true });
      return;
    }
    if (event.target === this.trigger) {
      if (key === "ArrowDown" || key === "ArrowUp" || key === "Enter" || key === " ") {
        event.preventDefault();
        this.open({ focus: true });
      }
      return;
    }
    const index = this.optionButtons.indexOf(event.target);
    if (index < 0) return;
    if (key === "ArrowDown") {
      event.preventDefault();
      this._focusOption(index + 1);
    } else if (key === "ArrowUp") {
      event.preventDefault();
      this._focusOption(index - 1);
    } else if (key === "Home" || key === "End") {
      event.preventDefault();
      this._focusOption(key === "Home" ? 0 : this.optionButtons.length - 1);
    } else if (key === "Enter" || key === " ") {
      event.preventDefault();
      this._select(event.target.dataset.mapName);
    }
  }

  _selectedIndex() {
    const index = this.optionButtons.findIndex((button) => button.dataset.mapName === this.selectedMap);
    return index < 0 ? 0 : index;
  }

  _focusOption(index) {
    if (!this.optionButtons.length) return;
    const wrapped = (index + this.optionButtons.length) % this.optionButtons.length;
    this.optionButtons[wrapped].focus();
  }

  destroy() {
    if (typeof document !== "undefined") {
      document.removeEventListener("pointerdown", this._onDocumentPointerDown);
    }
    this.root?.removeEventListener?.("keydown", this._onRootKeyDown);
    this.clear();
    this.onSelect = () => {};
  }
}

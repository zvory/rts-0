import {
  BASE_COMMAND_SUPPLY_CAP,
  COMMAND_CAR_SUPPLY_CAP_BONUS,
  ENTRENCHMENT_AREA_DAMAGE_REDUCTION,
  ENTRENCHMENT_DIG_IN_TICKS,
  ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION,
  ENTRENCHMENT_RANGE_BONUS_TILES,
  STATS,
  TICK_HZ,
  UPGRADES,
} from "./config.js";
import { KIND, UPGRADE } from "./protocol.js";

const SELECTION_BUDGET_ROWS = 4;
const SELECTION_BUDGET_BLOCK_ROWS = 2;
const SELECTION_BUDGET_COLS = Math.ceil(BASE_COMMAND_SUPPLY_CAP / SELECTION_BUDGET_BLOCK_ROWS);
const SELECTION_OVERFLOW_FLASH_MS = 1400;

export function selectionBudgetBlockShape(weight) {
  const safeWeight = Math.max(1, Math.ceil(Number.isFinite(weight) ? weight : 1));
  if (safeWeight === 1) return { cols: 1, rows: 1 };
  if (safeWeight === 2) return { cols: 2, rows: 1 };
  if (safeWeight === 3) return { cols: 3, rows: 1 };
  if (safeWeight === 4) return { cols: 2, rows: 2 };
  if (safeWeight === 5) return { cols: 3, rows: 2, reservedCells: 1 };
  if (safeWeight === 6) return { cols: 3, rows: 2 };
  return {
    cols: Math.ceil(safeWeight / SELECTION_BUDGET_BLOCK_ROWS),
    rows: SELECTION_BUDGET_BLOCK_ROWS,
  };
}

export function selectionBudgetGridModel(entities, overflow = null) {
  const budget = selectionBudgetForHudEntities(entities);
  const cols = SELECTION_BUDGET_COLS;
  const blocks = (entities || []).map((entity, sourceIndex) => {
    const weight = commandWeight(entity?.kind);
    const shape = selectionBudgetBlockShape(weight);
    const st = STATS[entity?.kind] || {};
    return {
      id: entity?.id,
      kind: entity?.kind,
      owner: entity?.owner,
      icon: st.icon || "",
      label: st.label || entity?.kind || "",
      weight,
      cols: shape.cols,
      rows: shape.rows,
      reservedCells: shape.reservedCells || 0,
      sourceIndex,
      page: 0,
      col: 1,
      row: 1,
      placed: false,
    };
  });
  const pages = blocks.length > 0 ? [createSelectionBudgetPage(cols, 0)] : [];
  const seenKinds = new Set();
  const representatives = [];
  const duplicates = [];
  for (const block of blocks) {
    if (!seenKinds.has(block.kind)) {
      seenKinds.add(block.kind);
      block.representative = true;
      representatives.push(block);
    } else {
      block.representative = false;
      duplicates.push(block);
    }
  }

  // Seed the first page with one of each selected kind before duplicates. Sorting those
  // representatives largest-first preserves contiguous room for wide/two-row portraits.
  // If they do not all fit, keep packing them together on subsequent pages before duplicates.
  for (const block of representatives.sort(compareSelectionBudgetBlocks)) {
    placeSelectionBlockOnPages(block, pages, cols);
  }
  for (const block of duplicates.sort(compareSelectionBudgetBlocks)) {
    placeSelectionBlockOnPages(block, pages, cols);
  }

  return {
    used: budget.used,
    cap: budget.cap,
    cols,
    rows: SELECTION_BUDGET_ROWS,
    blocks,
    pages: pages.map(({ index, used, blocks: pageBlocks }) => ({
      index,
      used,
      blocks: pageBlocks.sort((a, b) => a.sourceIndex - b.sourceIndex),
    })),
    overflow: overflow || null,
  };
}

export class HudSelectionPanel {
  constructor(panel, state, controlPolicy = null, unitIconMarkupForKind = null) {
    this.panel = panel;
    this.state = state;
    this.controlPolicy = controlPolicy;
    this.unitIconMarkupForKind = unitIconMarkupForKind;
    this._renderSig = null;
    this._selectionOverflowSig = null;
    this._selectionOverflowUntil = 0;
    this._activePage = 0;
    this._pageSelectionSig = null;
    this._lastFrameViews = null;
    this._onClick = (ev) => this._handleClick(ev);
    this._onContextMenu = (ev) => this._handleContextMenu(ev);
    if (this.panel && typeof this.panel.addEventListener === "function") {
      this.panel.addEventListener("click", this._onClick);
      this.panel.addEventListener("contextmenu", this._onContextMenu);
    }
  }

  destroy() {
    if (this.panel && typeof this.panel.removeEventListener === "function") {
      this.panel.removeEventListener("click", this._onClick);
      this.panel.removeEventListener("contextmenu", this._onContextMenu);
    }
    if (this.panel) this.panel.innerHTML = "";
    this._renderSig = null;
    this._pageSelectionSig = null;
    this._lastFrameViews = null;
  }

  /** Render the selection summary: single-entity detail or multi-entity command budget grid. */
  render(frameViews = null, { profiler = null } = {}) {
    this._profiler = profiler || null;
    this._lastFrameViews = frameViews;
    const panel = this.panel;
    if (!panel) return;

    const sel = Array.isArray(frameViews?.selectedEntities)
      ? frameViews.selectedEntities
      : typeof this.state?.selectedEntities === "function"
      ? this.state.selectedEntities()
      : [];
    if (!sel || sel.length === 0) {
      if (this._renderSig === "empty") {
        this._recordSelectionDiagnostic("hud.dirty.selectionPanel.hit");
        return;
      }
      this._recordSelectionDiagnostic("hud.dirty.selectionPanel.miss");
      this._renderSig = "empty";
      this._activePage = 0;
      this._pageSelectionSig = null;
      panel.innerHTML = "";
      return;
    }

    if (sel.length === 1) {
      this._activePage = 0;
      this._pageSelectionSig = null;
      const sig = selectionPanelSignature(sel, null, this.state);
      if (sig === this._renderSig) {
        this._recordSelectionDiagnostic("hud.dirty.selectionPanel.hit");
        return;
      }
      this._recordSelectionDiagnostic("hud.dirty.selectionPanel.miss");
      this._renderSig = sig;
      panel.innerHTML = "";
      panel.appendChild(this._selectionEntityNode(this._singleSelectionNode(sel[0]), sel[0]));
      return;
    }

    const overflow = this._visibleSelectionOverflow();
    const pageSelectionSig = sel
      .map((entity) => `${sigValue(entity?.id)}:${sigValue(entity?.kind)}`)
      .join("|");
    if (pageSelectionSig !== this._pageSelectionSig) {
      this._pageSelectionSig = pageSelectionSig;
      this._activePage = 0;
    }
    const sig = `${selectionPanelSignature(sel, overflow, this.state)}|page:${this._activePage}`;
    if (sig === this._renderSig) {
      this._recordSelectionDiagnostic("hud.dirty.selectionPanel.hit");
      return;
    }
    this._recordSelectionDiagnostic("hud.dirty.selectionPanel.miss");
    this._renderSig = sig;
    const model = selectionBudgetGridModel(sel, overflow);

    const frag = document.createDocumentFragment();
    const header = document.createElement("div");
    header.className = "sel-header";
    const count = document.createElement("span");
    count.className = "sel-count-label";
    count.textContent = `${sel.length} selected`;
    const budget = document.createElement("span");
    budget.className = "sel-budget-counter" + (overflow ? " overflow" : "");
    budget.textContent = `${model.used} / ${model.cap}`;
    header.appendChild(count);
    header.appendChild(budget);
    frag.appendChild(header);

    if (model.pages.length > 1) {
      const tabs = document.createElement("div");
      tabs.className = "sel-page-tabs";
      tabs.setAttribute("role", "group");
      tabs.setAttribute("aria-label", "Selected unit pages");
      for (const page of model.pages) {
        const tab = document.createElement("button");
        const active = page.index === this._activePage;
        tab.type = "button";
        tab.className = `sel-page-tab${active ? " active" : ""}`;
        tab.textContent = String(page.index + 1);
        tab.title = `Selection page ${page.index + 1}: ${page.blocks.length} units, ${page.used} command supply`;
        tab.setAttribute("data-selection-page", String(page.index));
        tab.setAttribute("aria-pressed", active ? "true" : "false");
        tab.setAttribute("aria-label", `Selection page ${page.index + 1} of ${model.pages.length}`);
        tabs.appendChild(tab);
      }
      frag.appendChild(tabs);
    }

    const activePage = model.pages[this._activePage] || model.pages[0];
    const grid = document.createElement("div");
    grid.className = "sel-budget-grid";
    grid.style.setProperty("--sel-budget-cols", String(model.cols));
    grid.style.setProperty("--sel-budget-rows", String(model.rows));
    grid.setAttribute(
      "aria-label",
      `Selection page ${this._activePage + 1} of ${model.pages.length}; command supply ${model.used} of ${model.cap}`,
    );
    for (const block of activePage?.blocks || []) {
      const cell = document.createElement("button");
      cell.type = "button";
      cell.className = [
        "sel-budget-block",
        `weight-${block.weight}`,
        block.reservedCells ? "has-reserved-cell" : "",
        block.placed ? "" : "unplaced",
      ].filter(Boolean).join(" ");
      cell.style.gridColumn = `${block.col} / span ${block.cols}`;
      cell.style.gridRow = `${block.row} / span ${block.rows}`;
      cell.title = `${block.label}: ${block.weight} command supply`;
      const teamColor = this.state?.playerById?.(block.owner)?.color ||
        this.state?.players?.find?.((player) =>
          Number(player?.id) === Number(block.owner))?.color ||
        "#0072b2";
      const unitIconMarkup = this.unitIconMarkupForKind?.(block.kind, { teamColor }) || "";
      if (unitIconMarkup) {
        cell.className += " has-unit-render-icon";
        cell.innerHTML = unitIconMarkup;
      } else {
        cell.textContent = block.icon;
      }
      this._selectionEntityNode(cell, block);
      grid.appendChild(cell);
    }
    frag.appendChild(grid);
    if (overflow) {
      const notice = document.createElement("div");
      notice.className = "sel-budget-overflow";
      notice.textContent = "Selection limit reached";
      frag.appendChild(notice);
    }

    panel.innerHTML = "";
    panel.appendChild(frag);
  }

  _recordSelectionDiagnostic(label, amount = 1) {
    this._profiler?.recordDiagnosticCounter?.(label, amount);
  }

  _handleClick(ev) {
    const pageNode = ev?.target && typeof ev.target.closest === "function"
      ? ev.target.closest("[data-selection-page]")
      : null;
    if (pageNode && (!this.panel?.contains || this.panel.contains(pageNode))) {
      const page = Number(pageNode.getAttribute("data-selection-page"));
      if (Number.isInteger(page) && page >= 0 && page !== this._activePage) {
        ev.preventDefault?.();
        this._activePage = page;
        this._renderSig = null;
        this.render(this._lastFrameViews, { profiler: this._profiler });
      }
      return;
    }
    const node = this._eventNode(ev);
    if (!node) return;
    ev.preventDefault?.();
    const id = Number(node.getAttribute("data-selection-entity-id"));
    this._applySelectionClick(id, {
      shiftKey: !!ev.shiftKey,
      ctrlKey: !!ev.ctrlKey,
      metaKey: !!ev.metaKey,
    });
  }

  _handleContextMenu(ev) {
    if (!ev.ctrlKey && !ev.metaKey) return;
    const node = this._eventNode(ev);
    if (!node) return;
    ev.preventDefault?.();
    const id = Number(node.getAttribute("data-selection-entity-id"));
    this._applySelectionClick(id, { ctrlKey: true });
  }

  _eventNode(ev) {
    const target = ev?.target;
    const node = target && typeof target.closest === "function"
      ? target.closest("[data-selection-entity-id]")
      : null;
    if (!node) return null;
    if (this.panel && typeof this.panel.contains === "function" && !this.panel.contains(node)) {
      return null;
    }
    return node;
  }

  _applySelectionClick(id, modifiers = {}) {
    if (!Number.isInteger(id) || !this.state) return;
    const entity = typeof this.state.entityById === "function"
      ? this.state.entityById(id)
      : null;
    if (!entity) return;

    if (modifiers.shiftKey && typeof this.state.removeFromSelection === "function") {
      this.state.removeFromSelection([id]);
      return;
    }

    if ((modifiers.ctrlKey || modifiers.metaKey) && typeof this.state.setSelection === "function") {
      const sel = typeof this.state.selectedEntities === "function"
        ? this.state.selectedEntities()
        : [];
      this.state.setSelection(
        sel.filter((e) => e.kind === entity.kind).map((e) => e.id),
        { controlPolicy: this.controlPolicy },
      );
      return;
    }

    if (typeof this.state.setSelection === "function") {
      this.state.setSelection([id], { controlPolicy: this.controlPolicy });
    }
  }

  _visibleSelectionOverflow() {
    const overflow = this.state?.selectionBudgetOverflow;
    if (!overflow) {
      this._selectionOverflowSig = null;
      this._selectionOverflowUntil = 0;
      return null;
    }

    const now = Date.now();
    const sig = `${overflow.used}/${overflow.cap}/${overflow.seq ?? ""}`;
    if (sig !== this._selectionOverflowSig) {
      this._selectionOverflowSig = sig;
      this._selectionOverflowUntil = now + SELECTION_OVERFLOW_FLASH_MS;
    }
    return now <= this._selectionOverflowUntil ? overflow : null;
  }

  /** Build the detail node for a single selected entity (icon, name, HP bar). */
  _singleSelectionNode(e) {
    const st = STATS[e.kind] || {};
    const node = document.createElement("div");
    node.className = "sel-single";

    const hp = e.hp ?? 0;
    const maxHp = e.maxHp ?? hp ?? 1;
    const frac = maxHp > 0 ? Math.max(0, Math.min(1, hp / maxHp)) : 0;
    const hpClass = frac > 0.5 ? "good" : frac > 0.25 ? "mid" : "low";

    let prodHtml = "";
    const queue = e.prodQueue ?? 0;
    if (queue > 0) {
      const pct = Math.round((e.prodProgress ?? 0) * 100);
      const kindLabel = (e.prodUpgrade && UPGRADES[e.prodUpgrade]?.label) ||
        (e.prodKind && STATS[e.prodKind] && STATS[e.prodKind].label) ||
        e.prodKind ||
        "";
      const queued = queue > 1 ? ` (+${queue - 1})` : "";
      const pending = e.prodWaiting
        ? ` <span class="sel-prod-waiting">waiting for resources / supply</span>`
        : e.optimisticProduction
          ? ` <span class="sel-prod-pending">pending</span>`
          : "";
      prodHtml =
        `<div class="sel-prod-label">${kindLabel}${queued}${pending}</div>` +
        `<div class="sel-prod-bar${e.optimisticProduction ? " optimistic" : ""}` +
        `${e.prodWaiting ? " waiting" : ""}">` +
        `<div class="sel-prod-fill" style="width:${pct}%"></div></div>`;
    }

    const resourceRemainingHtml = isResourceNodeKind(e.kind)
      ? `<div class="sel-stat sel-resource-remaining"><span>${st.label || e.kind} Remaining:</span>` +
        `<strong>${formatResourceRemaining(e.remaining)}</strong></div>`
      : "";
    const entrenchment = entrenchmentSelectionStatus(e, this.state);
    const entrenchmentHtml = entrenchment
      ? `<div class="sel-stat sel-trench-status"><span>${entrenchment.label}:</span>` +
        `<strong>${entrenchment.value}</strong></div>`
      : "";

    node.innerHTML =
      `<div class="sel-name"><span class="sel-icon">${st.icon || ""}</span>` +
      `${st.label || e.kind}</div>` +
      `<div class="sel-hpbar"><div class="sel-hpfill ${hpClass}" ` +
      `style="width:${(frac * 100).toFixed(0)}%"></div></div>` +
      `<div class="sel-hptext">${hp} / ${maxHp}</div>` +
      resourceRemainingHtml +
      entrenchmentHtml +
      prodHtml;
    return node;
  }

  _selectionEntityNode(node, entity) {
    node.setAttribute("data-selection-entity-id", String(entity.id));
    node.setAttribute("data-selection-kind", String(entity.kind || ""));
    node.setAttribute(
      "aria-label",
      `Selected ${entity.label || STATS[entity.kind]?.label || entity.kind || "entity"}`,
    );
    return node;
  }
}

export function entrenchmentSelectionStatus(entity, state = null) {
  if (!isEntrenchmentEligibleKind(entity?.kind)) return null;
  const occupiedTrenchId = Number(entity?.occupiedTrenchId);
  if (Number.isInteger(occupiedTrenchId) && occupiedTrenchId > 0) {
    return {
      label: "Trench",
      value: `Occupied: +${ENTRENCHMENT_RANGE_BONUS_TILES} range, ` +
        `-${percent(ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION)} direct, ` +
        `-${percent(ENTRENCHMENT_AREA_DAMAGE_REDUCTION)} blast`,
    };
  }

  if (canReportOwnResearch(entity, state) && playerHasEntrenchment(state)) {
    return {
      label: "Entrenchment",
      value: `Hold still ${formatDigInSeconds()}s to dig`,
    };
  }

  return {
    label: "Trench",
    value: "Can use existing trenches",
  };
}

function selectionPanelSignature(entities, overflow, state = null) {
  if (!entities || entities.length === 0) return "empty";
  if (entities.length === 1) return `single:${selectionDetailSignature(entities[0], state)}`;
  const selected = entities.map((entity) => selectionBudgetEntitySignature(entity, state)).join("|");
  const overflowSig = overflow
    ? `${numberSig(overflow.used)}:${numberSig(overflow.cap)}:${sigValue(overflow.seq)}`
    : "none";
  return `multi:${selected}|overflow:${overflowSig}`;
}

function selectionDetailSignature(entity, state = null) {
  if (!entity) return "missing";
  const productionPct = Math.round(clamp01(Number(entity.prodProgress) || 0) * 100);
  const entrenchment = entrenchmentSelectionStatus(entity, state);
  return [
    sigValue(entity.id),
    sigValue(entity.kind),
    sigValue(entity.label),
    sigValue(entity.hp),
    sigValue(entity.maxHp),
    sigValue(entity.occupiedTrenchId),
    entrenchment ? `${entrenchment.label}:${entrenchment.value}` : "",
    isResourceNodeKind(entity.kind) ? formatResourceRemaining(entity.remaining) : "",
    sigValue(entity.prodQueue),
    sigValue(entity.prodKind),
    sigValue(entity.prodUpgrade),
    productionPct,
    entity.prodWaiting ? 1 : 0,
    entity.optimisticProduction ? 1 : 0,
  ].join(":");
}

function isResourceNodeKind(kind) {
  return kind === KIND.STEEL || kind === KIND.OIL;
}

function formatResourceRemaining(value) {
  const remaining = Number(value);
  return Number.isFinite(remaining) ? String(Math.max(0, Math.round(remaining))) : "0";
}

function isEntrenchmentEligibleKind(kind) {
  return kind === KIND.RIFLEMAN || kind === KIND.PANZERFAUST || kind === KIND.MACHINE_GUNNER;
}

function canReportOwnResearch(entity, state) {
  if (!state || entity?.owner == null) return false;
  if (typeof state.isOwnOwner === "function") return state.isOwnOwner(entity.owner);
  return Number(entity.owner) === Number(state.playerId);
}

function playerHasEntrenchment(state) {
  return Array.isArray(state?.upgrades) && state.upgrades.includes(UPGRADE.ENTRENCHMENT);
}

function formatDigInSeconds() {
  const seconds = TICK_HZ > 0 ? ENTRENCHMENT_DIG_IN_TICKS / TICK_HZ : 0;
  return Number.isInteger(seconds) ? String(seconds) : seconds.toFixed(1);
}

function percent(value) {
  return `${Math.round(value * 100)}%`;
}

function selectionBudgetEntitySignature(entity, state = null) {
  if (!entity) return "missing";
  const ownerColor = state?.playerById?.(entity.owner)?.color ||
    state?.players?.find?.((player) =>
      Number(player?.id) === Number(entity.owner))?.color;
  return [
    sigValue(entity.id),
    sigValue(entity.kind),
    sigValue(entity.label),
    sigValue(entity.owner),
    sigValue(ownerColor),
  ].join(":");
}

function numberSig(value, digits = 0) {
  const number = Number(value);
  if (!Number.isFinite(number)) return "";
  return digits > 0 ? number.toFixed(digits) : String(Math.round(number));
}

function sigValue(value) {
  return value == null ? "" : String(value);
}

function clamp01(value) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, value));
}

function selectionBudgetForHudEntities(entities) {
  let used = 0;
  let cap = BASE_COMMAND_SUPPLY_CAP;
  for (const entity of entities || []) {
    if (!entity) continue;
    const weight = commandWeight(entity.kind);
    used += weight;
    if (entity.kind === KIND.COMMAND_CAR) cap += COMMAND_CAR_SUPPLY_CAP_BONUS + weight;
  }
  return { used, cap, over: used > cap };
}

function commandWeight(kind) {
  const supply = STATS[kind]?.supply;
  return Number.isFinite(supply) && supply > 0 ? supply : 1;
}

function compareSelectionBudgetBlocks(a, b) {
  return (b.rows * b.cols) - (a.rows * a.cols) ||
    b.rows - a.rows ||
    b.cols - a.cols ||
    a.sourceIndex - b.sourceIndex;
}

function placeSelectionBlockOnPages(block, pages, cols) {
  const shape = { cols: block.cols, rows: block.rows };
  let placement = null;
  let page = null;
  for (const candidate of pages) {
    const placed = placeSelectionBudgetBlock(candidate.occupied, cols, shape);
    if (!placed) continue;
    placement = placed;
    page = candidate;
    break;
  }
  if (!page) {
    page = createSelectionBudgetPage(cols, pages.length);
    pages.push(page);
    placement = placeSelectionBudgetBlock(page.occupied, cols, shape);
  }
  block.page = page.index;
  block.col = placement?.col ?? 1;
  block.row = placement?.row ?? 1;
  block.placed = !!placement;
  page.blocks.push(block);
  page.used += block.weight;
}

function createSelectionBudgetPage(cols, index) {
  return {
    index,
    used: 0,
    blocks: [],
    occupied: Array.from(
      { length: SELECTION_BUDGET_ROWS },
      () => Array(cols).fill(false),
    ),
  };
}

function placeSelectionBudgetBlock(occupied, maxCols, shape) {
  for (let row = 0; row <= SELECTION_BUDGET_ROWS - shape.rows; row++) {
    for (let col = 0; col <= maxCols - shape.cols; col++) {
      if (selectionBudgetSpaceFree(occupied, row, col, shape)) {
        for (let dy = 0; dy < shape.rows; dy++) {
          for (let dx = 0; dx < shape.cols; dx++) occupied[row + dy][col + dx] = true;
        }
        return { row: row + 1, col: col + 1 };
      }
    }
  }
  return null;
}

function selectionBudgetSpaceFree(occupied, row, col, shape) {
  for (let dy = 0; dy < shape.rows; dy++) {
    for (let dx = 0; dx < shape.cols; dx++) {
      if (occupied[row + dy]?.[col + dx]) return false;
    }
  }
  return true;
}

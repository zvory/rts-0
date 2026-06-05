// HUD — the DOM overlay for the game screen: resource/supply bar, the selected-units
// panel, and the context-sensitive command card (unit actions, worker build submenu,
// train buttons for production buildings). See DESIGN.md §4.1 (HUD) and §5 for balance.
//
// The HUD is plain DOM (not Pixi). It is rebuilt cheaply each frame from `state`; the
// only stateful trick is reusing command-card buttons between frames so that holding a
// stable selection does not thrash the DOM (and so hotkeys keep working). All gameplay
// effects go through `net.command(...)` or `state.beginPlacement(...)` — the HUD never
// mutates game state directly.

import { cmd } from "./protocol.js";
import { KIND, STATE, isBuilding, isUnit } from "./protocol.js";
import { STATS, WORKER_BUILDABLE, PLAYER_PALETTE } from "./config.js";

// Command-card hotkeys follow the keyboard grid (3 columns):
//   Q W E
//   A S D
//   Z X C
const GRID_HOTKEYS = Object.freeze(["Q", "W", "E", "A", "S", "D", "Z", "X", "C"]);

/** True if `playerId` owns at least one completed entity of `kind` in `entities`. */
export function playerHasCompletedKind(entities, playerId, kind) {
  for (const e of entities || []) {
    if (e.owner === playerId && e.kind === kind && e.buildProgress == null) return true;
  }
  return false;
}

/** Format tank lifetime movement oil for the selected-entity detail panel. */
export function formatTankOilUsed(value) {
  const oilUsed = typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, value)
    : 0;
  return oilUsed >= 10 ? `${Math.round(oilUsed)}` : oilUsed.toFixed(1);
}

/**
 * The bottom/top DOM HUD: resources, selected panel, and the command card.
 *
 * Wiring (main.js) constructs one HUD and calls `update()` once per rendered frame.
 */
export class HUD {
  /**
   * @param {HTMLElement} rootEl the game screen root (`#game-screen`); used to scope
   *   element lookups so multiple screens could coexist.
   * @param {import("./state.js").GameState} state shared game state (selection, resources).
   * @param {import("./net.js").Net} net network seam for issuing commands.
   */
  constructor(rootEl, state, net) {
    this.root = rootEl;
    this.state = state;
    this.net = net;

    // Resource / supply bar elements.
    this.elHud = rootEl.querySelector("#hud");
    this.elSteel = rootEl.querySelector("#res-steel");
    this.elOil = rootEl.querySelector("#res-oil");
    this.elSupply = rootEl.querySelector("#res-supply");

    // Selected-units panel + command card containers.
    this.elSelected = rootEl.querySelector("#selected-panel");
    this.elControlGroups = rootEl.querySelector("#control-group-tabs");
    this.elCommand = rootEl.querySelector("#command-card");

    // Signature of the last-rendered command card so we only rebuild its buttons when
    // the relevant selection/affordability actually changes (keeps DOM + hotkeys stable).
    this._cardSig = null;
    // Next producer index by selected eligible building ids, used to spread train clicks.
    this._trainRoundRobin = new Map();
    // Signature for the resource bar to avoid unnecessary DOM rebuilds.
    this._resSig = null;
    // Signature for the inert control-group tabs.
    this._controlGroupSig = null;
  }

  /**
   * Refresh the entire HUD from the latest snapshot/selection. Cheap and idempotent;
   * safe to call every frame.
   */
  update() {
    this._renderResources();
    this._renderControlGroupTabs();
    this._renderSelectedPanel();
    this._renderCommandCard();
  }

  /** Clear DOM-owned HUD state between matches. */
  destroy() {
    if (this.elSelected) {
      this.elSelected.innerHTML = "";
    }
    if (this.elControlGroups) {
      this.elControlGroups.innerHTML = "";
      this.elControlGroups.classList.add("empty");
    }
    if (this.elCommand) this.elCommand.innerHTML = "";
    if (this.elSupply) this.elSupply.classList.remove("supply-capped");
    this._cardSig = null;
    this._trainRoundRobin.clear();
    this._resSig = null;
    this._controlGroupSig = null;
  }

  // --- Resource / supply bar -------------------------------------------------

  /** Mirror `state.resources` into the top bar, or all players' resources in replay mode. */
  _renderResources() {
    const pr = this.state.playerResources;
    if (pr && pr.length > 0) {
      this._renderAllPlayersResources(pr);
    } else {
      this._renderSinglePlayerResources();
    }
  }

  _renderSinglePlayerResources() {
    const r = this.state.resources || { steel: 0, oil: 0, supplyUsed: 0, supplyCap: 0 };
    // Restore static HUD content if we previously switched to multi-player mode.
    if (this._resSig && this._resSig.startsWith("multi:")) {
      if (this.elHud) {
        this.elHud.innerHTML =
          `<div class="res"><span class="res-icon steel">▰</span><span id="res-steel">0</span></div>` +
          `<div class="res"><span class="res-icon oil">⬤</span><span id="res-oil">0</span></div>` +
          `<div class="res"><span class="res-icon supply">▲</span><span id="res-supply">0 / 0</span></div>`;
        this.elSteel = this.elHud.querySelector("#res-steel");
        this.elOil = this.elHud.querySelector("#res-oil");
        this.elSupply = this.elHud.querySelector("#res-supply");
      }
      this._resSig = null;
    }
    if (this.elSteel) this.elSteel.textContent = String(r.steel ?? 0);
    if (this.elOil) this.elOil.textContent = String(r.oil ?? 0);
    if (this.elSupply) {
      const used = r.supplyUsed ?? 0;
      const cap = r.supplyCap ?? 0;
      this.elSupply.textContent = `${used} / ${cap}`;
      // Flag over-cap (blocked production) so styles.css can color it.
      this.elSupply.classList.toggle("supply-capped", cap > 0 && used >= cap);
    }
  }

  /** Render one resource row per player, with a color-coded dot identifying each player. */
  _renderAllPlayersResources(playerResources) {
    if (!this.elHud) return;

    // Build a signature to avoid rebuilding every frame.
    const sig = "multi:" + playerResources.map(
      (p) => `${p.id}:${p.steel}:${p.oil}:${p.supplyUsed}:${p.supplyCap}`,
    ).join("|");
    if (sig === this._resSig) return;
    this._resSig = sig;

    const players = this.state.players || [];
    const frag = document.createDocumentFragment();
    for (const pr of playerResources) {
      // Look up this player's display color.
      const playerInfo = players.find((p) => p.id === pr.id);
      const idx = players.indexOf(playerInfo);
      const color = (playerInfo && playerInfo.color) || PLAYER_PALETTE[idx % PLAYER_PALETTE.length] || "#888";
      const name = (playerInfo && playerInfo.name) || `P${pr.id}`;
      const supplyCapped = pr.supplyCap > 0 && pr.supplyUsed >= pr.supplyCap;

      const row = document.createElement("div");
      row.className = "res replay-player-res";
      row.innerHTML =
        `<span class="replay-player-dot" style="background:${color}" title="${name}"></span>` +
        `<span class="res-icon steel">▰</span><span class="replay-res-val">${pr.steel}</span>` +
        `<span class="res-icon oil">⬤</span><span class="replay-res-val">${pr.oil}</span>` +
        `<span class="res-icon supply">▲</span>` +
        `<span class="replay-res-val${supplyCapped ? " supply-capped" : ""}">${pr.supplyUsed} / ${pr.supplyCap}</span>`;
      frag.appendChild(row);
    }
    this.elHud.innerHTML = "";
    this.elHud.appendChild(frag);
  }

  // --- Selected-units panel --------------------------------------------------

  /** Render fixed-position, non-clickable tabs for occupied local control groups. */
  _renderControlGroupTabs() {
    const tabs = this.elControlGroups;
    if (!tabs) return;

    const groups = this._controlGroupSummaries();
    const sig = groups.map((g) =>
      g ? `${g.key}:${g.count}:${g.icon}:${g.selected ? 1 : 0}` : "-",
    ).join("|");
    if (sig === this._controlGroupSig) return;
    this._controlGroupSig = sig;

    const any = groups.some(Boolean);
    tabs.classList.toggle("empty", !any);

    const frag = document.createDocumentFragment();
    for (const group of groups) {
      const slot = document.createElement("div");
      slot.className = "control-group-slot";
      if (group) {
        const tab = document.createElement("div");
        tab.className = "control-group-tab" + (group.selected ? " selected" : "");
        tab.setAttribute(
          "aria-label",
          `Control group ${group.key}: ${group.count} ${group.label}`,
        );
        tab.innerHTML =
          `<span class="control-group-key">${group.key}</span>` +
          `<span class="control-group-kind">${group.icon}</span>` +
          `<span class="control-group-count">${group.count}</span>`;
        slot.appendChild(tab);
      }
      frag.appendChild(slot);
    }

    tabs.innerHTML = "";
    tabs.appendChild(frag);
  }

  _controlGroupSummaries() {
    const selected = typeof this.state.selectedEntities === "function"
      ? this.state.selectedEntities()
      : [];
    const selectedIds = new Set(selected.map((e) => e.id));
    const selectedCount = selectedIds.size;
    const out = [];
    const groups = this.state.controlGroups || [];
    for (let slot = 0; slot < groups.length; slot++) {
      const entities = typeof this.state.controlGroupEntities === "function"
        ? this.state.controlGroupEntities(slot)
        : [];
      if (!entities || entities.length === 0) {
        out.push(null);
        continue;
      }
      const dominant = this._dominantControlGroupKind(entities);
      const st = STATS[dominant.kind] || {};
      out.push({
        key: slot === 9 ? "0" : String(slot + 1),
        count: entities.length,
        icon: st.icon || dominant.kind,
        label: st.label || dominant.kind,
        selected: this._controlGroupMatchesSelection(entities, selectedIds, selectedCount),
      });
    }
    return out;
  }

  _dominantControlGroupKind(entities) {
    const counts = new Map();
    let best = { kind: entities[0].kind, count: 0, first: 0 };
    for (let i = 0; i < entities.length; i++) {
      const kind = entities[i].kind;
      const entry = counts.get(kind) || { kind, count: 0, first: i };
      entry.count += 1;
      counts.set(kind, entry);
      if (
        entry.count > best.count ||
        (entry.count === best.count && entry.first < best.first)
      ) {
        best = entry;
      }
    }
    return best;
  }

  _controlGroupMatchesSelection(entities, selectedIds, selectedCount) {
    if (selectedCount === 0 || entities.length !== selectedCount) return false;
    for (const e of entities) {
      if (!selectedIds.has(e.id)) return false;
    }
    return true;
  }

  /**
   * Render the selection summary: for a single entity show its name + HP; for a
   * homogeneous group show the kind label and a count; for a mixed group list the
   * per-kind counts.
   */
  _renderSelectedPanel() {
    const panel = this.elSelected;
    if (!panel) return;

    const sel = this.state.selectedEntities();
    if (!sel || sel.length === 0) {
      panel.innerHTML = "";
      return;
    }

    if (sel.length === 1) {
      panel.innerHTML = "";
      panel.appendChild(this._singleSelectionNode(sel[0]));
      return;
    }

    // Multiple selected: header with total, then a grid of per-kind chips with counts.
    const counts = new Map();
    for (const e of sel) counts.set(e.kind, (counts.get(e.kind) || 0) + 1);

    const frag = document.createDocumentFragment();
    const header = document.createElement("div");
    header.className = "sel-header";
    header.textContent = `${sel.length} selected`;
    frag.appendChild(header);

    const grid = document.createElement("div");
    grid.className = "sel-grid";
    for (const [kind, count] of counts) {
      const chip = document.createElement("div");
      chip.className = "sel-chip";
      const st = STATS[kind] || {};
      chip.innerHTML =
        `<span class="sel-icon">${st.icon || ""}</span>` +
        `<span class="sel-label">${st.label || kind}</span>` +
        `<span class="sel-count">×${count}</span>`;
      grid.appendChild(chip);
    }
    frag.appendChild(grid);

    panel.innerHTML = "";
    panel.appendChild(frag);
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
      const kindLabel = (e.prodKind && STATS[e.prodKind] && STATS[e.prodKind].label) || e.prodKind || "";
      const queued = queue > 1 ? ` (+${queue - 1})` : "";
      prodHtml =
        `<div class="sel-prod-label">${kindLabel}${queued}</div>` +
        `<div class="sel-prod-bar"><div class="sel-prod-fill" style="width:${pct}%"></div></div>`;
    }

    const tankOilHtml = e.kind === KIND.TANK
      ? `<div class="sel-stat"><span>Oil Used:</span>` +
        `<strong>${formatTankOilUsed(e.oilUsed)}</strong></div>`
      : "";

    node.innerHTML =
      `<div class="sel-name"><span class="sel-icon">${st.icon || ""}</span>` +
      `${st.label || e.kind}</div>` +
      `<div class="sel-hpbar"><div class="sel-hpfill ${hpClass}" ` +
      `style="width:${(frac * 100).toFixed(0)}%"></div></div>` +
      `<div class="sel-hptext">${hp} / ${maxHp}</div>` +
      tankOilHtml +
      prodHtml;
    return node;
  }

  // --- Command card ----------------------------------------------------------

  /**
   * Render the context command card based on the current selection:
   *  - selected own units → action buttons for move / attack / stop.
   *  - a selected WORKER → unit action buttons plus a build-menu button.
   *  - worker build submenu → build buttons for WORKER_BUILDABLE plus a return button.
   *  - selected production buildings (have `STATS[kind].trains`) → train
   *    buttons for the primary producer's trainable units. Successive train clicks
   *    are distributed round-robin across the selected compatible producers. A
   *    cancel button is shown for the primary producer while producing in the
   *    bottom-right cell (`C` hotkey).
   *  - anything else → empty.
   *
   * Buttons are disabled when unaffordable (vs `state.resources`) or when tech
   * requirements are unmet (e.g. factory requires completed prerequisites).
   */
  _renderCommandCard() {
    const card = this.elCommand;
    if (!card) return;
    if (this.state.spectator) {
      if (this._cardSig !== "spectator") {
        card.innerHTML = "";
        this._cardSig = "spectator";
      }
      return;
    }

    const sel = this.state.selectedEntities() || [];
    const primary = this._commandSubject(sel);

    if (!primary) {
      this._renderEmptyCard(card);
      return;
    }

    if (this.state.commandCardMode === "workerBuild" && this._workerOnlySelection(sel)) {
      this._renderBuildCard(card);
    } else if (this._selectedOwnUnits(sel).length > 0) {
      this._renderUnitCard(card, sel);
    } else {
      this._renderTrainCard(card, primary);
    }
  }

  /** Render a stable, inert command-card grid when no actionable selection exists. */
  _renderEmptyCard(card) {
    if (this._cardSig === "empty") return;
    this._cardSig = "empty";

    const frag = document.createDocumentFragment();
    this._padCard(frag, 0);
    card.innerHTML = "";
    card.appendChild(frag);
  }

  /**
   * Choose the entity that drives the command card. We only show actions the player
   * owns and can act on: a worker (build) or a production building (train). Prefer a
   * production building if one is selected, else a worker.
   * @returns {object|null}
   */
  _commandSubject(sel) {
    let worker = null;
    for (const e of sel) {
      if (!this._isOwn(e)) continue;
      if (isUnit(e.kind)) return e;
      if (isBuilding(e.kind) && this._trainsOf(e.kind).length > 0) return e;
    }
    return worker;
  }

  _isOwn(e) {
    return e && e.owner === this.state.playerId;
  }

  /** The trainable unit kinds for a building kind (empty array if none). */
  _trainsOf(kind) {
    const st = STATS[kind];
    return (st && st.trains) || [];
  }

  /** Building/unit prerequisite kinds as an array. */
  _requirementsOf(st) {
    if (!st || !st.requires) return [];
    return Array.isArray(st.requires) ? st.requires : [st.requires];
  }

  /** Own selected entities that can receive unit commands. */
  _selectedOwnUnits(sel) {
    return sel.filter((e) => this._isOwn(e) && isUnit(e.kind));
  }

  _selectedOwnRiflemanIds(sel) {
    return this._selectedOwnUnits(sel)
      .filter((e) => e.kind === KIND.RIFLEMAN)
      .map((e) => e.id);
  }

  _selectedOwnReadyChargeRiflemanIds(sel) {
    return this._selectedOwnUnits(sel)
      .filter((e) => e.kind === KIND.RIFLEMAN && (e.chargeCooldownLeft || 0) === 0)
      .map((e) => e.id);
  }

  /** True when the actionable selected units are workers and no army unit is selected. */
  _workerOnlySelection(sel) {
    const ownUnits = this._selectedOwnUnits(sel);
    return ownUnits.length > 0 && ownUnits.every((e) => e.kind === KIND.WORKER);
  }

  // --- Unit card (units selected) ------------------------------------------

  _renderUnitCard(card, sel) {
    const ownUnits = this._selectedOwnUnits(sel);
    const unitIds = ownUnits.map((e) => e.id);
    const atGunIds = ownUnits.filter((e) => e.kind === KIND.AT_TEAM).map((e) => e.id);
    const riflemanIds = this._selectedOwnRiflemanIds(sel);
    const readyChargeRiflemanIds = this._selectedOwnReadyChargeRiflemanIds(sel);
    const chargeUnlocked =
      riflemanIds.length > 0 && this._playerHasCompleteKind(KIND.TRAINING_CENTRE);
    const chargeReadyCount = readyChargeRiflemanIds.length;
    const showChargeReadyCount = chargeUnlocked && chargeReadyCount < riflemanIds.length;
    const hasArmyUnit = ownUnits.some((e) => e.kind !== KIND.WORKER);
    const workerSelected = !hasArmyUnit && ownUnits.some((e) => e.kind === KIND.WORKER);

    const sig =
      `units|${unitIds.join(".")}|target:${this.state.commandTarget || ""}|` +
      `|at:${atGunIds.join(".")}|` +
      `|rifle:${riflemanIds.join(".")}|charge:${chargeUnlocked ? 1 : 0}:${chargeReadyCount}|` +
      (workerSelected ? "worker-main" : "no-build");
    if (sig === this._cardSig) return;
    this._cardSig = sig;

    const frag = document.createDocumentFragment();
    let idx = 0;

    if (workerSelected) {
      // Workers mirror the standard unit layout: Q=Move, A=Attack, S=Hold.
      frag.appendChild(this._cmdButton({
        icon: "MV",
        label: "Move",
        title: "Move to a target point",
        hotkey: GRID_HOTKEYS[0],
        enabled: unitIds.length > 0,
        cls: this.state.commandTarget === "move" ? "active" : "",
        onClick: () => this.state.beginCommandTarget("move"),
      }));
      for (let i = 0; i < 2; i++) {
        const el = document.createElement("div");
        el.className = "cmd-empty";
        frag.appendChild(el);
      }
      frag.appendChild(this._cmdButton({
        icon: "AT",
        label: "Attack",
        title: "Attack a target or attack-move to a point",
        hotkey: GRID_HOTKEYS[3],
        enabled: unitIds.length > 0,
        cls: this.state.commandTarget === "attack" ? "active" : "",
        onClick: () => this.state.beginCommandTarget("attack"),
      }));
      frag.appendChild(this._cmdButton({
        icon: "HD",
        label: "Hold",
        title: "Hold position / stop selected units",
        hotkey: GRID_HOTKEYS[4],
        enabled: unitIds.length > 0,
        cls: "",
        onClick: () => {
          this.net.command(cmd.stop(unitIds));
          this.state.endCommandTarget();
        },
      }));
      const empty = document.createElement("div");
      empty.className = "cmd-empty";
      frag.appendChild(empty);
      idx = 6;
      frag.appendChild(this._cmdButton({
        icon: "BLD",
        label: "Build",
        title: "Open worker build menu",
        hotkey: GRID_HOTKEYS[idx++],
        enabled: unitIds.length > 0,
        cls: "",
        onClick: () => this.state.openWorkerBuildMenu(),
      }));
    } else {
      // Non-worker units: Q=Move, W/E=empty, A=Attack, S=Hold.
      frag.appendChild(this._cmdButton({
        icon: "MV",
        label: "Move",
        title: "Move to a target point",
        hotkey: GRID_HOTKEYS[0],
        enabled: unitIds.length > 0,
        cls: this.state.commandTarget === "move" ? "active" : "",
        onClick: () => this.state.beginCommandTarget("move"),
      }));
      for (let i = 0; i < 2; i++) {
        const el = document.createElement("div");
        el.className = "cmd-empty";
        frag.appendChild(el);
      }
      frag.appendChild(this._cmdButton({
        icon: "AT",
        label: "Attack",
        title: "Attack a target or attack-move to a point",
        hotkey: GRID_HOTKEYS[3],
        enabled: unitIds.length > 0,
        cls: this.state.commandTarget === "attack" ? "active" : "",
        onClick: () => this.state.beginCommandTarget("attack"),
      }));
      frag.appendChild(this._cmdButton({
        icon: "HD",
        label: "Hold",
        title: "Hold position / stop selected units",
        hotkey: GRID_HOTKEYS[4],
        enabled: unitIds.length > 0,
        cls: "",
        onClick: () => {
          this.net.command(cmd.stop(unitIds));
          this.state.endCommandTarget();
        },
      }));
      idx = 5;
      if (chargeUnlocked || atGunIds.length > 0) {
        const empty = document.createElement("div");
        empty.className = "cmd-empty";
        frag.appendChild(empty);
        idx = 6;
      }
      if (chargeUnlocked) {
        frag.appendChild(this._cmdButton({
          icon: "CHG",
          label: "Charge",
          title: "Riflemen sprint briefly at double movement speed",
          hotkey: GRID_HOTKEYS[idx++],
          enabled: chargeReadyCount > 0,
          countBadge: showChargeReadyCount ? `${chargeReadyCount}` : "",
          cls: "",
          onClick: () => {
            this.net.command(cmd.charge(readyChargeRiflemanIds));
            this.state.endCommandTarget();
          },
        }));
      }
      if (atGunIds.length > 0) {
        frag.appendChild(this._cmdButton({
          icon: "SET",
          label: "Set Up",
          title: "Set up selected AT guns toward a target point",
          hotkey: GRID_HOTKEYS[idx++],
          enabled: true,
          cls: this.state.commandTarget === "setupAtGuns" ? "active" : "",
          onClick: () => this.state.beginCommandTarget("setupAtGuns"),
        }));
        frag.appendChild(this._cmdButton({
          icon: "TD",
          label: "Tear Down",
          title: "Pack up selected AT guns",
          hotkey: GRID_HOTKEYS[idx++],
          enabled: true,
          cls: "",
          onClick: () => {
            this.net.command(cmd.tearDownAtGuns(atGunIds));
            this.state.endCommandTarget();
          },
        }));
      }
    }

    this._padCard(frag, idx);
    card.innerHTML = "";
    card.appendChild(frag);
  }

  // --- Build card (worker selected) -----------------------------------------

  _renderBuildCard(card) {
    const res = this.state.resources || { steel: 0, oil: 0 };

    // Signature: which buttons exist + their enabled state. Rebuild only on change.
    const sig =
      "build|" +
      WORKER_BUILDABLE.map((k) => `${k}:${this._canBuild(k, res) ? 1 : 0}`).join(",");
    if (sig === this._cardSig) return;
    this._cardSig = sig;

    const frag = document.createDocumentFragment();
    let idx = 0;
    for (const kind of WORKER_BUILDABLE) {
      const st = STATS[kind];
      if (!st) continue;
      const enabled = this._canBuild(kind, res);
      const reason = this._buildDisabledReason(kind, res);
      const btn = this._cmdButton({
        icon: st.icon,
        label: st.label,
        hotkey: GRID_HOTKEYS[idx++],
        cost: st.cost,
        enabled,
        title: reason,
        onClick: () => this.state.beginPlacement(kind),
      });
      frag.appendChild(btn);
    }
    this._padCard(frag, idx, 8);
    frag.appendChild(this._cmdButton({
      icon: "RTN",
      label: "Worker",
      hotkey: GRID_HOTKEYS[8],
      enabled: true,
      title: "Return to worker commands",
      onClick: () => this.state.closeCommandCardMenu(),
    }));
    card.innerHTML = "";
    card.appendChild(frag);
  }

  /** A worker can build `kind` if affordable and all tech requirements are satisfied. */
  _canBuild(kind, res) {
    const st = STATS[kind];
    if (!st) return false;
    if (this._requirementsOf(st).some((req) => !this._playerHasCompleteKind(req))) return false;
    return this._affordable(st.cost, res);
  }

  /** Human-readable disabled reason for a build button tooltip ("" when enabled). */
  _buildDisabledReason(kind, res) {
    const st = STATS[kind];
    if (!st) return "";
    const missing = this._requirementsOf(st).find((req) => !this._playerHasCompleteKind(req));
    if (missing) {
      const reqLabel = (STATS[missing] && STATS[missing].label) || missing;
      return `Requires ${reqLabel}`;
    }
    if (!this._affordable(st.cost, res)) return "Not enough resources";
    return "";
  }

  // --- Train card (production building selected) -----------------------------

  _renderTrainCard(card, building) {
    const res = this.state.resources || { steel: 0, oil: 0 };
    const trains = this._trainsOf(building.kind);
    const producing = (building.prodQueue ?? 0) > 0 || building.state === STATE.TRAIN;
    const cancelSlot = 8;

    // Signature includes the building id (so switching buildings rebuilds), each
    // trainable unit's affordability, the selected compatible producer set for
    // each unit, and whether a cancel button is shown.
    const sig =
      `train|${building.id}|` +
      trains.map((u) => {
        const producerIds = this._selectedProducerBuildingsForUnit(u).map((e) => e.id).join(".");
        return `${u}:${this._canTrain(u, res) ? 1 : 0}:${producerIds}`;
      }).join(",") +
      `|cancel:${producing ? 1 : 0}`;
    if (sig === this._cardSig) return;
    this._cardSig = sig;

    const frag = document.createDocumentFragment();
    let idx = 0;
    for (const unit of trains) {
      const st = STATS[unit];
      if (!st) continue;
      const enabled = this._canTrain(unit, res);
      const btn = this._cmdButton({
        icon: st.icon,
        label: st.label,
        hotkey: GRID_HOTKEYS[idx++],
        cost: st.cost,
        enabled,
        title: this._trainDisabledReason(unit, res),
        onClick: () => this._issueTrain(unit),
      });
      frag.appendChild(btn);
    }

    if (producing) {
      // Fill any gap between the train buttons and the fixed cancel slot so
      // the card always renders as a complete 3x3 grid with C in slot 8.
      this._padCard(frag, idx, cancelSlot);
      const cancelBtn = this._cmdButton({
        icon: "CNCL",
        label: "Cancel",
        hotkey: GRID_HOTKEYS[cancelSlot],
        enabled: true,
        cls: "cancel",
        title: "Cancel current production",
        onClick: () => this.net.command(cmd.cancel(building.id)),
      });
      frag.appendChild(cancelBtn);
    } else {
      this._padCard(frag, idx);
    }

    card.innerHTML = "";
    card.appendChild(frag);
  }

  /** Selected own production buildings that can train `unit`, in selection order. */
  _selectedProducerBuildingsForUnit(unit) {
    const sel = this.state.selectedEntities() || [];
    return sel.filter(
      (e) =>
        this._isOwn(e) &&
        isBuilding(e.kind) &&
        e.buildProgress == null &&
        this._trainsOf(e.kind).includes(unit),
    );
  }

  /** Pick the next selected compatible producer for `unit` and advance its cursor. */
  _nextProducerBuildingForUnit(unit) {
    const producers = this._selectedProducerBuildingsForUnit(unit);
    if (producers.length === 0) return null;

    const key = producers.map((e) => e.id).join(".");
    const idx = (this._trainRoundRobin.get(key) || 0) % producers.length;
    this._trainRoundRobin.set(key, (idx + 1) % producers.length);
    return producers[idx];
  }

  /** Queue `unit` at the next selected compatible production building. */
  _issueTrain(unit) {
    const building = this._nextProducerBuildingForUnit(unit);
    if (!building) return;
    this.net.command(cmd.train(building.id, unit));
  }

  // --- Shared helpers --------------------------------------------------------

  /** True if `cost` ({steel,oil}) is affordable against `res` ({steel,oil}). */
  _affordable(cost, res) {
    if (!cost) return true;
    const steel = res.steel ?? 0;
    const oil = res.oil ?? 0;
    return steel >= (cost.steel ?? 0) && oil >= (cost.oil ?? 0);
  }

  /** A unit can be trained if affordable and its completed-building tech is present. */
  _canTrain(unit, res) {
    const st = STATS[unit];
    if (!st) return false;
    if (this._requirementsOf(st).some((req) => !this._playerHasCompleteKind(req))) return false;
    return this._affordable(st.cost, res);
  }

  /** Human-readable disabled reason for a train button tooltip ("" when enabled). */
  _trainDisabledReason(unit, res) {
    const st = STATS[unit];
    if (!st) return "";
    const missing = this._requirementsOf(st).find((req) => !this._playerHasCompleteKind(req));
    if (missing) {
      const reqLabel = (STATS[missing] && STATS[missing].label) || missing;
      return `Requires ${reqLabel}`;
    }
    if (!this._affordable(st.cost, res)) return "Not enough resources";
    return "";
  }

  /** True if the player owns at least one completed entity of `kind`. */
  _playerHasCompleteKind(kind) {
    // entitiesInterpolated(1) reflects the latest snapshot positions but, more
    // importantly here, the latest set of entities.
    return playerHasCompletedKind(this.state.entitiesInterpolated(1), this.state.playerId, kind);
  }

  /**
   * Pad a command-card fragment with empty placeholders up to `target` slots
   * (default 9 — a full 3x3 grid). Use a smaller target to reserve trailing
   * slots for fixed-position buttons (e.g. cancel in slot 8).
   * @param {DocumentFragment} frag
   * @param {number} filled number of real buttons already appended.
   * @param {number} [target=9] desired total slot count after padding.
   */
  _padCard(frag, filled, target = 9) {
    for (let i = filled; i < target; i++) {
      const el = document.createElement("div");
      el.className = "cmd-empty";
      frag.appendChild(el);
    }
  }

  /**
   * Build a command-card button element.
   * @param {object} opts
   * @param {string} [opts.icon] glyph shown large.
   * @param {string} opts.label visible name.
   * @param {string} [opts.hotkey] keyboard hint shown in a corner.
   * @param {{steel:number,oil:number}} [opts.cost] cost badge (omitted if absent).
   * @param {boolean} opts.enabled whether the action is currently available.
   * @param {string} [opts.title] tooltip / disabled reason.
   * @param {string} [opts.cls] extra class (e.g. "cancel").
   * @param {string} [opts.countBadge] top-right ready count for partially-available abilities.
   * @param {() => void} opts.onClick click handler (skipped when disabled).
   * @returns {HTMLButtonElement}
   */
  _cmdButton(opts) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "cmd-btn" + (opts.cls ? ` ${opts.cls}` : "");
    btn.disabled = !opts.enabled;
    if (opts.title) btn.title = opts.title;
    if (opts.hotkey) {
      // Expose the hotkey so Input/keyboard handling and styles.css can find it.
      btn.dataset.hotkey = opts.hotkey;
    }

    const costHtml = opts.cost
      ? `<span class="cmd-cost">` +
        (opts.cost.steel ? `<span class="c-steel">${opts.cost.steel}</span>` : "") +
        (opts.cost.steel && opts.cost.oil ? `<span class="c-sep">/</span>` : "") +
        (opts.cost.oil ? `<span class="c-oil">${opts.cost.oil}</span>` : "") +
        `</span>`
      : "";

    btn.innerHTML =
      `<span class="cmd-icon">${opts.icon || ""}</span>` +
      `<span class="cmd-label">${opts.label || ""}</span>` +
      (opts.hotkey ? `<span class="cmd-hotkey">${opts.hotkey}</span>` : "") +
      (opts.countBadge ? `<span class="cmd-ready-count">${opts.countBadge}</span>` : "") +
      costHtml;

    if (opts.enabled && typeof opts.onClick === "function") {
      btn.addEventListener("click", (ev) => {
        ev.preventDefault();
        opts.onClick();
      });
    }
    return btn;
  }
}

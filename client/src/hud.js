// HUD — the DOM overlay for the game screen: resource/supply bar, the selected-units
// panel, and the context-sensitive command card (build buttons for workers, train
// buttons for production buildings). See DESIGN.md §4.1 (HUD) and §5 for balance.
//
// The HUD is plain DOM (not Pixi). It is rebuilt cheaply each frame from `state`; the
// only stateful trick is reusing command-card buttons between frames so that holding a
// stable selection does not thrash the DOM (and so hotkeys keep working). All gameplay
// effects go through `net.command(...)` or `state.beginPlacement(...)` — the HUD never
// mutates game state directly.

import { cmd } from "./protocol.js";
import { KIND, STATE, isBuilding, isUnit } from "./protocol.js";
import { STATS, WORKER_BUILDABLE } from "./config.js";

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
    this.elMinerals = rootEl.querySelector("#res-minerals");
    this.elGas = rootEl.querySelector("#res-gas");
    this.elSupply = rootEl.querySelector("#res-supply");

    // Selected-units panel + command card containers.
    this.elSelected = rootEl.querySelector("#selected-panel");
    this.elCommand = rootEl.querySelector("#command-card");

    // Signature of the last-rendered command card so we only rebuild its buttons when
    // the relevant selection/affordability actually changes (keeps DOM + hotkeys stable).
    this._cardSig = null;
  }

  /**
   * Refresh the entire HUD from the latest snapshot/selection. Cheap and idempotent;
   * safe to call every frame.
   */
  update() {
    this._renderResources();
    this._renderSelectedPanel();
    this._renderCommandCard();
  }

  // --- Resource / supply bar -------------------------------------------------

  /** Mirror `state.resources` into the top bar. */
  _renderResources() {
    const r = this.state.resources || { minerals: 0, gas: 0, supplyUsed: 0, supplyCap: 0 };
    if (this.elMinerals) this.elMinerals.textContent = String(r.minerals ?? 0);
    if (this.elGas) this.elGas.textContent = String(r.gas ?? 0);
    if (this.elSupply) {
      const used = r.supplyUsed ?? 0;
      const cap = r.supplyCap ?? 0;
      this.elSupply.textContent = `${used} / ${cap}`;
      // Flag over-cap (blocked production) so styles.css can color it.
      this.elSupply.classList.toggle("supply-capped", cap > 0 && used >= cap);
    }
  }

  // --- Selected-units panel --------------------------------------------------

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
      panel.classList.add("empty");
      return;
    }
    panel.classList.remove("empty");

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

    node.innerHTML =
      `<div class="sel-name"><span class="sel-icon">${st.icon || ""}</span>` +
      `${st.label || e.kind}</div>` +
      `<div class="sel-hpbar"><div class="sel-hpfill ${hpClass}" ` +
      `style="width:${(frac * 100).toFixed(0)}%"></div></div>` +
      `<div class="sel-hptext">${hp} / ${maxHp}</div>`;
    return node;
  }

  // --- Command card ----------------------------------------------------------

  /**
   * Render the context command card based on the current selection:
   *  - selected own units → action buttons for move / attack / stop.
   *  - a selected WORKER → action buttons plus build buttons for WORKER_BUILDABLE.
   *  - a single selected production building (has `STATS[kind].trains`) → train
   *    buttons for each trainable unit, plus a cancel button while producing.
   *  - anything else → empty.
   *
   * Buttons are disabled when unaffordable (vs `state.resources`) or when tech
   * requirements are unmet (e.g. barracks requires an existing Industrial Center).
   */
  _renderCommandCard() {
    const card = this.elCommand;
    if (!card) return;

    const sel = this.state.selectedEntities() || [];
    const primary = this._commandSubject(sel);

    if (!primary) {
      if (this._cardSig !== "empty") {
        card.innerHTML = "";
        this._cardSig = "empty";
      }
      return;
    }

    if (this._selectedOwnUnits(sel).length > 0) {
      this._renderUnitCard(card, sel);
    } else {
      this._renderTrainCard(card, primary);
    }
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
      if (e.kind === KIND.WORKER && !worker) worker = e;
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

  /** Own selected entities that can receive unit commands. */
  _selectedOwnUnits(sel) {
    return sel.filter((e) => this._isOwn(e) && isUnit(e.kind));
  }

  // --- Unit card (units selected) ------------------------------------------

  _renderUnitCard(card, sel) {
    const ownUnits = this._selectedOwnUnits(sel);
    const unitIds = ownUnits.map((e) => e.id);
    const workerSelected = ownUnits.some((e) => e.kind === KIND.WORKER);
    const res = this.state.resources || { minerals: 0, gas: 0 };

    const sig =
      `units|${unitIds.join(".")}|target:${this.state.commandTarget || ""}|` +
      (workerSelected
        ? WORKER_BUILDABLE.map((k) => `${k}:${this._canBuild(k, res) ? 1 : 0}`).join(",")
        : "no-build");
    if (sig === this._cardSig) return;
    this._cardSig = sig;

    const frag = document.createDocumentFragment();
    const actionButtons = [
      {
        icon: "MV",
        label: "Move",
        hotkey: "V",
        title: "Move to a target point",
        active: this.state.commandTarget === "move",
        onClick: () => this.state.beginCommandTarget("move"),
      },
      {
        icon: "AT",
        label: "Attack",
        hotkey: "A",
        title: "Attack a target or attack-move to a point",
        active: this.state.commandTarget === "attack",
        onClick: () => this.state.beginCommandTarget("attack"),
      },
      {
        icon: "HD",
        label: "Hold",
        hotkey: "S",
        title: "Hold position / stop selected units",
        onClick: () => {
          this.net.command(cmd.stop(unitIds));
          this.state.endCommandTarget();
        },
      },
    ];

    for (const action of actionButtons) {
      frag.appendChild(this._cmdButton({
        ...action,
        enabled: unitIds.length > 0,
        cls: action.active ? "active" : "",
      }));
    }

    if (workerSelected) {
      for (const kind of WORKER_BUILDABLE) {
        const st = STATS[kind];
        if (!st) continue;
        const enabled = this._canBuild(kind, res);
        const reason = this._buildDisabledReason(kind, res);
        frag.appendChild(this._cmdButton({
          icon: st.icon,
          label: st.label,
          hotkey: st.hotkey,
          cost: st.cost,
          enabled,
          title: reason,
          onClick: () => {
            this.state.endCommandTarget();
            this.state.beginPlacement(kind);
          },
        }));
      }
    }

    card.innerHTML = "";
    card.appendChild(frag);
  }

  // --- Build card (worker selected) -----------------------------------------

  _renderBuildCard(card) {
    const res = this.state.resources || { minerals: 0, gas: 0 };

    // Signature: which buttons exist + their enabled state. Rebuild only on change.
    const sig =
      "build|" +
      WORKER_BUILDABLE.map((k) => `${k}:${this._canBuild(k, res) ? 1 : 0}`).join(",");
    if (sig === this._cardSig) return;
    this._cardSig = sig;

    const frag = document.createDocumentFragment();
    for (const kind of WORKER_BUILDABLE) {
      const st = STATS[kind];
      if (!st) continue;
      const enabled = this._canBuild(kind, res);
      const reason = this._buildDisabledReason(kind, res);
      const btn = this._cmdButton({
        icon: st.icon,
        label: st.label,
        hotkey: st.hotkey,
        cost: st.cost,
        enabled,
        title: reason,
        onClick: () => this.state.beginPlacement(kind),
      });
      frag.appendChild(btn);
    }
    card.innerHTML = "";
    card.appendChild(frag);
  }

  /** A worker can build `kind` if affordable and any tech requirement is satisfied. */
  _canBuild(kind, res) {
    const st = STATS[kind];
    if (!st) return false;
    if (st.requires && !this._playerHasKind(st.requires)) return false;
    return this._affordable(st.cost, res);
  }

  /** Human-readable disabled reason for a build button tooltip ("" when enabled). */
  _buildDisabledReason(kind, res) {
    const st = STATS[kind];
    if (!st) return "";
    if (st.requires && !this._playerHasKind(st.requires)) {
      const reqLabel = (STATS[st.requires] && STATS[st.requires].label) || st.requires;
      return `Requires ${reqLabel}`;
    }
    if (!this._affordable(st.cost, res)) return "Not enough resources";
    return "";
  }

  // --- Train card (production building selected) -----------------------------

  _renderTrainCard(card, building) {
    const res = this.state.resources || { minerals: 0, gas: 0 };
    const trains = this._trainsOf(building.kind);
    const producing = (building.prodQueue ?? 0) > 0 || building.state === STATE.TRAIN;

    // Signature includes the building id (so switching buildings rebuilds), each
    // trainable unit's affordability, and whether a cancel button is shown.
    const sig =
      `train|${building.id}|` +
      trains.map((u) => `${u}:${this._canTrain(u, res) ? 1 : 0}`).join(",") +
      `|cancel:${producing ? 1 : 0}`;
    if (sig === this._cardSig) {
      // Even when the button set is unchanged, the progress label can move; refresh it.
      this._updateTrainProgress(card, building);
      return;
    }
    this._cardSig = sig;

    const frag = document.createDocumentFragment();
    for (const unit of trains) {
      const st = STATS[unit];
      if (!st) continue;
      const enabled = this._canTrain(unit, res);
      const btn = this._cmdButton({
        icon: st.icon,
        label: st.label,
        hotkey: st.hotkey,
        cost: st.cost,
        enabled,
        title: this._trainDisabledReason(unit, res),
        onClick: () => this.net.command(cmd.train(building.id, unit)),
      });
      frag.appendChild(btn);
    }

    if (producing) {
      const cancelBtn = this._cmdButton({
        icon: "✕",
        label: "Cancel",
        hotkey: "Esc",
        enabled: true,
        cls: "cancel",
        title: "Cancel current production",
        onClick: () => this.net.command(cmd.cancel(building.id)),
      });
      frag.appendChild(cancelBtn);
    }

    // A small production status line (queue count + progress) under the buttons.
    const status = document.createElement("div");
    status.className = "cmd-prod-status";
    frag.appendChild(status);

    card.innerHTML = "";
    card.appendChild(frag);
    this._updateTrainProgress(card, building);
  }

  /** Update only the production progress text/queue for the current train card. */
  _updateTrainProgress(card, building) {
    const status = card.querySelector(".cmd-prod-status");
    if (!status) return;
    const queue = building.prodQueue ?? 0;
    if (queue <= 0) {
      status.textContent = "";
      return;
    }
    const pct = Math.round((building.prodProgress ?? 0) * 100);
    const kind = building.prodKind;
    const label = (kind && STATS[kind] && STATS[kind].label) || kind || "";
    status.textContent = `${label} ${pct}%` + (queue > 1 ? `  (+${queue - 1} queued)` : "");
  }

  // --- Shared helpers --------------------------------------------------------

  /** True if `cost` ({min,gas}) is affordable against `res` ({minerals,gas}). */
  _affordable(cost, res) {
    if (!cost) return true;
    const minerals = res.minerals ?? 0;
    const gas = res.gas ?? 0;
    return minerals >= (cost.min ?? 0) && gas >= (cost.gas ?? 0);
  }

  /** A unit can be trained if affordable and its completed-building tech is present. */
  _canTrain(unit, res) {
    const st = STATS[unit];
    if (!st) return false;
    if (st.requires && !this._playerHasCompleteKind(st.requires)) return false;
    return this._affordable(st.cost, res);
  }

  /** Human-readable disabled reason for a train button tooltip ("" when enabled). */
  _trainDisabledReason(unit, res) {
    const st = STATS[unit];
    if (!st) return "";
    if (st.requires && !this._playerHasCompleteKind(st.requires)) {
      const reqLabel = (STATS[st.requires] && STATS[st.requires].label) || st.requires;
      return `Requires ${reqLabel}`;
    }
    if (!this._affordable(st.cost, res)) return "Not enough resources";
    return "";
  }

  /** True if the player currently owns at least one entity of `kind`. */
  _playerHasKind(kind) {
    // entitiesInterpolated(1) reflects the latest snapshot positions but, more
    // importantly here, the latest set of entities. We only need owner+kind.
    const list = this.state.entitiesInterpolated(1);
    for (const e of list) {
      if (e.owner === this.state.playerId && e.kind === kind) return true;
    }
    return false;
  }

  /** True if the player owns at least one completed entity of `kind`. */
  _playerHasCompleteKind(kind) {
    const list = this.state.entitiesInterpolated(1);
    for (const e of list) {
      if (e.owner === this.state.playerId && e.kind === kind && e.buildProgress == null) return true;
    }
    return false;
  }

  /**
   * Build a command-card button element.
   * @param {object} opts
   * @param {string} [opts.icon] glyph shown large.
   * @param {string} opts.label visible name.
   * @param {string} [opts.hotkey] keyboard hint shown in a corner.
   * @param {{min:number,gas:number}} [opts.cost] cost badge (omitted if absent).
   * @param {boolean} opts.enabled whether the action is currently available.
   * @param {string} [opts.title] tooltip / disabled reason.
   * @param {string} [opts.cls] extra class (e.g. "cancel").
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
        (opts.cost.min ? `<span class="c-min">${opts.cost.min}</span>` : "") +
        (opts.cost.gas ? `<span class="c-gas">${opts.cost.gas}</span>` : "") +
        `</span>`
      : "";

    btn.innerHTML =
      `<span class="cmd-icon">${opts.icon || ""}</span>` +
      `<span class="cmd-label">${opts.label || ""}</span>` +
      (opts.hotkey ? `<span class="cmd-hotkey">${opts.hotkey}</span>` : "") +
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

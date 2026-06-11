// HUD — the DOM overlay for the game screen: resource/supply bar, the selected-units
// panel, and the context-sensitive command card (unit actions, worker build submenu,
// train buttons for production buildings). See docs/design/client-ui.md §4.1 (HUD) and
// docs/design/balance.md for balance.
//
// The HUD is plain DOM (not Pixi). It is rebuilt cheaply each frame from `state`; the
// only stateful trick is reusing command-card buttons between frames so that holding a
// stable selection does not thrash the DOM (and so hotkeys keep working). All gameplay
// effects go through `net.command(...)` or `state.beginPlacement(...)` — the HUD never
// mutates game state directly.

import { ABILITY, cmd } from "./protocol.js";
import { KIND, STATE, isBuilding, isUnit } from "./protocol.js";
import {
  ABILITIES,
  PLAYER_PALETTE,
  STATS,
  TICK_HZ,
  UPGRADES,
  WORKER_BUILDABLE,
} from "./config.js";
import { GRID_HOTKEYS, buildCommandCardDescriptors } from "./hud_command_card.js";
const RESOURCE_ICON_FALLBACKS = Object.freeze({
  steel: "▰",
  oil: "⬤",
  supply: "▲",
});

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
 * Group cooldown-left values that are visually close enough to share one clock arm.
 * @param {number[]} cooldowns ticks left; ready values (<= 0) are ignored
 * @param {number} totalTicks full cooldown duration in ticks
 * @param {number} [toleranceTicks] max intra-group spread before splitting
 * @returns {{count:number,cooldownLeft:number,progress:number,rotationDeg:number}[]}
 */
export function groupCooldownClocks(
  cooldowns,
  totalTicks,
  toleranceTicks = Math.max(2, Math.round(totalTicks / 30)),
) {
  if (!Array.isArray(cooldowns) || !(totalTicks > 0)) return [];

  const active = cooldowns
    .filter((value) => typeof value === "number" && Number.isFinite(value) && value > 0)
    .sort((a, b) => b - a);
  if (active.length === 0) return [];

  const groups = [];
  for (const cooldownLeft of active) {
    const group = groups[groups.length - 1];
    if (group && Math.abs(group.avgCooldownLeft - cooldownLeft) <= toleranceTicks) {
      group.sumCooldownLeft += cooldownLeft;
      group.count += 1;
      group.avgCooldownLeft = group.sumCooldownLeft / group.count;
      continue;
    }
    groups.push({
      count: 1,
      sumCooldownLeft: cooldownLeft,
      avgCooldownLeft: cooldownLeft,
    });
  }

  return groups.map((group) => {
    const cooldownLeft = Math.max(0, Math.min(totalTicks, group.avgCooldownLeft));
    const progress = 1 - cooldownLeft / totalTicks;
    return {
      count: group.count,
      cooldownLeft,
      progress,
      rotationDeg: progress * 360,
    };
  });
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
   * @param {import("./audio.js").Audio} [audio] optional audio engine for local UI notices.
   */
  constructor(rootEl, state, net, audio = null) {
    this.root = rootEl;
    this.state = state;
    this.net = net;
    this.audio = audio;

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
    // Next producer index by selected producing building ids, used to undo production
    // in the opposite order from training when cancel is pressed repeatedly.
    this._cancelRoundRobin = new Map();
    // Signature for the resource bar to avoid unnecessary DOM rebuilds.
    this._resSig = null;
    // Signature for the inert control-group tabs.
    this._controlGroupSig = null;
    // Cache the initial top-bar resource icon markup so command-card hovers and
    // replay rows stay in sync with the shared HUD icons.
    this._resourceIcons = this._readResourceIconHtml();
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
    this._cancelRoundRobin.clear();
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
          `<div class="res">${this._resourceIcon("steel")}<span id="res-steel">0</span></div>` +
          `<div class="res">${this._resourceIcon("oil")}<span id="res-oil">0</span></div>` +
          `<div class="res">${this._resourceIcon("supply")}<span id="res-supply">0 / 0</span></div>`;
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
        `${this._resourceIcon("steel")}<span class="replay-res-val">${pr.steel}</span>` +
        `${this._resourceIcon("oil")}<span class="replay-res-val">${pr.oil}</span>` +
        `${this._resourceIcon("supply")}` +
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
      const kindLabel = (e.prodUpgrade && UPGRADES[e.prodUpgrade]?.label) ||
        (e.prodKind && STATS[e.prodKind] && STATS[e.prodKind].label) ||
        e.prodKind ||
        "";
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
   *    cancel button is shown while any selected compatible producer is producing
   *    in the bottom-right cell (`C` hotkey).
   *  - anything else → empty.
   *
   * Buttons are hard-disabled when tech requirements are unmet (e.g. Vehicle Works
   * requires completed prerequisites). Buttons with available tech but missing
   * resources stay clickable so clicks/hotkeys can play the relevant notice.
   */
  _renderCommandCard() {
    const card = this.elCommand;
    if (!card) return;
    const descriptorCard = buildCommandCardDescriptors(this._commandDescriptorContext());
    if (descriptorCard.kind === "spectator") {
      if (this._cardSig !== descriptorCard.signature) {
        card.innerHTML = "";
        this._cardSig = descriptorCard.signature;
      }
      return;
    }
    if (descriptorCard.signature === this._cardSig) {
      if (descriptorCard.abilityAffordances) {
        this._syncAbilityCooldownClocks(descriptorCard.abilityAffordances);
      }
      return;
    }
    this._cardSig = descriptorCard.signature;
    this._renderDescriptorCard(card, descriptorCard);
  }

  _commandDescriptorContext() {
    return {
      spectator: this.state.spectator,
      playerId: this.state.playerId,
      selection: this.state.selectedEntities() || [],
      resources: this.state.resources || { steel: 0, oil: 0 },
      upgrades: this.state.upgrades || [],
      commandCardMode: this.state.commandCardMode,
      commandTarget: this.state.commandTarget,
      groupCooldownClocks,
      playerHasCompleteKind: (kind) => this._playerHasCompleteKind(kind),
    };
  }

  _renderDescriptorCard(card, descriptorCard) {
    const frag = document.createDocumentFragment();
    const slots = Array.isArray(descriptorCard.slots) ? descriptorCard.slots : [];
    for (let i = 0; i < 9; i++) {
      const descriptor = slots[i] || null;
      frag.appendChild(descriptor ? this._cmdButton(this._descriptorButtonOptions(descriptor)) : this._emptyCommandSlot());
    }
    card.innerHTML = "";
    card.appendChild(frag);
  }

  _descriptorButtonOptions(descriptor) {
    return {
      icon: descriptor.icon,
      label: descriptor.label,
      ability: descriptor.ability,
      hotkey: descriptor.hotkey,
      cost: descriptor.cost,
      enabled: descriptor.enabled,
      unaffordable: descriptor.unaffordable,
      title: descriptor.title,
      tooltipHtml: descriptor.tooltipKind
        ? this._kindTooltipHtml(descriptor.tooltipKind)
        : descriptor.tooltipUpgrade
          ? this._upgradeTooltipHtml(descriptor.tooltipUpgrade)
          : descriptor.tooltipHtml,
      cls: descriptor.cls,
      countBadge: descriptor.countBadge,
      cooldownClocks: descriptor.cooldownClocks,
      repeatable: descriptor.repeatable,
      onUnavailable: descriptor.onUnavailableIntent
        ? () => this._dispatchCommandIntent(descriptor.onUnavailableIntent)
        : null,
      onContextMenu: descriptor.contextIntent
        ? () => this._dispatchCommandIntent(descriptor.contextIntent)
        : null,
      onClick: (ev) => this._dispatchCommandIntent(descriptor.intent, ev),
    };
  }

  _dispatchCommandIntent(intent, ev = {}) {
    if (!intent || typeof intent !== "object") return;
    switch (intent.type) {
      case "beginCommandTarget":
        this.state.beginCommandTarget(intent.target);
        return;
      case "openWorkerBuildMenu":
        this.state.openWorkerBuildMenu();
        return;
      case "closeCommandCardMenu":
        this.state.closeCommandCardMenu();
        return;
      case "beginPlacement":
        this.state.beginPlacement(intent.building);
        return;
      case "stop":
        this.net.command(cmd.stop(intent.unitIds || []));
        this.state.endCommandTarget();
        return;
      case "train":
        this._issueTrain(intent.unit);
        return;
      case "cancelProduction":
        this._issueCancelProduction(intent.buildingKind);
        return;
      case "research":
        this._issueResearch(intent.upgrade);
        return;
      case "ability":
        this._dispatchAbilityIntent(intent, ev);
        return;
      case "setAutocast":
        this.net.command(cmd.setAutocast(intent.ability, intent.unitIds || [], !!intent.enabled));
        this.state.endCommandTarget();
        return;
      case "playNotEnough":
        this._playNotEnoughForCost(intent.cost);
        return;
      default:
        return;
    }
  }

  _dispatchAbilityIntent(intent, ev = {}) {
    if (intent.targetMode === "worldPoint") {
      this.state.beginCommandTarget({ kind: "ability", ability: intent.ability });
    } else {
      this.net.command(cmd.useAbility(
        intent.ability,
        intent.readyIds || [],
        null,
        null,
        !!ev.shiftKey,
      ));
      this.state.endCommandTarget();
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
      if (isBuilding(e.kind) && (this._trainsOf(e.kind).length > 0 || this._researchesOf(e.kind).length > 0)) return e;
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

  _researchesOf(kind) {
    const st = STATS[kind];
    return (st && st.researches) || [];
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

  _abilityCooldownLeft(entity, ability) {
    const projected = Array.isArray(entity.abilities)
      ? entity.abilities.find((entry) => entry.ability === ability)
      : null;
    if (projected && typeof projected.cooldownLeft === "number") return projected.cooldownLeft;
    if (ability === ABILITY.CHARGE) return entity.chargeCooldownLeft || 0;
    return 0;
  }

  _abilityRemainingUses(entity, ability) {
    const projected = Array.isArray(entity.abilities)
      ? entity.abilities.find((entry) => entry.ability === ability)
      : null;
    return projected && typeof projected.remainingUses === "number"
      ? projected.remainingUses
      : null;
  }

  _abilityAutocastEnabled(entity, ability) {
    const projected = Array.isArray(entity.abilities)
      ? entity.abilities.find((entry) => entry.ability === ability)
      : null;
    return projected && typeof projected.autocastEnabled === "boolean"
      ? projected.autocastEnabled
      : false;
  }

  _selectedAbilityAffordances(sel) {
    const ownUnits = this._selectedOwnUnits(sel);
    const res = this.state.resources || { steel: 0, oil: 0 };
    return Object.values(ABILITIES)
      .map((definition) => {
        const carriers = ownUnits.filter((e) => definition.carriers.includes(e.kind));
        if (carriers.length === 0) return null;
        const unlocked = this._abilityUnlocked(definition);
        const affordable = this._affordable(definition.cost, res);
        const readyUnits = carriers.filter((e) =>
          this._abilityCooldownLeft(e, definition.ability) === 0 &&
          this._abilityRemainingUses(e, definition.ability) !== 0
        );
        const cooldowns = carriers.map((e) =>
          this._abilityCooldownLeft(e, definition.ability),
        );
        const depletedCount = carriers.filter(
          (e) => this._abilityRemainingUses(e, definition.ability) === 0,
        ).length;
        const autocastEnabledIds = carriers
          .filter((e) => this._abilityAutocastEnabled(e, definition.ability))
          .map((e) => e.id);
        return {
          definition,
          unlocked,
          affordable,
          depletedCount,
          carrierIds: carriers.map((e) => e.id),
          readyIds: readyUnits.map((e) => e.id),
          autocastEnabledIds,
          cooldownClocks: groupCooldownClocks(cooldowns, definition.cooldownTicks),
        };
      })
      .filter(Boolean);
  }

  _abilityUnlocked(definition) {
    const requirements = this._requirementsOf(definition);
    return requirements.every((req) => this._playerHasCompleteKind(req));
  }

  _abilityTargetActive(ability) {
    return this.state.commandTarget?.kind === "ability" &&
      this.state.commandTarget.ability === ability;
  }

  _commandTargetSig() {
    const target = this.state.commandTarget;
    if (!target) return "";
    if (typeof target === "string") return target;
    return `${target.kind || ""}:${target.ability || ""}`;
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
    const setupGunIds = ownUnits
      .filter((e) => e.kind === KIND.AT_TEAM || e.kind === KIND.ARTILLERY)
      .map((e) => e.id);
    const abilityAffordances = this._selectedAbilityAffordances(sel);
    const hasArmyUnit = ownUnits.some((e) => e.kind !== KIND.WORKER);
    const workerSelected = !hasArmyUnit && ownUnits.some((e) => e.kind === KIND.WORKER);

    const sig =
      `units|${unitIds.join(".")}|target:${this._commandTargetSig()}|` +
      `|setup:${setupGunIds.join(".")}|` +
      `|abilities:${abilityAffordances.map((affordance) =>
        `${affordance.definition.ability}:${affordance.unlocked ? 1 : 0}:${affordance.affordable ? 1 : 0}:` +
        `${affordance.depletedCount}:` +
        `${affordance.readyIds.join(".")}:` +
        `${affordance.autocastEnabledIds.join(".")}:` +
        `${affordance.cooldownClocks.map((group) => group.count).join(",")}`,
      ).join("|")}|` +
      (workerSelected ? "worker-main" : "no-build");
    if (sig === this._cardSig) {
      this._syncAbilityCooldownClocks(abilityAffordances);
      return;
    }
    this._cardSig = sig;

    const frag = document.createDocumentFragment();
    let idx = 0;

    if (workerSelected) {
      // Workers mirror the standard unit layout: Q=Move, A=Attack, S=Stop.
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
        icon: "ST",
        label: "Stop",
        title: "Stop selected units",
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
      // Non-worker units: Q=Move, W/E=empty, A=Attack, S=Stop; abilities/AT slot in D/Z/X/C.
      const slots = new Array(9).fill(null);
      slots[0] = this._cmdButton({
        icon: "MV",
        label: "Move",
        title: "Move to a target point",
        hotkey: GRID_HOTKEYS[0],
        enabled: unitIds.length > 0,
        cls: this.state.commandTarget === "move" ? "active" : "",
        onClick: () => this.state.beginCommandTarget("move"),
      });
      slots[3] = this._cmdButton({
        icon: "AT",
        label: "Attack",
        title: "Attack a target or attack-move to a point",
        hotkey: GRID_HOTKEYS[3],
        enabled: unitIds.length > 0,
        cls: this.state.commandTarget === "attack" ? "active" : "",
        onClick: () => this.state.beginCommandTarget("attack"),
      });
      slots[4] = this._cmdButton({
        icon: "ST",
        label: "Stop",
        title: "Stop selected units",
        hotkey: GRID_HOTKEYS[4],
        enabled: unitIds.length > 0,
        cls: "",
        onClick: () => {
          this.net.command(cmd.stop(unitIds));
          this.state.endCommandTarget();
        },
      });

      let sequentialSlot = 6;
      const claimSlot = (preferred) => {
        if (preferred != null && preferred >= 0 && preferred < 9 && slots[preferred] == null) {
          return preferred;
        }
        while (sequentialSlot < 9 && slots[sequentialSlot] != null) sequentialSlot++;
        return sequentialSlot < 9 ? sequentialSlot++ : -1;
      };

      for (const affordance of abilityAffordances) {
        if (!affordance.unlocked) continue;
        const definition = affordance.definition;
        const readyCount = affordance.readyIds.length;
        const showReadyCount = readyCount < affordance.carrierIds.length;
        const preferred = definition.hotkey ? GRID_HOTKEYS.indexOf(definition.hotkey) : -1;
        const slot = claimSlot(preferred);
        if (slot < 0) continue;
        slots[slot] = this._cmdButton({
          icon: definition.icon,
          label: definition.label,
          title: this._abilityDisabledReason(affordance),
          ability: definition.ability,
          hotkey: GRID_HOTKEYS[slot],
          enabled: readyCount > 0 && affordance.affordable,
          unaffordable: readyCount > 0 && !affordance.affordable,
          countBadge: showReadyCount ? `${readyCount}` : "",
          cooldownClocks: affordance.cooldownClocks,
          cost: definition.cost,
          cls: [
            this._abilityTargetActive(definition.ability) ? "active" : "",
            affordance.autocastEnabledIds.length > 0 ? "autocast-enabled" : "",
          ].filter(Boolean).join(" "),
          onUnavailable: () => this._playNotEnoughForCost(definition.cost),
          onContextMenu: definition.ability === ABILITY.MORTAR_FIRE
            ? (ev) => {
                ev.preventDefault();
                this.net.command(cmd.setAutocast(
                  definition.ability,
                  affordance.carrierIds,
                  false,
                ));
                this.state.endCommandTarget();
              }
            : null,
          onClick: (ev) => {
            if (definition.targetMode === "worldPoint") {
              this.state.beginCommandTarget({ kind: "ability", ability: definition.ability });
            } else {
              this.net.command(cmd.useAbility(
                definition.ability,
                affordance.readyIds,
                null,
                null,
                !!ev.shiftKey,
              ));
              this.state.endCommandTarget();
            }
          },
        });
      }

      if (setupGunIds.length > 0) {
        const setupSlot = claimSlot(null);
        if (setupSlot >= 0) {
          slots[setupSlot] = this._cmdButton({
            icon: "SET",
            label: "Set Up",
            title: "Set up selected support weapons toward a target point",
            hotkey: GRID_HOTKEYS[setupSlot],
            enabled: true,
            cls: this.state.commandTarget === "setupAtGuns" ? "active" : "",
            onClick: () => this.state.beginCommandTarget("setupAtGuns"),
          });
        }
      }

      for (let i = 0; i < 9; i++) {
        if (slots[i]) {
          frag.appendChild(slots[i]);
        } else {
          const el = document.createElement("div");
          el.className = "cmd-empty";
          frag.appendChild(el);
        }
      }
      card.innerHTML = "";
      card.appendChild(frag);
      return;
    }

    this._padCard(frag, idx);
    card.innerHTML = "";
    card.appendChild(frag);
  }

  // --- Build card (worker selected) -----------------------------------------

  _renderBuildCard(card) {
    const res = this.state.resources || { steel: 0, oil: 0 };

    // Signature: which buttons exist + their availability state. Rebuild only on change.
    const sig =
      "build|" +
      WORKER_BUILDABLE.map((k) => `${k}:${this._buildAvailability(k, res)}`).join(",");
    if (sig === this._cardSig) return;
    this._cardSig = sig;

    const frag = document.createDocumentFragment();
    let idx = 0;
    for (const kind of WORKER_BUILDABLE) {
      const st = STATS[kind];
      if (!st) continue;
      const availability = this._buildAvailability(kind, res);
      const enabled = availability === "ready";
      const reason = this._buildDisabledReason(kind, res);
      const btn = this._cmdButton({
        icon: st.icon,
        label: st.label,
        hotkey: GRID_HOTKEYS[idx++],
        cost: st.cost,
        enabled,
        unaffordable: availability === "unaffordable",
        title: reason,
        tooltipHtml: this._kindTooltipHtml(kind),
        onUnavailable: () => this._playNotEnoughForCost(st.cost),
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
    return this._buildAvailability(kind, res) === "ready";
  }

  _buildAvailability(kind, res) {
    const st = STATS[kind];
    if (!st) return "locked";
    if (this._requirementsOf(st).some((req) => !this._playerHasCompleteKind(req))) return "locked";
    return this._affordable(st.cost, res) ? "ready" : "unaffordable";
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
    const researches = this._availableResearchesOf(building.kind);
    const producingBuildings = this._selectedProducingBuildingsForKind(building.kind);
    const producing = producingBuildings.length > 0;
    const cancelSlot = 8;

    // Signature includes the building id (so switching buildings rebuilds), each
    // trainable unit's affordability, the selected compatible producer set for
    // each unit, and whether a cancel button is shown.
    const sig =
      `train|${building.id}|` +
      trains.map((u) => {
        const producerIds = this._selectedProducerBuildingsForUnit(u).map((e) => e.id).join(".");
        return `${u}:${this._trainAvailability(u, res)}:${producerIds}`;
      }).join(",") +
      `|research:` +
      researches.map((u) => `${u}:${this._researchAvailability(u, res)}`).join(",") +
      `|cancel:${producingBuildings.map((e) => e.id).join(".")}`;
    if (sig === this._cardSig) return;
    this._cardSig = sig;

    const slots = new Array(9).fill(null);
    for (const unit of trains) {
      const st = STATS[unit];
      if (!st) continue;
      const slot = slots.findIndex((entry) => entry == null);
      if (slot < 0 || slot === cancelSlot) break;
      const availability = this._trainAvailability(unit, res);
      const enabled = availability === "ready";
      const reason = this._trainDisabledReason(unit, res);
      slots[slot] = this._cmdButton({
        icon: st.icon,
        label: st.label,
        hotkey: GRID_HOTKEYS[slot],
        cost: st.cost,
        enabled,
        unaffordable: availability === "unaffordable",
        title: reason,
        tooltipHtml: this._kindTooltipHtml(unit),
        repeatable: true,
        onUnavailable: () => this._playNotEnoughForCost(st.cost),
        onClick: () => this._issueTrain(unit),
      });
    }
    for (const upgrade of researches) {
      const def = UPGRADES[upgrade];
      if (!def) continue;
      const preferredSlot = this._researchSlotForUpgrade(upgrade, trains);
      const slot = this._firstOpenCommandSlot(slots, preferredSlot, cancelSlot);
      if (slot < 0) continue;
      const availability = this._researchAvailability(upgrade, res);
      const enabled = availability === "ready";
      const reason = this._researchDisabledReason(upgrade, res);
      slots[slot] = this._cmdButton({
        icon: def.icon,
        label: def.label,
        hotkey: GRID_HOTKEYS[slot],
        cost: def.cost,
        enabled,
        unaffordable: availability === "unaffordable",
        title: reason,
        tooltipHtml: this._upgradeTooltipHtml(upgrade),
        repeatable: false,
        onUnavailable: () => this._playNotEnoughForCost(def.cost),
        onClick: () => this._issueResearch(upgrade),
      });
    }

    if (producing) {
      slots[cancelSlot] = this._cmdButton({
        icon: "CNCL",
        label: "Cancel",
        hotkey: GRID_HOTKEYS[cancelSlot],
        enabled: true,
        cls: "cancel",
        title: "Cancel latest queued production",
        repeatable: true,
        onClick: () => this._issueCancelProduction(building.kind),
      });
    }

    const frag = document.createDocumentFragment();
    for (const slot of slots) {
      frag.appendChild(slot || this._emptyCommandSlot());
    }
    card.innerHTML = "";
    card.appendChild(frag);
  }

  _availableResearchesOf(kind) {
    const completed = this.state.upgrades || [];
    return this._researchesOf(kind).filter((upgrade) => !completed.includes(upgrade));
  }

  _researchSlotForUpgrade(upgrade, trains) {
    const unitIndex = trains.findIndex((unit) => STATS[unit]?.upgradeRequires === upgrade);
    if (unitIndex >= 0) return unitIndex + 3;
    const afterTrainSlot = trains.findIndex((unit) => STATS[unit] == null);
    return afterTrainSlot >= 0 ? afterTrainSlot : trains.length;
  }

  _firstOpenCommandSlot(slots, preferredSlot, reservedSlot = -1) {
    const trySlot = (slot) =>
      slot >= 0 && slot < slots.length && slot !== reservedSlot && slots[slot] == null;
    if (trySlot(preferredSlot)) return preferredSlot;
    for (let slot = 0; slot < slots.length; slot++) {
      if (trySlot(slot)) return slot;
    }
    return -1;
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

  /** Selected own completed producers of `kind` that currently have production to cancel. */
  _selectedProducingBuildingsForKind(kind) {
    const sel = this.state.selectedEntities() || [];
    return sel.filter(
      (e) =>
        this._isOwn(e) &&
        e.kind === kind &&
        isBuilding(e.kind) &&
        e.buildProgress == null &&
        ((e.prodQueue ?? 0) > 0 || e.state === STATE.TRAIN),
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

  /** Pick the next producing building in reverse selection order and advance its cursor. */
  _previousProducingBuildingForKind(kind) {
    const producers = this._selectedProducingBuildingsForKind(kind);
    if (producers.length === 0) return null;

    const key = producers.map((e) => e.id).join(".");
    let idx = this._cancelRoundRobin.get(key);
    if (idx == null) idx = producers.length - 1;
    idx = ((idx % producers.length) + producers.length) % producers.length;
    const producer = producers[idx];
    this._cancelRoundRobin.set(key, (idx - 1 + producers.length) % producers.length);
    return producer;
  }

  /** Queue `unit` at the next selected compatible production building. */
  _issueTrain(unit) {
    const building = this._nextProducerBuildingForUnit(unit);
    if (!building) return;
    this.net.command(cmd.train(building.id, unit));
  }

  /** Cancel one production item from the next selected producer in reverse order. */
  _issueCancelProduction(kind) {
    const building = this._previousProducingBuildingForKind(kind);
    if (!building) return;
    this.net.command(cmd.cancel(building.id));
  }

  _issueResearch(upgrade) {
    const def = UPGRADES[upgrade];
    if (!def) return;
    const building = (this.state.selectedEntities() || []).find(
      (e) => this._isOwn(e) && e.kind === def.researchedAt && e.buildProgress == null,
    );
    if (!building) return;
    this.net.command(cmd.research(building.id, upgrade));
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
    return this._trainAvailability(unit, res) === "ready";
  }

  _trainAvailability(unit, res) {
    const st = STATS[unit];
    if (!st) return "locked";
    if (this._requirementsOf(st).some((req) => !this._playerHasCompleteKind(req))) return "locked";
    if (st.upgradeRequires && !(this.state.upgrades || []).includes(st.upgradeRequires)) return "locked";
    return this._affordable(st.cost, res) ? "ready" : "unaffordable";
  }

  _canResearch(upgrade, res) {
    return this._researchAvailability(upgrade, res) === "ready";
  }

  _researchAvailability(upgrade, res) {
    const def = UPGRADES[upgrade];
    if (!def) return "locked";
    if ((this.state.upgrades || []).includes(upgrade)) return "locked";
    if (this._selectedProducingBuildingsForKind(def.researchedAt)
      .some((e) => e.prodUpgrade === upgrade)) return "locked";
    if (def.requiresUpgrade && !(this.state.upgrades || []).includes(def.requiresUpgrade)) return "locked";
    return this._affordable(def.cost, res) ? "ready" : "unaffordable";
  }

  _researchDisabledReason(upgrade, res) {
    const def = UPGRADES[upgrade];
    if (!def) return "";
    if ((this.state.upgrades || []).includes(upgrade)) return "Researched";
    if (this._selectedProducingBuildingsForKind(def.researchedAt)
      .some((e) => e.prodUpgrade === upgrade)) return "Researching";
    if (def.requiresUpgrade && !(this.state.upgrades || []).includes(def.requiresUpgrade)) {
      return def.requiresText || `Requires ${UPGRADES[def.requiresUpgrade]?.label || def.requiresUpgrade}`;
    }
    if (!this._affordable(def.cost, res)) return "Not enough resources";
    return "";
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
    if (st.upgradeRequires && !(this.state.upgrades || []).includes(st.upgradeRequires)) {
      return st.upgradeRequiresText ||
        `Requires ${(UPGRADES[st.upgradeRequires] && UPGRADES[st.upgradeRequires].label) || st.upgradeRequires}`;
    }
    if (!this._affordable(st.cost, res)) return "Not enough resources";
    return "";
  }

  _abilityDisabledReason(affordance) {
    if (!affordance.unlocked) {
      const missing = this._requirementsOf(affordance.definition)
        .find((req) => !this._playerHasCompleteKind(req));
      if (missing) {
        const reqLabel = (STATS[missing] && STATS[missing].label) || missing;
        return `Requires ${reqLabel}`;
      }
    }
    if (!affordance.affordable) return "Not enough resources";
    if (affordance.depletedCount === affordance.carrierIds.length) return "Depleted";
    if (affordance.readyIds.length === 0) return "On cooldown";
    return affordance.definition.title || "";
  }

  _missingResourceSoundId(cost, res = this.state.resources || { steel: 0, oil: 0 }) {
    if (!cost) return null;
    const steelShort = (res.steel ?? 0) < (cost.steel ?? 0);
    const oilShort = (res.oil ?? 0) < (cost.oil ?? 0);
    if (steelShort) return "notice_steel";
    if (oilShort) return "notice_oil";
    return null;
  }

  _playNotEnoughForCost(cost) {
    const soundId = this._missingResourceSoundId(cost);
    if (soundId && this.audio) {
      this.audio.play(soundId, { category: "alert", priority: 4 });
    }
  }

  /** Detailed command-card hover for any buildable or trainable kind. */
  _kindTooltipHtml(kind) {
    const st = STATS[kind];
    if (!st) return "";
    const cost = st.cost || {};
    const requirements = this._requirementsOf(st);
    const upgradeRequirement = st.upgradeRequires
      ? (st.upgradeRequiresText ||
        ((UPGRADES[st.upgradeRequires] && UPGRADES[st.upgradeRequires].label) || st.upgradeRequires))
      : null;
    const requirementLabels = requirements.length > 0 || upgradeRequirement
      ? requirements.map((req) => (STATS[req] && STATS[req].label) || req).join(", ")
        + (requirements.length > 0 && upgradeRequirement ? ", " : "")
        + (upgradeRequirement || "")
      : "None";
    const buildSeconds = Math.max(0, (st.buildTicks || 0) / TICK_HZ);
    const buildTime = Number.isInteger(buildSeconds)
      ? `${buildSeconds}s`
      : `${buildSeconds.toFixed(1)}s`;

    return (
      `<span class="cmd-tooltip-title">${st.label}</span>` +
      `<span class="cmd-tooltip-costs">` +
        `<span class="cmd-tooltip-cost">${this._resourceIcon("steel")}<span>${cost.steel ?? 0}</span></span>` +
        `<span class="cmd-tooltip-cost">${this._resourceIcon("oil")}<span>${cost.oil ?? 0}</span></span>` +
        `<span class="cmd-tooltip-cost">${this._resourceIcon("supply")}<span>${st.supply ?? 0}</span></span>` +
      `</span>` +
      `<span class="cmd-tooltip-row"><span>Requires</span><strong>${requirementLabels}</strong></span>` +
      `<span class="cmd-tooltip-row"><span>Build time</span><strong>${buildTime}</strong></span>`
    );
  }

  _upgradeTooltipHtml(upgrade) {
    const def = UPGRADES[upgrade];
    if (!def) return "";
    const cost = def.cost || {};
    const seconds = Math.max(0, (def.researchTicks || 0) / TICK_HZ);
    const time = Number.isInteger(seconds) ? `${seconds}s` : `${seconds.toFixed(1)}s`;
    return (
      `<span class="cmd-tooltip-title">${def.label}</span>` +
      `<span class="cmd-tooltip-costs">` +
        `<span class="cmd-tooltip-cost">${this._resourceIcon("steel")}<span>${cost.steel ?? 0}</span></span>` +
        `<span class="cmd-tooltip-cost">${this._resourceIcon("oil")}<span>${cost.oil ?? 0}</span></span>` +
      `</span>` +
      `<span class="cmd-tooltip-desc">${def.description}</span>` +
      `<span class="cmd-tooltip-row"><span>Research time</span><strong>${time}</strong></span>`
    );
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
      frag.appendChild(this._emptyCommandSlot());
    }
  }

  _emptyCommandSlot() {
    const el = document.createElement("div");
    el.className = "cmd-empty";
    return el;
  }

  _syncAbilityCooldownClocks(abilityAffordances) {
    if (!this.elCommand || typeof this.elCommand.querySelector !== "function") return;
    for (const affordance of abilityAffordances) {
      const button = this.elCommand.querySelector(
        `button[data-ability="${affordance.definition.ability}"]`,
      );
      if (!button) continue;
      this._syncCooldownClockElement(button, affordance.cooldownClocks);
    }
  }

  _syncCooldownClockElement(button, cooldownClocks) {
    const clocks = Array.isArray(cooldownClocks) ? cooldownClocks : [];
    const arms = typeof button.querySelectorAll === "function"
      ? Array.from(button.querySelectorAll(".cmd-cd-arm"))
      : [];
    if (arms.length !== clocks.length) return;
    for (let i = 0; i < arms.length; i++) {
      arms[i].style.setProperty("--cooldown-rotation", `${clocks[i].rotationDeg.toFixed(1)}deg`);
    }
  }

  /**
   * Build a command-card button element.
   * @param {object} opts
   * @param {string} [opts.icon] glyph shown large.
   * @param {string} opts.label visible name.
   * @param {string} [opts.ability] ability id for dynamic cooldown-clock refreshes.
   * @param {string} [opts.hotkey] keyboard hint shown in a corner.
   * @param {{steel:number,oil:number}} [opts.cost] cost badge (omitted if absent).
   * @param {boolean} opts.enabled whether the action is currently available.
   * @param {boolean} [opts.unaffordable] true when tech is available but resources are short.
   * @param {string} [opts.title] tooltip / disabled reason.
   * @param {string} [opts.tooltipHtml] rich hover content rendered inside the button.
   * @param {string} [opts.cls] extra class (e.g. "cancel").
   * @param {string} [opts.countBadge] top-right ready count for partially-available abilities.
   * @param {{count:number,rotationDeg:number}[]} [opts.cooldownClocks] grouped cooldown clocks.
   * @param {boolean} [opts.repeatable] whether native keyboard repeat may trigger this button.
   * @param {() => void} [opts.onUnavailable] click handler for unaffordable buttons.
   * @param {(ev: MouseEvent) => void} [opts.onContextMenu] right-click handler.
   * @param {(ev: MouseEvent) => void} opts.onClick click handler (skipped when disabled).
   * @returns {HTMLButtonElement}
   */
  _cmdButton(opts) {
    const btn = document.createElement("button");
    btn.type = "button";
    const classes = ["cmd-btn"];
    if (opts.cls) classes.push(opts.cls);
    if (opts.unaffordable) classes.push("unaffordable");
    btn.className = classes.join(" ");
    btn.disabled = !opts.enabled && !opts.unaffordable;
    if (opts.title) btn.title = opts.title;
    if (opts.hotkey) {
      // Expose the hotkey so Input/keyboard handling and styles.css can find it.
      btn.dataset.hotkey = opts.hotkey;
    }
    if (opts.ability) btn.dataset.ability = opts.ability;
    if (opts.repeatable) btn.dataset.repeatable = "true";
    if (typeof opts.onContextMenu === "function") {
      btn.addEventListener("contextmenu", (ev) => {
        ev.preventDefault();
        opts.onContextMenu(ev);
      });
    }

    const costHtml = opts.cost
      ? `<span class="cmd-cost">` +
        (opts.cost.steel ? `<span class="c-steel">${opts.cost.steel}</span>` : "") +
        (opts.cost.steel && opts.cost.oil ? `<span class="c-sep">/</span>` : "") +
        (opts.cost.oil ? `<span class="c-oil">${opts.cost.oil}</span>` : "") +
        `</span>`
      : "";
    const cooldownClocks = Array.isArray(opts.cooldownClocks) ? opts.cooldownClocks : [];
    const cooldownHtml = cooldownClocks.length > 0
      ? `<span class="cmd-cooldowns" aria-hidden="true">` +
          `<span class="cmd-cd-clock">` +
            cooldownClocks.map((group) =>
              `<span class="cmd-cd-arm" style="--cooldown-rotation:${group.rotationDeg.toFixed(1)}deg"></span>`
            ).join("") +
          `</span>` +
        `</span>`
      : "";

    btn.innerHTML =
      `<span class="cmd-icon">${opts.icon || ""}</span>` +
      `<span class="cmd-label">${opts.label || ""}</span>` +
      (opts.hotkey ? `<span class="cmd-hotkey">${opts.hotkey}</span>` : "") +
      cooldownHtml +
      (opts.countBadge ? `<span class="cmd-ready-count">${opts.countBadge}</span>` : "") +
      costHtml +
      (opts.tooltipHtml ? `<span class="cmd-tooltip" role="tooltip">${opts.tooltipHtml}</span>` : "");

    if (opts.enabled && typeof opts.onClick === "function") {
      btn.addEventListener("click", (ev) => {
        ev.preventDefault();
        opts.onClick(ev);
      });
    } else if (opts.unaffordable && typeof opts.onUnavailable === "function") {
      btn.addEventListener("click", (ev) => {
        ev.preventDefault();
        opts.onUnavailable(ev);
      });
    }
    return btn;
  }

  _readResourceIconHtml() {
    const icons = {};
    for (const kind of Object.keys(RESOURCE_ICON_FALLBACKS)) {
      const el = this.elHud?.querySelector(`.res-icon.${kind}`);
      icons[kind] = el
        ? el.outerHTML
        : `<span class="res-icon ${kind}">${RESOURCE_ICON_FALLBACKS[kind]}</span>`;
    }
    return icons;
  }

  _resourceIcon(kind) {
    return this._resourceIcons[kind] ||
      `<span class="res-icon ${kind}">${RESOURCE_ICON_FALLBACKS[kind] || ""}</span>`;
  }
}

// HUD — the DOM overlay for the game screen: resource/supply bar, the selected-units
// panel, and the context-sensitive command card (unit actions, worker build submenu,
// train buttons for production buildings). See docs/design/client-ui.md §4.1 (HUD) and
// docs/design/balance.md for balance.
//
// The HUD is plain DOM (not Pixi). It is rebuilt cheaply each frame from `state`; the
// only stateful trick is reusing command-card buttons between frames so that holding a
// stable selection does not thrash the DOM (and so hotkeys keep working). All gameplay
// effects go through `commandIssuer.issueCommand(...)` or the injected client intent facade.

import { cmd } from "./protocol.js";
import { ABILITY, STATE, isBuilding, isUnit } from "./protocol.js";
import {
  ABILITIES,
  STATS,
  TICK_HZ,
  UPGRADES,
} from "./config.js";
import {
  buildControlGroupSummaries,
  controlGroupMatchesSelection,
  controlGroupTabsSignature,
  dominantControlGroupKind,
  renderControlGroupTabs,
} from "./hud_control_groups.js";
import { buildCommandCardDescriptors } from "./hud_command_card.js";
import {
  createCommandButton,
  emptyCommandSlot,
  renderDescriptorCardDom,
  syncCooldownClockElement,
} from "./hud_command_dom.js";
import {
  renderAllPlayersResources,
  renderSinglePlayerResources,
  restoreSinglePlayerResourceShell,
} from "./hud_resources.js";
import { HudSelectionPanel } from "./hud_selection_panel.js";
import { resourceIconHtml } from "./resource_icons.js";
export {
  formatTankOilUsed,
  selectionBudgetBlockShape,
  selectionBudgetGridModel,
} from "./hud_selection_panel.js";

const BREAKTHROUGH_VOICE_IDS = Object.freeze([
  "unit_breakthrough_todes_rit_01",
  "unit_breakthrough_koste_es_01",
]);

function escapeQueueText(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

/** True if `playerId` owns at least one completed entity of `kind` in `entities`. */
export function playerHasCompletedKind(entities, playerId, kind) {
  for (const e of entities || []) {
    if (e.owner === playerId && e.kind === kind && e.buildProgress == null) return true;
  }
  return false;
}

function frameSelectedEntities(state, frameViews = null) {
  if (Array.isArray(frameViews?.selectedEntities)) return frameViews.selectedEntities;
  return typeof state?.selectedEntities === "function" ? state.selectedEntities() || [] : [];
}

function frameCurrentEntities(state, frameViews = null) {
  if (Array.isArray(frameViews?.currentEntities)) return frameViews.currentEntities;
  return typeof state?.entitiesInterpolated === "function" ? state.entitiesInterpolated(1) || [] : [];
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

export function formatGameTime(tick, tickHz = TICK_HZ) {
  const safeTick = Number.isFinite(tick) ? Math.max(0, Math.trunc(tick)) : 0;
  const safeTickHz = Number.isFinite(tickHz) && tickHz > 0 ? tickHz : TICK_HZ;
  const totalSeconds = Math.floor(safeTick / safeTickHz);
  const seconds = totalSeconds % 60;
  const totalMinutes = Math.floor(totalSeconds / 60);
  const minutes = totalMinutes % 60;
  const hours = Math.floor(totalMinutes / 60);
  const two = (value) => String(value).padStart(2, "0");
  return hours > 0
    ? `${hours}:${two(minutes)}:${two(seconds)}`
    : `${two(minutes)}:${two(seconds)}`;
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
   * @param {{issueCommand(command: object): object|boolean}} commandIssuer gameplay command seam.
   * @param {import("./audio.js").Audio} [audio] optional audio engine for local UI notices.
   * @param {import("./hotkey_profiles.js").HotkeyProfileService} [hotkeyProfiles] active hotkey resolver.
   * @param {import("./client_intent.js").ClientIntent} [clientIntent] browser-local command/placement intent facade.
   * @param {object} [controlPolicy] policy that decides command-surface and owner control.
   * @param {import("./camera.js").Camera} [camera] viewport camera for command-card focus actions.
   */
  constructor(rootEl, state, commandIssuer, audio = null, hotkeyProfiles = null, clientIntent = null, controlPolicy = null, camera = null) {
    this.root = rootEl;
    this.state = state;
    this.commandIssuer = commandIssuer;
    this.audio = audio;
    this.hotkeyProfiles = hotkeyProfiles;
    this.clientIntent = clientIntent;
    this.controlPolicy = controlPolicy;
    this.camera = camera;

    // Resource / supply bar elements.
    this.elHud = rootEl.querySelector("#hud");
    this.elSteel = rootEl.querySelector("#res-steel");
    this.elOil = rootEl.querySelector("#res-oil");
    this.elSupply = rootEl.querySelector("#res-supply");
    this.elGameTimer = rootEl.querySelector("#game-timer");
    this.elProductionQueue = rootEl.querySelector("#production-request-queue");
    // Rebuild the static shell once so the top bar, replay rows, and command-card
    // hovers all use the shared resource icon definitions.
    if (this.elHud) {
      this._restoreSinglePlayerResourceShell();
    }

    // Selected-units panel + command card containers.
    this.elSelected = rootEl.querySelector("#selected-panel");
    this.elControlGroups = rootEl.querySelector("#control-group-tabs");
    this.elCommand = rootEl.querySelector("#command-card");
    this.selectionPanel = new HudSelectionPanel(this.elSelected, this.state);

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
    this._gameTimerSig = null;
    this._productionQueueSig = null;
    this._renderGameTimer();
  }

  /**
   * Refresh the entire HUD from the latest snapshot/selection. Cheap and idempotent;
   * safe to call every frame.
   */
  update(frameViews = null, { profiler = null } = {}) {
    this._profiler = profiler || null;
    this._renderGameTimer();
    this._renderProductionQueue();
    this._renderResources();
    this._renderControlGroupTabs(frameViews);
    this._renderSelectedPanel(frameViews);
    this._renderCommandCard(frameViews);
  }

  /** Clear DOM-owned HUD state between matches. */
  destroy() {
    this.selectionPanel?.destroy();
    if (this.elControlGroups) {
      this.elControlGroups.innerHTML = "";
      this.elControlGroups.classList.add("empty");
    }
    if (this.elCommand) this.elCommand.innerHTML = "";
    if (this.elSupply) this.elSupply.classList.remove("supply-capped");
    if (this.elGameTimer) {
      this.elGameTimer.textContent = "00:00";
      this.elGameTimer.title = "Game time 00:00";
    }
    this._cardSig = null;
    this._trainRoundRobin.clear();
    this._cancelRoundRobin.clear();
    this._resSig = null;
    this._controlGroupSig = null;
    this._gameTimerSig = null;
    this._productionQueueSig = null;
    if (this.elProductionQueue) this.elProductionQueue.innerHTML = "";
  }

  _issueCommand(command, options = {}) {
    const selected = typeof this.state?.selectedEntities === "function"
      ? this.state.selectedEntities()
      : [];
    const result = issueGameplayCommand(this.commandIssuer, command, options);
    this._intent()?.recordPlannedCommand?.(command, selected, result);
    return result;
  }

  _intent() {
    return this.clientIntent;
  }

  _renderGameTimer() {
    if (!this.elGameTimer) return;
    const text = formatGameTime(this.state?.tick ?? 0, TICK_HZ);
    if (text === this._gameTimerSig) return;
    this.elGameTimer.textContent = text;
    this.elGameTimer.title = `Game time ${text}`;
    this._gameTimerSig = text;
  }

  _renderProductionQueue() {
    const root = this.elProductionQueue;
    if (!root) return;
    const queue = Array.isArray(this.state?.productionQueue) ? this.state.productionQueue : [];
    const signature = queue
      .map((request) => `${request.requestKind}:${request.item}:${request.producerId}:${request.remaining ?? "auto"}`)
      .join("|");
    if (signature === this._productionQueueSig) return;
    this._productionQueueSig = signature;
    const rows = queue.slice(0, 6).map((request, index) => {
      const label = request.requestKind === "research"
        ? UPGRADES[request.item]?.label || request.item
        : STATS[request.item]?.label || request.item;
      const quantity = request.remaining == null ? "∞" : `×${request.remaining}`;
      return `<div class="production-request-row">` +
        `<span class="production-request-index">${index + 1}</span>` +
        `<span class="production-request-label">${escapeQueueText(label)}</span>` +
        `<span class="production-request-quantity">${quantity}</span>` +
        `</div>`;
    }).join("");
    const more = queue.length > 6
      ? `<div class="production-request-more">+${queue.length - 6} more</div>`
      : "";
    root.innerHTML = `<div class="production-request-title">QUEUE</div>` +
      (rows || `<div class="production-request-empty">—</div>`) + more;
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
    const result = renderSinglePlayerResources({
      state: this.state,
      elHud: this.elHud,
      elSteel: this.elSteel,
      elOil: this.elOil,
      elSupply: this.elSupply,
      currentSig: this._resSig,
      recordDiagnostic: (label, amount) => this._recordHudDiagnostic(label, amount),
    });
    this._resSig = result.sig;
    this.elSteel = result.elSteel;
    this.elOil = result.elOil;
    this.elSupply = result.elSupply;
  }

  _restoreSinglePlayerResourceShell() {
    const result = restoreSinglePlayerResourceShell(this.elHud);
    this.elSteel = result.elSteel;
    this.elOil = result.elOil;
    this.elSupply = result.elSupply;
  }

  /** Render one resource row per player, with a color-coded dot identifying each player. */
  _renderAllPlayersResources(playerResources) {
    const result = renderAllPlayersResources({
      state: this.state,
      playerResources,
      elHud: this.elHud,
      currentSig: this._resSig,
      recordDiagnostic: (label, amount) => this._recordHudDiagnostic(label, amount),
    });
    this._resSig = result.sig;
  }

  // --- Selected-units panel --------------------------------------------------

  /** Render fixed-position, non-clickable tabs for occupied local control groups. */
  _renderControlGroupTabs(frameViews = null) {
    const tabs = this.elControlGroups;
    if (!tabs) return;

    const groups = this._controlGroupSummaries(frameViews);
    const sig = controlGroupTabsSignature(groups);
    if (sig === this._controlGroupSig) {
      this._recordHudDiagnostic("hud.dirty.controlGroups.hit");
      return;
    }
    this._recordHudDiagnostic("hud.dirty.controlGroups.miss");
    this._controlGroupSig = sig;

    renderControlGroupTabs(tabs, groups);
  }

  _controlGroupSummaries(frameViews = null) {
    const selected = frameSelectedEntities(this.state, frameViews);
    return buildControlGroupSummaries(this.state, selected);
  }

  _dominantControlGroupKind(entities) {
    return dominantControlGroupKind(entities);
  }

  _controlGroupMatchesSelection(entities, selectedIds, selectedCount) {
    return controlGroupMatchesSelection(entities, selectedIds, selectedCount);
  }

  _renderSelectedPanel(frameViews = null) {
    this.selectionPanel?.render(frameViews, { profiler: this._profiler });
  }

  // --- Command card ----------------------------------------------------------

  /**
   * Render the context command card based on the current selection:
   *  - selected own units → action buttons for move / attack / stop.
   *  - a selected WORKER → unit action buttons plus a build-menu button.
   *  - worker build submenu → build buttons plus a return button.
   *  - selected production buildings (have `STATS[kind].trains`) → train
   *    buttons for the primary producer's trainable units. Successive train clicks
   *    are distributed round-robin across the selected compatible producers. A
   *    cancel button is shown while any selected compatible producer is producing.
   *  - anything else → empty.
   *
   * Buttons are hard-disabled when tech requirements are unmet (e.g. Vehicle Works
   * requires completed prerequisites). Buttons with available tech but missing
   * resources stay clickable so clicks/hotkeys can play the relevant notice.
   */
  _renderCommandCard(frameViews = null) {
    const card = this.elCommand;
    if (!card) return;
    let descriptorCard = buildCommandCardDescriptors(this._commandDescriptorContext(frameViews));
    if (this.hotkeyProfiles) descriptorCard = this.hotkeyProfiles.resolveCard(descriptorCard);
    const cardSig = `${descriptorCard.signature}|hotkeys:${this.hotkeyProfiles?.revision || 0}`;
    if (descriptorCard.kind === "spectator") {
      if (this._cardSig !== cardSig) {
        card.innerHTML = "";
        this._cardSig = cardSig;
        this._recordHudDiagnostic("hud.dirty.commandCard.miss");
      } else {
        this._recordHudDiagnostic("hud.dirty.commandCard.hit");
      }
      return;
    }
    if (cardSig === this._cardSig) {
      this._recordHudDiagnostic("hud.dirty.commandCard.hit");
      if (descriptorCard.abilityAffordances) {
        this._syncAbilityCooldownClocks(descriptorCard.abilityAffordances);
      }
      return;
    }
    this._recordHudDiagnostic("hud.dirty.commandCard.miss");
    this._cardSig = cardSig;
    this._renderDescriptorCard(card, descriptorCard);
  }

  _commandDescriptorContext(frameViews = null) {
    const selection = frameSelectedEntities(this.state, frameViews);
    const currentEntities = frameCurrentEntities(this.state, frameViews);
    const commandOwner = this._commandOwnerForSelection(selection);
    this._recordHudDiagnostic(
      Array.isArray(frameViews?.selectedEntities)
        ? "entityViews.cache.hit.hud.selected"
        : "entityViews.uncached.hud.selected",
    );
    this._recordHudDiagnostic(
      Array.isArray(frameViews?.currentEntities)
        ? "entityViews.cache.hit.hud.current"
        : "entityViews.uncached.hud.current",
    );
    return {
      spectator: this.state.spectator,
      commandSurfaceEnabled: this._canUseCommandSurface(),
      state: this.state,
      playerId: commandOwner ?? this.state.playerId,
      commandOwner,
      factionId: this._commandFactionId(commandOwner),
      selection,
      currentEntities,
      resources: this._commandResources(commandOwner),
      optimisticProduction: this.state.optimisticProduction || [],
      upgrades: this._commandUpgrades(commandOwner),
      productionQueue: this.state.productionQueue || [],
      commandCardMode: this._intent()?.commandCardMode,
      commandTarget: this._intent()?.commandTarget,
      controlPolicy: this.controlPolicy,
      groupCooldownClocks,
      playerHasCompleteKind: (kind) => playerHasCompletedKind(
        currentEntities,
        commandOwner ?? this.state.playerId,
        kind,
      ),
    };
  }

  _canUseCommandSurface() {
    if (typeof this.controlPolicy?.canUseCommandSurface === "function") {
      return !!this.controlPolicy.canUseCommandSurface(this.state);
    }
    return !this.state?.spectator;
  }

  _renderDescriptorCard(card, descriptorCard) {
    this._clearAbilityHoverPreview();
    renderDescriptorCardDom(
      card,
      descriptorCard,
      (descriptor) => this._cmdButton(this._descriptorButtonOptions(descriptor)),
    );
  }

  _descriptorButtonOptions(descriptor) {
    return {
      commandId: descriptor.commandId,
      slotIndex: descriptor.slotIndex,
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
      autocastToggle: descriptor.contextIntent?.type === "setAutocast",
      onMouseEnter: descriptor.intent?.type === "ability"
        ? () => this._showAbilityHoverPreview(descriptor.intent.ability, descriptor.intent.readyIds || [])
        : null,
      onMouseLeave: descriptor.intent?.type === "ability"
        ? () => this._clearAbilityHoverPreview()
        : null,
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
        this._intent()?.beginCommandTarget?.(intent.target, { shiftKey: !!ev.shiftKey });
        return;
      case "openWorkerBuildMenu":
        this._intent()?.openWorkerBuildMenu?.();
        return;
      case "closeCommandCardMenu":
        this._intent()?.closeCommandCardMenu?.();
        return;
      case "beginPlacement":
        this._intent()?.beginPlacement?.(intent.building);
        return;
      case "stop":
        this._issueCommand(cmd.stop(intent.unitIds || []));
        this._intent()?.endCommandTarget?.();
        return;
      case "holdPosition":
        this._issueCommand(cmd.holdPosition(intent.unitIds || []));
        this._intent()?.endCommandTarget?.();
        return;
      case "train":
        this._issueTrain(intent.unit, {
          quantity: intent.automatic ? 1 : (ev.shiftKey ? 5 : 1),
          automatic: !!intent.automatic,
        });
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
        this._issueCommand(cmd.setAutocast(intent.ability, intent.unitIds || [], !!intent.enabled));
        this._intent()?.endCommandTarget?.();
        return;
      case "playNotEnough":
        this._playNotEnoughForCost(intent.cost, intent.supply);
        return;
      default:
        return;
    }
  }

  _dispatchAbilityIntent(intent, ev = {}) {
    if (intent.targetMode === "recast") {
      this._issueCommand(cmd.recastAbility(
        intent.ability,
        intent.readyIds || [],
        intent.targetObjectId ?? null,
        !!ev.shiftKey,
      ));
      this._intent()?.endCommandTarget?.();
      return;
    }
    if (intent.targetMode === "worldPoint") {
      this._intent()?.beginCommandTarget?.({ kind: "ability", ability: intent.ability }, { shiftKey: !!ev.shiftKey });
    } else {
      this._issueCommand(cmd.useAbility(
        intent.ability,
        intent.readyIds || [],
        null,
        null,
        !!ev.shiftKey,
      ));
      if (intent.ability === ABILITY.BREAKTHROUGH && this.audio) {
        const id = this.audio.pickVariant(BREAKTHROUGH_VOICE_IDS);
        if (id) this.audio.play(id, { category: "unit_voice", priority: 3 });
      }
      this._intent()?.endCommandTarget?.();
    }
  }

  _showAbilityHoverPreview(ability, unitIds = []) {
    const definition = ABILITIES[ability];
    if (!definition || definition.targetMode !== "self" || !definition.radiusTiles) return;
    if (this._intent()?.commandTarget) return;
    const ready = new Set((unitIds || []).map((id) => Number(id)));
    const origins = this._abilityAreaOrigins(definition)
      .filter((origin) => ready.size === 0 || ready.has(origin.id));
    if (origins.length === 0) return;
    const tileSize = this.state.map?.tileSize || 32;
    this._intent()?.updateAbilityTargetPreview?.({
      ability,
      source: "commandCardHover",
      carriers: origins,
      areaOrigins: origins,
      radiusPx: definition.radiusTiles * tileSize,
      hoverInRange: true,
    });
  }

  _clearAbilityHoverPreview() {
    if (this._intent()?.abilityTargetPreview?.source === "commandCardHover") {
      this._intent()?.updateAbilityTargetPreview?.(null);
    }
  }

  _abilityAreaOrigins(definition) {
    const carriers = Array.isArray(definition?.carriers) ? definition.carriers : [];
    return (this.state.selectedEntities() || [])
      .filter((e) =>
        this._isOwn(e) &&
        carriers.includes(e.kind) &&
        e.buildProgress == null)
      .map((e) => ({ id: e.id, kind: e.kind, x: e.x, y: e.y }));
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
    const owner = this._commandOwnerForSelection();
    if (owner != null) return e && Number(e.owner) === owner;
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

  _requirementsAnyOf(st) {
    if (!st || !st.requiresAny) return [];
    return Array.isArray(st.requiresAny) ? st.requiresAny : [st.requiresAny];
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
        ((e.prodQueue ?? 0) > 0 || e.state === STATE.TRAIN ||
          (this.state.productionQueue || []).some((request) => request.producerId === e.id)),
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
  _issueTrain(unit, { quantity = 1, automatic = false } = {}) {
    const building = this._nextProducerBuildingForUnit(unit);
    if (!building) return;
    this._issueCommand(cmd.train(building.id, unit, quantity, automatic));
  }

  _selectAndPanToEntity(entityId) {
    const id = Number(entityId);
    if (!Number.isInteger(id)) return;
    const entity = (typeof this.state?.entityById === "function" ? this.state.entityById(id) : null) ||
      frameCurrentEntities(this.state).find((e) => e.id === id);
    if (!entity) return;
    this.state?.setSelection?.([id]);
    if (Number.isFinite(entity.x) && Number.isFinite(entity.y)) {
      this.camera?.focusAt?.({ x: entity.x, y: entity.y });
    }
  }

  /** Cancel one production item from the next selected producer in reverse order. */
  _issueCancelProduction(kind) {
    const building = this._previousProducingBuildingForKind(kind);
    if (!building) return;
    this._issueCommand(cmd.cancel(building.id));
  }

  _issueResearch(upgrade) {
    const def = UPGRADES[upgrade];
    if (!def) return;
    const building = (this.state.selectedEntities() || []).find(
      (e) => this._isOwn(e) && e.kind === def.researchedAt && e.buildProgress == null,
    );
    if (!building) return;
    this._issueCommand(cmd.research(building.id, upgrade));
  }

  // --- Shared helpers --------------------------------------------------------

  /** True if `cost` ({steel,oil}) is affordable against `res` ({steel,oil}). */
  _affordable(cost, res) {
    if (!cost) return true;
    const steel = res.steel ?? 0;
    const oil = res.oil ?? 0;
    return steel >= (cost.steel ?? 0) && oil >= (cost.oil ?? 0);
  }

  _missingResourceSoundId(cost, res = null, supply = null) {
    if (!cost) return null;
    const resources = res || this._commandResources();
    const steelShort = (resources.steel ?? 0) < (cost.steel ?? 0);
    const oilShort = (resources.oil ?? 0) < (cost.oil ?? 0);
    if (steelShort) return "notice_steel";
    if (oilShort) return "notice_oil";
    if (Number.isFinite(supply) && supply > 0) {
      const used = resources.supplyUsed ?? 0;
      const cap = resources.supplyCap ?? 0;
      if (used + supply > cap) return "notice_supply";
    }
    return null;
  }

  _playNotEnoughForCost(cost, supply = null) {
    const soundId = this._missingResourceSoundId(cost, undefined, supply);
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
    const anyRequirements = this._requirementsAnyOf(st);
    const upgradeRequirement = st.upgradeRequires
      ? (st.upgradeRequiresText ||
        ((UPGRADES[st.upgradeRequires] && UPGRADES[st.upgradeRequires].label) || st.upgradeRequires))
      : null;
    const requirementParts = [];
    if (requirements.length > 0) {
      requirementParts.push(requirements.map((req) => (STATS[req] && STATS[req].label) || req).join(", "));
    }
    if (anyRequirements.length > 0) {
      requirementParts.push(anyRequirements.map((req) => (STATS[req] && STATS[req].label) || req).join(" or "));
    }
    if (upgradeRequirement) requirementParts.push(upgradeRequirement);
    const requirementLabels = requirementParts.join(", ") || "None";
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
      (st.description ? `<span class="cmd-tooltip-desc">${st.description}</span>` : "") +
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
  _playerHasCompleteKind(kind, frameViews = null, owner = null) {
    // currentEntities reflects the latest snapshot positions but, more
    // importantly here, the latest set of entities.
    return playerHasCompletedKind(
      frameCurrentEntities(this.state, frameViews),
      owner ?? this.state.playerId,
      kind,
    );
  }

  _commandOwnerForSelection(selection = null) {
    const policy = this.controlPolicy || this.state?.controlPolicy;
    const selected = selection || (typeof this.state?.selectedEntities === "function" ? this.state.selectedEntities() || [] : []);
    const owner = typeof policy?.commandOwnerForSelection === "function"
      ? policy.commandOwnerForSelection(selected, this.state)
      : typeof policy?.commandOwner === "function"
        ? policy.commandOwner(this.state)
        : this.state?.playerId;
    const ownerId = Number(owner);
    return Number.isInteger(ownerId) && ownerId > 0 ? ownerId : null;
  }

  _commandResources(owner = this._commandOwnerForSelection()) {
    const policy = this.controlPolicy || this.state?.controlPolicy;
    if (typeof policy?.commandResources === "function") {
      return policy.commandResources(this.state, owner);
    }
    return this.state.resources || { steel: 0, oil: 0, supplyUsed: 0, supplyCap: 0 };
  }

  _commandFactionId(owner = this._commandOwnerForSelection()) {
    const policy = this.controlPolicy || this.state?.controlPolicy;
    if (typeof policy?.commandFactionId === "function") {
      return policy.commandFactionId(this.state, owner);
    }
    return this.state.localFactionId;
  }

  _commandUpgrades(owner = this._commandOwnerForSelection()) {
    const policy = this.controlPolicy || this.state?.controlPolicy;
    if (typeof policy?.commandUpgrades === "function") {
      return policy.commandUpgrades(this.state, owner);
    }
    return this.state.upgrades || [];
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
    return emptyCommandSlot();
  }

  _syncAbilityCooldownClocks(abilityAffordances) {
    if (!this.elCommand || typeof this.elCommand.querySelector !== "function") return;
    for (const affordance of abilityAffordances) {
      const button = this.elCommand.querySelector(
        `button[data-ability="${affordance.definition.ability}"]`,
      );
      if (!button) continue;
      this._recordHudDiagnostic("hud.dirty.abilityCooldownClocks.sync");
      this._syncCooldownClockElement(button, affordance.cooldownClocks);
    }
  }

  _recordHudDiagnostic(label, amount = 1) {
    this._profiler?.recordDiagnosticCounter?.(label, amount);
  }

  _syncCooldownClockElement(button, cooldownClocks) {
    syncCooldownClockElement(button, cooldownClocks);
  }

  _cmdButton(opts) {
    return createCommandButton(opts);
  }

  _resourceIcon(kind) {
    return resourceIconHtml(kind);
  }
}

function issueGameplayCommand(sender, command, options = {}) {
  if (sender && typeof sender.issueCommand === "function") {
    return sender.issueCommand(command, options);
  }
  if (sender && typeof sender.command === "function" && sender.command.length < 2) {
    return sender.command(command);
  }
  return false;
}

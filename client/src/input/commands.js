import {
  ABILITY,
  ABILITY_OBJECT_KIND,
  cmd,
  PASSABLE,
  isUnit,
  isBuilding,
  isResource,
  KIND,
  ORDER_STAGE,
} from "../protocol.js";
import { ABILITIES, MINING_CC_RANGE_TILES, STATS, TANK_BODY, isProducerBuilding } from "../config.js";
import { DEFAULT_HIT_RADIUS, DEFAULT_TILE_SIZE, HIT_PAD_PX, OWN_HIT_BONUS, ZOOM_STEP } from "./constants.js";
import { commandHotkeyFromEvent } from "./placement.js";
import { armPostQuickCastSelectionGuard } from "./quick_cast_selection_guard.js";

export function _onRightClick(p, ev = {}) {
  const intent = clientIntent(this);
  if (intent?.activeLabTool) {
    cancelActiveLabTool(this, "rightClick");
    return;
  }
  // During placement, right-click cancels.
  if (intent?.placement) {
    this._cancelPlacementDrag?.();
    intent.endPlacement?.();
    return;
  }
  // Right-click also cancels a pending command-card target (consistent with Esc).
  if (intent?.commandTarget) {
    intent.endCommandTarget?.();
    return;
  }

  const ownUnits = this._selectedOwnUnitIds();
  const queued = !!ev.shiftKey;
  if (ownUnits.length === 0) {
    // No units selected: a buildings-only selection sets a rally point on any
    // unit-producing buildings in it. Units in the selection take priority, so a
    // mixed selection always moves the units (handled by the branch above).
    const producers = this._selectedProducerBuildingIds();
    if (producers.length > 0) {
      const world = this._worldAt(p.x, p.y);
      for (const building of producers) {
        this._issueCommand(cmd.setRally(building, world.x, world.y, queued, ORDER_STAGE.MOVE));
      }
      this._addCommandFeedback("move", world.x, world.y, queued);
    }
    return;
  }

  const world = this._worldAt(p.x, p.y);
  const workers = this._selectedWorkerIds();
  if (workers.length > 0) {
    const resource = this._resourceAtWorld(world.x, world.y);
    if (resource && resource.remaining !== 0) {
      this._issueCommand(cmd.gather(workers, resource.id, queued));
      this._addCommandFeedback("move", world.x, world.y, queued);
      return;
    }
  }

  const target = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ false);
  if (target && ownOwner(this.state, target.owner) && _isOwnIncompleteBuilding(target)) {
    const resume = _resumeConstructionIntent(target, this.state.map);
    if (resume && workers.length > 0) {
      this._issueCommand(cmd.build(workers, resume.building, resume.tileX, resume.tileY, queued));
      this._addCommandFeedback("move", target.x, target.y, queued);
      return;
    }
  }
  if (target && workers.length > 0 && _isCompletedTankTrap(target)) {
    this._issueCommand(cmd.deconstruct(workers, target.id, queued));
    this._addCommandFeedback("move", target.x, target.y, queued);
    return;
  }
  if (target && enemyOwner(this.state, target.owner) && !isResource(target.kind)) {
    // Enemy entity -> attack.
    this._issueCommand(cmd.attack(ownUnits, target.id, queued));
    this._addCommandFeedback("attack", target.x, target.y, queued);
    return;
  }
  if (target && isResource(target.kind) && target.remaining !== 0) {
    // Resource node -> gather, but only with the workers in the selection.
    if (workers.length > 0) {
      this._issueCommand(cmd.gather(workers, target.id, queued));
      this._addCommandFeedback("move", world.x, world.y, queued);
      return;
    }
    // Selection has no workers: fall through to a move onto the node's position.
  }
  // Default -> move to the world point.
  this._issueCommand(cmd.move(ownUnits, world.x, world.y, queued));
  this._addCommandFeedback("move", world.x, world.y, queued);
}

export function _issueTargetedCommand(p, ev = {}) {
  const intent = clientIntent(this);
  const commandTarget = intent?.commandTarget;
  const ownUnits = this._selectedOwnUnitIds();
  const producers = ownUnits.length === 0 ? this._selectedProducerBuildingIds() : [];
  const world = this._worldAt(p.x, p.y);
  if (ownUnits.length === 0) {
    if (producers.length === 0) return;
    const queued = !!ev.shiftKey;
    if (commandTarget === "move" || commandTarget === "attack") {
      const kind = commandTarget === "attack" ? ORDER_STAGE.ATTACK_MOVE : ORDER_STAGE.MOVE;
      for (const building of producers) {
        this._issueCommand(cmd.setRally(building, world.x, world.y, queued, kind));
      }
      this._addCommandFeedback(kind === ORDER_STAGE.ATTACK_MOVE ? "attack" : "move", world.x, world.y, queued);
    }
    return;
  }
  if (commandTarget === "setupAntiTankGuns") {
    const antiTankGuns = this._selectedOwnAntiTankGunIds();
    if (antiTankGuns.length > 0) {
      const queued = !!ev.shiftKey;
      this._issueCommand(cmd.setupAntiTankGuns(antiTankGuns, world.x, world.y, queued));
      this._addCommandFeedback("move", world.x, world.y, queued);
    }
    return;
  }
  if (commandTarget?.kind === "ability") {
    const ability = commandTarget.ability;
    const definition = ABILITIES[ability];
    const carriers = definition?.carriers;
    const units = Array.isArray(carriers)
      ? this.state
          .selectedEntities()
          .filter((e) => ownOwner(this.state, e.owner) && carriers.includes(e.kind))
          .map((e) => e.id)
      : ownUnits;
    if (units.length === 0) return;
    const command = ability === ABILITY.POINT_FIRE
      ? cmd.pointFire(units, world.x, world.y, !!ev.shiftKey)
      : cmd.useAbility(ability, units, world.x, world.y, !!ev.shiftKey);
    this._issueCommand(command);
    this._addCommandFeedback(
      ability === ABILITY.MORTAR_FIRE ? "mortar" : ability === ABILITY.POINT_FIRE ? "artillery" : "attack",
      world.x,
      world.y,
      !!ev.shiftKey,
      definition?.radiusTiles,
    );
    return;
  }
  if (commandTarget === "move") {
    this._issueCommand(cmd.move(ownUnits, world.x, world.y, !!ev.shiftKey));
    this._addCommandFeedback("move", world.x, world.y, !!ev.shiftKey);
    return;
  }

  const target = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ false);
  if (target && enemyOwner(this.state, target.owner) && !isResource(target.kind)) {
    this._issueCommand(cmd.attack(ownUnits, target.id, !!ev.shiftKey));
    this._addCommandFeedback("attack", target.x, target.y, !!ev.shiftKey);
    return;
  }

  this._issueCommand(cmd.attackMove(ownUnits, world.x, world.y, !!ev.shiftKey));
  this._addCommandFeedback("attack", world.x, world.y, !!ev.shiftKey);
}

export function _selectedOwnUnitIds() {
  return this.state
    .selectedEntities()
    .filter((e) => ownOwner(this.state, e.owner) && isUnit(e.kind))
    .map((e) => e.id);
}

export function _selectedProducerBuildingIds() {
  return this.state
    .selectedEntities()
    .filter((e) => ownOwner(this.state, e.owner) && isBuilding(e.kind) && isProducerBuilding(e.kind))
    .map((e) => e.id);
}

export function _selectedWorkerIds() {
  return this.state
    .selectedEntities()
    .filter((e) => ownOwner(this.state, e.owner) && e.kind === KIND.WORKER)
    .map((e) => e.id);
}

export function _selectedOwnAntiTankGunIds() {
  return this.state
    .selectedEntities()
    .filter((e) => ownOwner(this.state, e.owner) && (e.kind === KIND.ANTI_TANK_GUN || e.kind === KIND.ARTILLERY))
    .map((e) => e.id);
}

function _isOwnIncompleteBuilding(target) {
  return (
    isBuilding(target.kind) &&
    typeof target.buildProgress === "number" &&
    target.buildProgress < 1
  );
}

function _isCompletedTankTrap(target) {
  return (
    target.kind === KIND.TANK_TRAP &&
    !(typeof target.buildProgress === "number" && target.buildProgress < 1)
  );
}

function _resumeConstructionIntent(target, map) {
  if (!map) return null;
  const stat = STATS[target.kind];
  if (!stat?.footW || !stat?.footH) return null;
  const tileSize = map.tileSize || DEFAULT_TILE_SIZE;
  const tileX = Math.round(target.x / tileSize - stat.footW * 0.5);
  const tileY = Math.round(target.y / tileSize - stat.footH * 0.5);
  if (!Number.isFinite(tileX) || !Number.isFinite(tileY)) return null;
  return { building: target.kind, tileX, tileY };
}

export function _refreshAbilityTargetPreview() {
  const intent = clientIntent(this);
  const target = intent?.commandTarget;
  if (!target || target.kind !== "ability" || !this.mouse) {
    intent?.updateAbilityTargetPreview?.(null);
    return;
  }
  const definition = ABILITIES[target.ability];
  if (!definition || !Array.isArray(definition.carriers) || !definition.rangeTiles) {
    intent?.updateAbilityTargetPreview?.(null);
    return;
  }
  const carriers = this.state
    .selectedEntities()
    .filter((e) => ownOwner(this.state, e.owner) && definition.carriers.includes(e.kind));
  if (carriers.length === 0) {
    intent?.updateAbilityTargetPreview?.(null);
    return;
  }
  const tileSize = this.state.map?.tileSize || 32;
  const rangePx = definition.rangeTiles * tileSize;
  const minRangePx = (definition.minRangeTiles || 0) * tileSize;
  const world = this._worldAt(this.mouse.x, this.mouse.y);
  let hoverInRange = false;
  let hoverInsideMinRange = false;
  for (const c of carriers) {
    const dist = Math.hypot(world.x - c.x, world.y - c.y);
    if (minRangePx > 0 && dist < minRangePx) {
      hoverInsideMinRange = true;
    }
    if (dist <= rangePx && dist >= minRangePx) {
      hoverInRange = true;
      break;
    }
  }
  const abilityObjects = Array.isArray(this.state.abilityObjects) ? this.state.abilityObjects : [];
  const returnMarkers = abilityObjects
    .filter((object) =>
      object.kind === ABILITY_OBJECT_KIND.RETURN_MARKER &&
      object.ability === target.ability &&
      ownOwner(this.state, object.owner) &&
      Number.isFinite(object.x) &&
      Number.isFinite(object.y))
    .map((object) => ({
      id: object.id,
      kind: object.kind,
      x: object.x,
      y: object.y,
      radiusPx: 13,
      expiresIn: object.expiresIn,
    }));
  const anchorOrigins = target.ability === ABILITY.EKAT_LINE_SHOT
    ? abilityObjects
      .filter((object) =>
        object.kind === ABILITY_OBJECT_KIND.MAGIC_ANCHOR &&
        ownOwner(this.state, object.owner) &&
        Number.isFinite(object.x) &&
        Number.isFinite(object.y))
      .map((object) => ({
        id: object.id,
        kind: object.kind,
        x: object.x,
        y: object.y,
        radiusPx: object.ownerState?.radius || 8,
        expiresIn: object.expiresIn,
      }))
    : [];
  const carrierOrigins = carriers.map((carrier) => ({
    id: carrier.id,
    kind: carrier.kind,
    x: carrier.x,
    y: carrier.y,
    radiusPx: Math.max(5, (STATS[carrier.kind]?.size || 8) * 0.45),
  }));
  intent?.updateAbilityTargetPreview?.({
    ability: target.ability,
    mouseX: world.x,
    mouseY: world.y,
    carriers,
    rangeOrigins: carrierOrigins,
    pathOrigins: target.ability === ABILITY.EKAT_LINE_SHOT
      ? carrierOrigins.concat(anchorOrigins)
      : [],
    returnMarkers,
    rangePx,
    minRangePx,
    radiusPx: (definition.radiusTiles || 0) * tileSize,
    hoverInRange,
    hoverInsideMinRange,
  });
}

export function _refreshAntiTankGunSetupPreview() {
  const intent = clientIntent(this);
  if (!this.mouse || intent?.commandTarget !== "setupAntiTankGuns") {
    intent?.updateAntiTankGunSetupPreview?.(null);
    return;
  }
  const guns = this.state
    .selectedEntities()
    .filter((e) => ownOwner(this.state, e.owner) && (e.kind === KIND.ANTI_TANK_GUN || e.kind === KIND.ARTILLERY));
  if (guns.length === 0) {
    intent?.updateAntiTankGunSetupPreview?.(null);
    return;
  }
  const world = this._worldAt(this.mouse.x, this.mouse.y);
  intent?.updateAntiTankGunSetupPreview?.({ mouseX: world.x, mouseY: world.y, guns });
}

export function _refreshResourceMiningPreview() {
  const intent = clientIntent(this);
  if (this._drag || intent?.commandTarget || !this.mouse || this._selectedWorkerIds().length === 0) {
    intent?.updateResourceMiningPreview?.(null);
    return;
  }

  const world = this._worldAt(this.mouse.x, this.mouse.y);
  const target = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ false);
  if (!target || !isResource(target.kind) || target.remaining === 0) {
    intent?.updateResourceMiningPreview?.(null);
    return;
  }

  const nearest = this._nearestOwnCompletedCityCentre(target.x, target.y);
  if (!nearest) {
    intent?.updateResourceMiningPreview?.(null);
    return;
  }

  const rangePx = MINING_CC_RANGE_TILES * (this.state.map?.tileSize || DEFAULT_TILE_SIZE);
  intent?.updateResourceMiningPreview?.({
    resourceId: target.id,
    resourceX: target.x,
    resourceY: target.y,
    ccId: nearest.id,
    ccX: nearest.x,
    ccY: nearest.y,
    inRange: nearest.dist <= rangePx + 0.001,
  });
}

export function _nearestOwnCompletedCityCentre(x, y) {
  let best = null;
  for (const e of this.state.entitiesInterpolated(1)) {
    if (
      !ownOwner(this.state, e.owner) ||
      e.kind !== KIND.CITY_CENTRE ||
      (typeof e.buildProgress === "number" && e.buildProgress < 1)
    ) {
      continue;
    }
    const dist = Math.hypot(e.x - x, e.y - y);
    if (!best || dist < best.dist || (dist === best.dist && e.id < best.id)) {
      best = { id: e.id, x: e.x, y: e.y, dist };
    }
  }
  return best;
}

function ownOwner(state, owner) {
  if (state?.controlPolicy?.kind === "lab") {
    return state.controlPolicy.canControlOwner(owner, state);
  }
  return typeof state?.isOwnOwner === "function"
    ? state.isOwnOwner(owner)
    : Number(owner) === state?.playerId;
}

function enemyOwner(state, owner) {
  if (typeof state?.isEnemyOwner === "function") return state.isEnemyOwner(owner);
  const ownerId = Number(owner);
  return Number.isInteger(ownerId) && ownerId !== 0 && ownerId !== state?.playerId;
}

export function _activateCommandHotkey(ev) {
  const key = commandHotkeyFromEvent(ev);
  if (!key) return false;
  const card = document.getElementById("command-card");
  if (!card) return false;
  for (const btn of card.querySelectorAll("button[data-hotkey]")) {
    if ((btn.dataset.hotkey || "").toUpperCase() !== key) continue;
    if (ev.repeat && btn.dataset.repeatable !== "true") return false;
    ev.preventDefault();
    if (!btn.disabled) {
      if (typeof MouseEvent === "function" && typeof btn.dispatchEvent === "function") {
        btn.dispatchEvent(new MouseEvent("click", {
          bubbles: true,
          cancelable: true,
          altKey: !!ev.altKey,
          ctrlKey: !!ev.ctrlKey,
          metaKey: !!ev.metaKey,
          shiftKey: !!ev.shiftKey,
        }));
      } else {
        btn.click();
      }
    }
    return {
      handled: true,
      commandId: btn.dataset.commandId || null,
      hotkey: btn.dataset.hotkey || null,
      slotIndex: btn.dataset.slotIndex != null ? Number(btn.dataset.slotIndex) : null,
      armed: clientIntent(this)?.lastCommandTargetArm || null,
    };
  }
  return false;
}

export function _quickCastCommandTarget(ev = {}) {
  const intent = clientIntent(this);
  if (!intent?.commandTarget || !this.mouse) return false;
  const quickCastPoint = { x: this.mouse.x, y: this.mouse.y };
  this._issueTargetedCommand(this.mouse, ev);
  const issued = typeof intent.issueCommandTarget === "function"
    ? intent.issueCommandTarget(ev)
    : { keepArmed: false };
  if (!issued.keepArmed) {
    intent.endCommandTarget?.();
    armPostQuickCastSelectionGuard(this, quickCastPoint);
  }
  return true;
}

export function _cancel() {
  const intent = clientIntent(this);
  if (typeof intent?.closeCommandCardMenu === "function" && intent.closeCommandCardMenu()) {
    return;
  }
  if (intent?.activeLabTool) {
    cancelActiveLabTool(this, "escape");
    return;
  }
  if (intent?.placement) {
    this._cancelPlacementDrag?.();
    intent.endPlacement?.();
    return;
  }
  if (intent?.commandTarget) {
    intent.endCommandTarget?.();
    return;
  }
  this.state.clearSelection();
}

function clientIntent(input) {
  return input?.clientIntent || null;
}

function cancelActiveLabTool(input, reason) {
  const intent = clientIntent(input);
  if (!intent?.activeLabTool) return null;
  const cancelled = input?.labToolController?.cancel?.(reason);
  return cancelled || intent.cancelLabTool?.(reason) || null;
}

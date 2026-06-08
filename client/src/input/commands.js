import { cmd, PASSABLE, isUnit, isBuilding, isResource, KIND } from "../protocol.js";
import { ABILITIES, MINING_CC_RANGE_TILES, STATS, TANK_BODY, isProducerBuilding } from "../config.js";
import { DEFAULT_HIT_RADIUS, DEFAULT_TILE_SIZE, HIT_PAD_PX, OWN_HIT_BONUS, ZOOM_STEP } from "./constants.js";
import { commandHotkeyFromEvent } from "./placement.js";

export function _onRightClick(p, ev = {}) {
  // During placement, right-click cancels.
  if (this.state.placement) {
    this.state.endPlacement();
    return;
  }
  // Right-click also cancels a pending command-card target (consistent with Esc).
  if (this.state.commandTarget) {
    this.state.endCommandTarget();
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
        this.net.command(cmd.setRally(building, world.x, world.y, false));
      }
      this.state.addCommandFeedback("move", world.x, world.y, false);
    }
    return;
  }

  const me = this.state.playerId;
  const world = this._worldAt(p.x, p.y);
  const workers = this._selectedWorkerIds();
  if (workers.length > 0) {
    const resource = this._resourceAtWorld(world.x, world.y);
    if (resource && resource.remaining !== 0) {
      this.net.command(cmd.gather(workers, resource.id, queued));
      this.state.addCommandFeedback("move", world.x, world.y, queued);
      return;
    }
  }

  const target = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ false);
  if (target && target.owner === me && _isOwnIncompleteBuilding(target)) {
    const resume = _resumeConstructionIntent(target, this.state.map);
    if (resume && workers.length > 0) {
      for (const worker of workers) {
        this.net.command(cmd.build(worker, resume.building, resume.tileX, resume.tileY, queued));
      }
      this.state.addCommandFeedback("move", target.x, target.y, queued);
      return;
    }
  }
  if (target && target.owner !== me && target.owner !== 0 && !isResource(target.kind)) {
    // Enemy entity -> attack.
    this.net.command(cmd.attack(ownUnits, target.id, queued));
    this.state.addCommandFeedback("attack", target.x, target.y, queued);
    return;
  }
  if (target && isResource(target.kind) && target.remaining !== 0) {
    // Resource node -> gather, but only with the workers in the selection.
    if (workers.length > 0) {
      this.net.command(cmd.gather(workers, target.id, queued));
      this.state.addCommandFeedback("move", world.x, world.y, queued);
      return;
    }
    // Selection has no workers: fall through to a move onto the node's position.
  }
  // Default -> move to the world point.
  this.net.command(cmd.move(ownUnits, world.x, world.y, queued));
  this.state.addCommandFeedback("move", world.x, world.y, queued);
}

export function _issueTargetedCommand(p, ev = {}) {
  const ownUnits = this._selectedOwnUnitIds();
  if (ownUnits.length === 0) return;
  const world = this._worldAt(p.x, p.y);
  if (this.state.commandTarget === "setupAtGuns") {
    const atGuns = this._selectedOwnAtGunIds();
    if (atGuns.length > 0) {
      const queued = !!ev.shiftKey;
      this.net.command(cmd.setupAtGuns(atGuns, world.x, world.y, queued));
      this.state.addCommandFeedback("move", world.x, world.y, queued);
    }
    return;
  }
  if (this.state.commandTarget?.kind === "ability") {
    const ability = this.state.commandTarget.ability;
    const definition = ABILITIES[ability];
    const carriers = definition?.carriers;
    const units = Array.isArray(carriers)
      ? this.state
          .selectedEntities()
          .filter((e) => e.owner === this.state.playerId && carriers.includes(e.kind))
          .map((e) => e.id)
      : ownUnits;
    if (units.length === 0) return;
    this.net.command(cmd.useAbility(ability, units, world.x, world.y, !!ev.shiftKey));
    this.state.addCommandFeedback("attack", world.x, world.y, !!ev.shiftKey);
    return;
  }
  if (this.state.commandTarget === "move") {
    this.net.command(cmd.move(ownUnits, world.x, world.y, !!ev.shiftKey));
    this.state.addCommandFeedback("move", world.x, world.y, !!ev.shiftKey);
    return;
  }

  const target = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ false);
  const me = this.state.playerId;
  if (target && target.owner !== me && target.owner !== 0 && !isResource(target.kind)) {
    this.net.command(cmd.attack(ownUnits, target.id, !!ev.shiftKey));
    this.state.addCommandFeedback("attack", target.x, target.y, !!ev.shiftKey);
    return;
  }

  this.net.command(cmd.attackMove(ownUnits, world.x, world.y, !!ev.shiftKey));
  this.state.addCommandFeedback("attack", world.x, world.y, !!ev.shiftKey);
}

export function _selectedOwnUnitIds() {
  const me = this.state.playerId;
  return this.state
    .selectedEntities()
    .filter((e) => e.owner === me && isUnit(e.kind))
    .map((e) => e.id);
}

export function _selectedProducerBuildingIds() {
  const me = this.state.playerId;
  return this.state
    .selectedEntities()
    .filter((e) => e.owner === me && isBuilding(e.kind) && isProducerBuilding(e.kind))
    .map((e) => e.id);
}

export function _selectedWorkerIds() {
  const me = this.state.playerId;
  return this.state
    .selectedEntities()
    .filter((e) => e.owner === me && e.kind === KIND.WORKER)
    .map((e) => e.id);
}

export function _selectedOwnAtGunIds() {
  const me = this.state.playerId;
  return this.state
    .selectedEntities()
    .filter((e) => e.owner === me && e.kind === KIND.AT_TEAM)
    .map((e) => e.id);
}

function _isOwnIncompleteBuilding(target) {
  return (
    isBuilding(target.kind) &&
    typeof target.buildProgress === "number" &&
    target.buildProgress < 1
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
  const target = this.state.commandTarget;
  if (!target || target.kind !== "ability" || !this.mouse) {
    this.state.updateAbilityTargetPreview(null);
    return;
  }
  const definition = ABILITIES[target.ability];
  if (!definition || !Array.isArray(definition.carriers) || !definition.rangeTiles) {
    this.state.updateAbilityTargetPreview(null);
    return;
  }
  const me = this.state.playerId;
  const carriers = this.state
    .selectedEntities()
    .filter((e) => e.owner === me && definition.carriers.includes(e.kind));
  if (carriers.length === 0) {
    this.state.updateAbilityTargetPreview(null);
    return;
  }
  const tileSize = this.state.map?.tileSize || 32;
  const rangePx = definition.rangeTiles * tileSize;
  const world = this._worldAt(this.mouse.x, this.mouse.y);
  let hoverInRange = false;
  for (const c of carriers) {
    if (Math.hypot(world.x - c.x, world.y - c.y) <= rangePx) {
      hoverInRange = true;
      break;
    }
  }
  this.state.updateAbilityTargetPreview({
    ability: target.ability,
    mouseX: world.x,
    mouseY: world.y,
    carriers,
    rangePx,
    radiusPx: (definition.radiusTiles || 0) * tileSize,
    hoverInRange,
  });
}

export function _refreshAtGunSetupPreview() {
  if (!this.mouse || this.state.commandTarget !== "setupAtGuns") {
    this.state.updateAtGunSetupPreview(null);
    return;
  }
  const me = this.state.playerId;
  const guns = this.state
    .selectedEntities()
    .filter((e) => e.owner === me && e.kind === KIND.AT_TEAM);
  if (guns.length === 0) {
    this.state.updateAtGunSetupPreview(null);
    return;
  }
  const world = this._worldAt(this.mouse.x, this.mouse.y);
  this.state.updateAtGunSetupPreview({ mouseX: world.x, mouseY: world.y, guns });
}

export function _refreshResourceMiningPreview() {
  if (this._drag || this.state.commandTarget || !this.mouse || this._selectedWorkerIds().length === 0) {
    this.state.updateResourceMiningPreview(null);
    return;
  }

  const world = this._worldAt(this.mouse.x, this.mouse.y);
  const target = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ false);
  if (!target || !isResource(target.kind) || target.remaining === 0) {
    this.state.updateResourceMiningPreview(null);
    return;
  }

  const nearest = this._nearestOwnCompletedCityCentre(target.x, target.y);
  if (!nearest) {
    this.state.updateResourceMiningPreview(null);
    return;
  }

  const rangePx = MINING_CC_RANGE_TILES * (this.state.map?.tileSize || DEFAULT_TILE_SIZE);
  this.state.updateResourceMiningPreview({
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
  const me = this.state.playerId;
  let best = null;
  for (const e of this.state.entitiesInterpolated(1)) {
    if (
      e.owner !== me ||
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

export function _activateCommandHotkey(ev) {
  const key = commandHotkeyFromEvent(ev);
  if (!key) return false;
  const card = document.getElementById("command-card");
  if (!card) return false;
  for (const btn of card.querySelectorAll("button[data-hotkey]")) {
    if ((btn.dataset.hotkey || "").toUpperCase() !== key) continue;
    ev.preventDefault();
    if (!btn.disabled) btn.click();
    return true;
  }
  return false;
}

export function _enterAttackMove() {
  // Only meaningful when own units are selected; otherwise it's a no-op arming.
  if (this._selectedOwnUnitIds().length === 0) return;
  this.state.beginCommandTarget("attack");
}

export function _issueStop() {
  const ownUnits = this._selectedOwnUnitIds();
  if (ownUnits.length === 0) return;
  this.net.command(cmd.stop(ownUnits));
}

export function _cancel() {
  if (typeof this.state.closeCommandCardMenu === "function" && this.state.closeCommandCardMenu()) {
    return;
  }
  if (this.state.placement) {
    this.state.endPlacement();
    return;
  }
  if (this.state.commandTarget) {
    this.state.endCommandTarget();
    return;
  }
  this.state.clearSelection();
}

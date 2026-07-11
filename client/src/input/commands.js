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
  UPGRADE,
} from "../protocol.js";
import { ABILITIES, MINING_CC_RANGE_TILES, STATS, TANK_BODY, isProducerBuilding } from "../config.js";
import { DEFAULT_HIT_RADIUS, DEFAULT_TILE_SIZE, HIT_PAD_PX, OWN_HIT_BONUS, ZOOM_STEP } from "./constants.js";
import {
  buildArtilleryTargetLocks,
  isArtilleryFireAbility,
} from "./artillery_targeting.js";
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

  const queued = !!ev.shiftKey;
  issueNormalRightClickAction(this, normalRightClickAction(this, p), queued);
}

export function _issueTargetedCommand(p, ev = {}) {
  const intent = clientIntent(this);
  const commandTarget = intent?.commandTarget;
  const ownUnits = this._selectedOwnUnitIds();
  const landUnits = selectedOwnLandUnitIds(this, ownUnits);
  const producers = ownUnits.length === 0 ? this._selectedProducerBuildingIds() : [];
  const pickedTarget = commandTarget === "attack"
    ? this._entityAtScreen(p, /*ownPreferred=*/ false)
    : null;
  const world = this._groundAtScreen(p.x, p.y);
  if (!world && !explicitAttackCommandTarget(this.state, pickedTarget)) return false;
  if (ownUnits.length === 0) {
    if (producers.length === 0 || !world) return true;
    const queued = !!ev.shiftKey;
    if (commandTarget === "move" || commandTarget === "attack") {
      const kind = commandTarget === "attack" ? ORDER_STAGE.ATTACK_MOVE : ORDER_STAGE.MOVE;
      for (const building of producers) {
        this._issueCommand(cmd.setRally(building, world.x, world.y, queued, kind));
      }
      this._addCommandFeedback(kind === ORDER_STAGE.ATTACK_MOVE ? "attack" : "move", world.x, world.y, queued);
    }
    return true;
  }
  if (commandTarget === "setupAntiTankGuns") {
    if (!world) return false;
    const antiTankGuns = this._selectedOwnAntiTankGunIds();
    if (antiTankGuns.length > 0) {
      const queued = !!ev.shiftKey;
      this._issueCommand(cmd.setupAntiTankGuns(antiTankGuns, world.x, world.y, queued));
      this._addCommandFeedback("move", world.x, world.y, queued);
    }
    return true;
  }
  if (commandTarget?.kind === "ability") {
    if (!world) return false;
    const ability = commandTarget.ability;
    const definition = ABILITIES[ability];
    const carriers = definition?.carriers;
    const units = Array.isArray(carriers)
      ? this.state
          .selectedEntities()
          .filter((e) => ownOwner(this.state, e.owner) && carriers.includes(e.kind))
          .map((e) => e.id)
      : ownUnits;
    if (units.length === 0) return true;
    const queued = !!ev.shiftKey;
    const command = ability === ABILITY.POINT_FIRE
      ? cmd.pointFire(units, world.x, world.y, queued)
      : ability === ABILITY.BLANKET_FIRE
        ? cmd.blanketFire(units, world.x, world.y, queued)
        : cmd.useAbility(ability, units, world.x, world.y, queued);
    const selectedCarriers = this.state.selectedEntities().filter((e) => units.includes(e.id));
    const radiusTiles = abilityTargetRadiusTiles(definition, ability, this.state);
    const artilleryLocks = isArtilleryFireAbility(ability)
      ? buildArtilleryTargetLocks({
        ability,
        carriers: selectedCarriers.map((e) => plannedEntityForIntent(intent, e)),
        rawX: world.x,
        rawY: world.y,
        map: this.state.map,
        tileSize: this.state.map?.tileSize || DEFAULT_TILE_SIZE,
        definition,
        queued,
      })
      : [];
    this._issueCommand(command);
    if (isArtilleryFireAbility(ability)) {
      for (const lock of artilleryLocks) {
        this._addCommandFeedback("artillery", lock.x, lock.y, queued, radiusTiles);
      }
      return true;
    }
    const feedbackKind = ability === ABILITY.MORTAR_FIRE
      ? "mortar"
      : "attack";
    this._addCommandFeedback(feedbackKind, world.x, world.y, queued, radiusTiles);
    return true;
  }
  if (commandTarget === "move") {
    if (!world) return false;
    this._issueCommand(cmd.move(ownUnits, world.x, world.y, !!ev.shiftKey));
    this._addCommandFeedback("move", world.x, world.y, !!ev.shiftKey);
    return true;
  }

  const target = pickedTarget;
  if (explicitAttackCommandTarget(this.state, target)) {
    const issuedAttack = issueTargetAttackForLandUnits(this, landUnits, target, !!ev.shiftKey);
    if (issuedAttack) {
      this._addCommandFeedback("attack", target.x, target.y, !!ev.shiftKey);
    }
    return true;
  }

  if (!world) return false;
  const issuedAttackMove = issueAttackMoveForLandUnits(this, landUnits, world.x, world.y, !!ev.shiftKey);
  if (issuedAttackMove) {
    this._addCommandFeedback("attack", world.x, world.y, !!ev.shiftKey);
  }
  return true;
}

function normalRightClickAction(input, p) {
  const ownUnits = input._selectedOwnUnitIds();
  const landUnits = selectedOwnLandUnitIds(input, ownUnits);
  const world = input._groundAtScreen(p.x, p.y);
  const resource = input._resourceAtScreen(p);
  const target = input._entityAtScreen(p, /*ownPreferred=*/ false);
  if (ownUnits.length === 0) {
    // No units selected: a buildings-only selection sets a rally point on any
    // unit-producing buildings in it. Units in the selection take priority, so a
    // mixed selection always moves the units.
    const producers = input._selectedProducerBuildingIds();
    if (producers.length === 0 || !world) return null;
    return {
      kind: "setRally",
      producers,
      x: world.x,
      y: world.y,
      stage: ORDER_STAGE.MOVE,
      feedback: rightClickFeedback("move", world.x, world.y),
    };
  }

  const gatherers = input._selectedGathererIds();
  const workers = input._selectedWorkerIds();
  if (resource && resource.remaining !== 0) {
    const action = resourceRightClickAction(resource, world || resource, gatherers, workers, input.state.map);
    if (action) return action;
  }

  if (target && ownOwner(input.state, target.owner) && _isOwnIncompleteBuilding(target)) {
    const resume = _resumeConstructionIntent(target, input.state.map);
    if (resume && workers.length > 0) {
      return {
        kind: "build",
        units: workers,
        building: resume.building,
        tileX: resume.tileX,
        tileY: resume.tileY,
        feedback: rightClickFeedback("move", target.x, target.y),
      };
    }
  }
  if (target && workers.length > 0 && _isCompletedTankTrap(target)) {
    return {
      kind: "deconstruct",
      units: workers,
      target,
      feedback: rightClickFeedback("move", target.x, target.y),
    };
  }
  if (target && enemyOwner(input.state, target.owner) && isAttackableEntityTarget(target)) {
    return {
      kind: "attack",
      units: landUnits,
      target,
      feedback: rightClickFeedback("attack", target.x, target.y),
    };
  }
  if (target && isResource(target.kind) && target.remaining !== 0) {
    const action = resourceRightClickAction(target, world || target, gatherers, workers, input.state.map);
    if (action) return action;
    // Selection has no gatherers: fall through to a move onto the node's position.
  }
  if (!world) return null;
  return {
    kind: "move",
    units: ownUnits,
    x: world.x,
    y: world.y,
    feedback: rightClickFeedback("move", world.x, world.y),
  };
}

function resourceRightClickAction(resource, world, gatherers, workers, map) {
  const pumpJack = _pumpJackBuildIntentForResource(resource, map);
  if (resource.kind === KIND.OIL && workers.length > 0 && pumpJack) {
    return {
      kind: "build",
      units: workers,
      building: KIND.PUMP_JACK,
      tileX: pumpJack.tileX,
      tileY: pumpJack.tileY,
      feedback: rightClickFeedback("move", resource.x, resource.y),
    };
  }
  if (gatherers.length > 0 && resource.kind !== KIND.OIL) {
    return {
      kind: "gather",
      units: gatherers,
      target: resource,
      feedback: rightClickFeedback("move", world.x, world.y),
    };
  }
  return null;
}

function issueNormalRightClickAction(input, action, queued) {
  if (!action) return;
  switch (action.kind) {
    case "setRally":
      for (const building of action.producers) {
        input._issueCommand(cmd.setRally(building, action.x, action.y, queued, action.stage));
      }
      break;
    case "build":
      input._issueCommand(cmd.build(action.units, action.building, action.tileX, action.tileY, queued));
      break;
    case "gather":
      input._issueCommand(cmd.gather(action.units, action.target.id, queued));
      break;
    case "deconstruct":
      input._issueCommand(cmd.deconstruct(action.units, action.target.id, queued));
      break;
    case "attack":
      if (action.units.length > 0) {
        input._issueCommand(cmd.attack(action.units, action.target.id, queued));
      }
      break;
    case "move":
      input._issueCommand(cmd.move(action.units, action.x, action.y, queued));
      break;
    default:
      return;
  }
  if (action.feedback) {
    input._addCommandFeedback(action.feedback.kind, action.feedback.x, action.feedback.y, queued);
  }
}

function issueTargetAttackForLandUnits(input, landUnits, target, queued) {
  if (landUnits.length > 0) {
    input._issueCommand(cmd.attack(landUnits, target.id, queued));
    return true;
  }
  return false;
}

function issueAttackMoveForLandUnits(input, landUnits, x, y, queued) {
  if (landUnits.length > 0) {
    input._issueCommand(cmd.attackMove(landUnits, x, y, queued));
    return true;
  }
  return false;
}

function rightClickFeedback(kind, x, y) {
  return { kind, x, y };
}

function explicitAttackCommandTarget(state, target) {
  return !!target &&
    isAttackableEntityTarget(target) &&
    (ownOwner(state, target.owner) || enemyOwner(state, target.owner));
}

function isAttackableEntityTarget(target) {
  return !!target && !isResource(target.kind) && target.kind !== KIND.SCOUT_PLANE;
}

function attackTargetPreviewForRightClickAction(action) {
  if (action?.kind !== "attack" || !action.target) return null;
  return {
    targetId: action.target.id,
    kind: action.target.kind,
    x: action.target.x,
    y: action.target.y,
  };
}

export function _selectedOwnUnitIds() {
  return selectedEntities(this.state)
    .filter((e) => ownOwner(this.state, e.owner) && isUnit(e.kind) && e.kind !== KIND.SCOUT_PLANE)
    .map((e) => e.id);
}

export function _selectedOwnLandUnitIds() {
  return selectedOwnLandUnitIds(this, this._selectedOwnUnitIds?.() || []);
}

export function _selectedProducerBuildingIds() {
  return selectedEntities(this.state)
    .filter((e) => ownOwner(this.state, e.owner) && isBuilding(e.kind) && isProducerBuilding(e.kind))
    .map((e) => e.id);
}

export function _selectedWorkerIds() {
  return selectedEntities(this.state)
    .filter((e) => ownOwner(this.state, e.owner) && e.kind === KIND.WORKER)
    .map((e) => e.id);
}

export function _selectedGathererIds() {
  return selectedEntities(this.state)
    .filter((e) =>
      ownOwner(this.state, e.owner) &&
      (e.kind === KIND.WORKER || e.kind === KIND.GOLEM))
    .map((e) => e.id);
}

export function _selectedOwnAntiTankGunIds() {
  return selectedEntities(this.state)
    .filter((e) => ownOwner(this.state, e.owner) && (e.kind === KIND.ANTI_TANK_GUN || e.kind === KIND.ARTILLERY))
    .map((e) => e.id);
}

function selectedEntities(state) {
  return typeof state?.selectedEntities === "function" ? state.selectedEntities() || [] : [];
}

function selectedOwnUnitEntities(input) {
  return selectedEntities(input.state).filter((e) =>
    ownOwner(input.state, e.owner) && isUnit(e.kind) && e.kind !== KIND.SCOUT_PLANE);
}

function selectedOwnLandUnitIds(input, fallbackUnitIds = []) {
  const units = selectedOwnUnitEntities(input);
  if (units.length === 0) return fallbackUnitIds.slice();
  return units.map((e) => e.id);
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

export function _pumpJackBuildIntentForResource(resource, map) {
  if (!resource || resource.kind !== KIND.OIL || !map) return null;
  const stat = STATS[KIND.PUMP_JACK];
  if (!stat?.footW || !stat?.footH) return null;
  const tileSize = map.tileSize || DEFAULT_TILE_SIZE;
  const tileX = Math.round(resource.x / tileSize - stat.footW * 0.5);
  const tileY = Math.round(resource.y / tileSize - stat.footH * 0.5);
  if (!Number.isFinite(tileX) || !Number.isFinite(tileY)) return null;
  return { building: KIND.PUMP_JACK, tileX, tileY };
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
    .filter((e) => ownOwner(this.state, e.owner) && definition.carriers.includes(e.kind))
    .map((e) => plannedEntityForIntent(intent, e));
  if (carriers.length === 0) {
    intent?.updateAbilityTargetPreview?.(null);
    return;
  }
  const tileSize = this.state.map?.tileSize || 32;
  const rangePx = definition.rangeTiles * tileSize;
  const minRangePx = (definition.minRangeTiles || 0) * tileSize;
  const world = this._groundAtScreen(this.mouse.x, this.mouse.y);
  if (!world) {
    intent?.updateAbilityTargetPreview?.(null);
    return;
  }
  const locksRangeBand = isArtilleryFireAbility(target.ability);
  let hoverInRange = false;
  let hoverInsideMinRange = false;
  let artilleryLocks = [];
  if (locksRangeBand) {
    artilleryLocks = buildArtilleryTargetLocks({
      ability: target.ability,
      carriers,
      rawX: world.x,
      rawY: world.y,
      map: this.state.map,
      tileSize,
      definition,
      queued: setupPreviewQueued(this, intent),
    });
    hoverInRange = artilleryLocks.length > 0;
  }
  if (!locksRangeBand) {
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
  const primaryLock = artilleryLocks[0] || null;
  const radiusTiles = abilityTargetRadiusTiles(definition, target.ability, this.state);
  intent?.updateAbilityTargetPreview?.({
    ability: target.ability,
    mouseX: primaryLock?.x ?? world.x,
    mouseY: primaryLock?.y ?? world.y,
    rawMouseX: world.x,
    rawMouseY: world.y,
    carriers,
    artilleryLocks,
    rangeOrigins: carrierOrigins,
    pathOrigins: target.ability === ABILITY.EKAT_LINE_SHOT
      ? carrierOrigins.concat(anchorOrigins)
      : [],
    returnMarkers,
    rangePx,
    minRangePx,
    radiusPx: radiusTiles * tileSize,
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
    .filter((e) => ownOwner(this.state, e.owner) && (e.kind === KIND.ANTI_TANK_GUN || e.kind === KIND.ARTILLERY))
    .map((e) => supportWeaponSetupPreviewEntity(plannedEntityForIntent(intent, e), setupPreviewQueued(this, intent)));
  if (guns.length === 0) {
    intent?.updateAntiTankGunSetupPreview?.(null);
    return;
  }
  const world = this._groundAtScreen(this.mouse.x, this.mouse.y);
  if (!world) {
    intent?.updateAntiTankGunSetupPreview?.(null);
    return;
  }
  intent?.updateAntiTankGunSetupPreview?.({ mouseX: world.x, mouseY: world.y, guns });
}

function setupPreviewQueued(input, intent) {
  return intent?.commandComposer?.shiftPreserved === true || input?._shiftKeyDown === true;
}

function supportWeaponSetupPreviewEntity(entity, queued) {
  if (!queued) return entity;
  const origin = latestMovementOrderPlanPoint(entity);
  if (!origin) return entity;
  return { ...entity, x: origin.x, y: origin.y };
}

function plannedEntityForIntent(intent, entity) {
  return typeof intent?.entityWithPlannedOrder === "function"
    ? intent.entityWithPlannedOrder(entity)
    : entity;
}

function latestMovementOrderPlanPoint(entity) {
  if (!Array.isArray(entity?.orderPlan)) return null;
  let origin = null;
  for (const marker of entity.orderPlan) {
    if (
      (marker?.kind === ORDER_STAGE.MOVE || marker?.kind === ORDER_STAGE.ATTACK_MOVE) &&
      Number.isFinite(marker.x) &&
      Number.isFinite(marker.y)
    ) {
      origin = { x: marker.x, y: marker.y };
    }
  }
  return origin;
}

export function _refreshResourceMiningPreview() {
  const intent = clientIntent(this);
  if (this._drag || intent?.commandTarget || !this.mouse || this._selectedGathererIds().length === 0) {
    intent?.updateResourceMiningPreview?.(null);
    return;
  }

  const target = this._entityAtScreen(this.mouse, /*ownPreferred=*/ false);
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

export function _refreshAttackTargetPreview() {
  const intent = clientIntent(this);
  if (
    !intent ||
    this._drag ||
    intent?.activeLabTool ||
    intent?.placement ||
    intent?.commandTarget ||
    !this.mouse
  ) {
    intent?.updateAttackTargetPreview?.(null);
    return;
  }

  intent.updateAttackTargetPreview(attackTargetPreviewForRightClickAction(
    normalRightClickAction(this, this.mouse),
  ));
}

export function _nearestOwnCompletedCityCentre(x, y) {
  let best = null;
  for (const e of this._selectionEntities()) {
    if (
      !ownOwner(this.state, e.owner) ||
      (e.kind !== KIND.CITY_CENTRE && e.kind !== KIND.ZAMOK) ||
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
    if (typeof state.controlPolicy.isCommandOwner === "function") {
      return state.controlPolicy.isCommandOwner(owner, state);
    }
    return state.controlPolicy.canControlOwner(owner, state);
  }
  return typeof state?.isOwnOwner === "function"
    ? state.isOwnOwner(owner)
    : Number(owner) === state?.playerId;
}

function enemyOwner(state, owner) {
  if (state?.controlPolicy?.kind === "lab") {
    if (typeof state.controlPolicy.isCommandEnemyOwner === "function") {
      return state.controlPolicy.isCommandEnemyOwner(owner, state);
    }
    const commandOwner = typeof state.controlPolicy.commandOwner === "function"
      ? state.controlPolicy.commandOwner(state)
      : state.controlPolicy.issueAsOwnerForSelection?.(state.selectedEntities?.() || []);
    return fallbackEnemyOwner(commandOwner, owner);
  }
  if (typeof state?.isEnemyOwner === "function") return state.isEnemyOwner(owner);
  return fallbackEnemyOwner(state?.playerId, owner);
}

function fallbackEnemyOwner(commandOwner, owner) {
  const commandOwnerId = Number(commandOwner);
  const ownerId = Number(owner);
  return Number.isInteger(commandOwnerId) &&
    commandOwnerId > 0 &&
    Number.isInteger(ownerId) &&
    ownerId > 0 &&
    ownerId !== commandOwnerId;
}

function abilityTargetRadiusTiles(definition, ability, state) {
  const baseRadius = definition?.radiusTiles || 0;
  if (ability === ABILITY.SMOKE && commandUpgrades(state).includes(UPGRADE.SMOKE_PLUS)) {
    return definition?.upgradedRadiusTiles || baseRadius;
  }
  return baseRadius;
}

function commandUpgrades(state) {
  if (typeof state?.controlPolicy?.commandUpgrades === "function") {
    const upgrades = state.controlPolicy.commandUpgrades(state);
    return Array.isArray(upgrades) ? upgrades : [];
  }
  return Array.isArray(state?.upgrades) ? state.upgrades : [];
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
    const autocastToggle = ev.altKey && btn.dataset.autocastToggle === "true";
    if (autocastToggle) {
      dispatchCommandButtonMouseEvent(btn, "contextmenu", ev);
    } else if (!btn.disabled) {
      dispatchCommandButtonMouseEvent(btn, "click", ev);
    }
    return {
      handled: true,
      commandId: btn.dataset.commandId || null,
      hotkey: btn.dataset.hotkey || null,
      slotIndex: btn.dataset.slotIndex != null ? Number(btn.dataset.slotIndex) : null,
      autocastToggle,
      armed: clientIntent(this)?.lastCommandTargetArm || null,
    };
  }
  return false;
}

function dispatchCommandButtonMouseEvent(btn, type, ev) {
  if (typeof MouseEvent === "function" && typeof btn.dispatchEvent === "function") {
    btn.dispatchEvent(new MouseEvent(type, {
      bubbles: true,
      cancelable: true,
      altKey: !!ev.altKey,
      ctrlKey: !!ev.ctrlKey,
      metaKey: !!ev.metaKey,
      shiftKey: !!ev.shiftKey,
    }));
    return;
  }
  if (typeof btn.dispatchEvent === "function") {
    btn.dispatchEvent({
      type,
      bubbles: true,
      cancelable: true,
      altKey: !!ev.altKey,
      ctrlKey: !!ev.ctrlKey,
      metaKey: !!ev.metaKey,
      shiftKey: !!ev.shiftKey,
      preventDefault() {},
      stopPropagation() {},
    });
    return;
  }
  if (type === "click" && typeof btn.click === "function") btn.click();
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

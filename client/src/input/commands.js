import {
  ABILITY,
  ABILITY_OBJECT_KIND,
  cmd,
  isUnit,
  isBuilding,
  isResource,
  KIND,
  ORDER_STAGE,
  UPGRADE,
} from "../protocol.js";
import {
  ABILITIES,
  MINING_CC_RANGE_TILES,
  SCOUT_PLANE_SPEED_PX_PER_TICK,
  STATS,
  isProducerBuilding,
} from "../config.js";
import { DEFAULT_TILE_SIZE } from "./constants.js";
import {
  artilleryFireRadiusTiles,
  artilleryMinFireRadiusTiles,
  buildArtilleryTargetLocks,
  isArtilleryFireAbility,
} from "./artillery_targeting.js";
import {
  commandHotkeyCodeFromEvent,
  entityIntersectsRect,
  pumpJackBuildIntentForResource,
} from "./placement.js";
import { armPostQuickCastSelectionGuard } from "./quick_cast_selection_guard.js";
import {
  supportWeaponsWithSetupTargets,
} from "./support_weapon_setup_targeting.js";

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
  const resource = this._resourceAtScreen(p);
  const world = this._groundAtScreen(p.x, p.y);
  if (!world && !explicitAttackCommandTarget(this.state, pickedTarget, this.controlPolicy)) return false;
  if (ownUnits.length === 0) {
    if (producers.length === 0 || !world) return true;
    const queued = !!ev.shiftKey;
    if (commandTarget === "move" || commandTarget === "attack") {
      if (resource) return true;
      const kind = commandTarget === "attack" ? ORDER_STAGE.ATTACK_MOVE : ORDER_STAGE.MOVE;
      for (const building of producers) {
        this.commandInteraction.issueCommand(cmd.setRally(building, world.x, world.y, queued, kind));
      }
      this._addCommandFeedback(kind === ORDER_STAGE.ATTACK_MOVE ? "attack" : "move", world.x, world.y, queued);
    }
    return true;
  }
  if (commandTarget === "setupAntiTankGuns") {
    if (!world) return false;
    const supportWeapons = selectedOwnSupportWeaponEntities(this);
    if (supportWeapons.length > 0) {
      const queued = !!ev.shiftKey;
      this.commandInteraction.issueCommand(cmd.setupAntiTankGuns(
        supportWeapons.map((e) => e.id),
        world.x,
        world.y,
        queued,
      ));
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
          .filter((e) => ownOwner(this.state, e.owner, this.controlPolicy) && carriers.includes(e.kind))
          .map((e) => e.id)
      : ownUnits;
    if (units.length === 0) return true;
    const queued = !!ev.shiftKey;
    const selectedCarriers = this.state.selectedEntities().filter((e) => units.includes(e.id));
    const firstFireClick = ability === ABILITY.POINT_FIRE && !intent.artilleryFireCenter;
    if (firstFireClick) {
      const locks = buildArtilleryTargetLocks({
        ability,
        carriers: selectedCarriers.map((e) => plannedEntityForIntent(intent, e)),
        rawX: world.x,
        rawY: world.y,
        map: this.state.map,
        tileSize: this.state.map?.tileSize || DEFAULT_TILE_SIZE,
        definition,
        queued,
      });
      if (locks.length > 0) intent.beginArtilleryFireRadiusSelection?.(world.x, world.y);
      return false;
    }
    const fireCenter = ability === ABILITY.POINT_FIRE ? intent.artilleryFireCenter : null;
    const fireRadiusTiles = fireCenter
      ? artilleryFireRadiusTiles(
          fireCenter,
          world,
          this.state.map?.tileSize || DEFAULT_TILE_SIZE,
          artilleryMinFireRadiusTiles(commandUpgrades(this.state, this.controlPolicy)),
        )
      : null;
    const resolvedAbility = ability === ABILITY.POINT_FIRE && fireRadiusTiles != null
      ? ABILITY.BLANKET_FIRE
      : ability;
    const targetWorld = fireCenter || world;
    const command = fireCenter
      ? cmd.blanketFire(units, targetWorld.x, targetWorld.y, fireRadiusTiles, queued)
      : resolvedAbility === ABILITY.POINT_FIRE
        ? cmd.pointFire(units, targetWorld.x, targetWorld.y, queued)
        : cmd.useAbility(resolvedAbility, units, targetWorld.x, targetWorld.y, queued);
    const radiusTiles = fireRadiusTiles != null
      ? fireRadiusTiles
      : abilityTargetRadiusTiles(definition, ability, this.state, this.controlPolicy);
    const artilleryLocks = isArtilleryFireAbility(resolvedAbility)
      ? buildArtilleryTargetLocks({
        ability: resolvedAbility,
        carriers: selectedCarriers.map((e) => plannedEntityForIntent(intent, e)),
        rawX: targetWorld.x,
        rawY: targetWorld.y,
        map: this.state.map,
        tileSize: this.state.map?.tileSize || DEFAULT_TILE_SIZE,
        definition,
        queued,
      })
      : [];
    this.commandInteraction.issueCommand(command);
    if (fireCenter) intent.endArtilleryFireRadiusSelection?.();
    if (isArtilleryFireAbility(resolvedAbility)) {
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
    this.commandInteraction.issueCommand(cmd.move(ownUnits, world.x, world.y, !!ev.shiftKey));
    this._addCommandFeedback("move", world.x, world.y, !!ev.shiftKey);
    return true;
  }

  const target = pickedTarget;
  if (explicitAttackCommandTarget(this.state, target, this.controlPolicy)) {
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
    if (resource) return null;
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
  const contextualResource = resource || pumpJackOilUnderFriendlyUnit(input, target, workers);
  if (contextualResource && contextualResource.remaining !== 0) {
    const action = resourceRightClickAction(
      contextualResource,
      world || contextualResource,
      gatherers,
      workers,
      input.state.map,
    );
    if (action) return action;
  }

  if (target && ownOwner(input.state, target.owner, input.controlPolicy) && _isOwnIncompleteBuilding(target)) {
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
  if (target && enemyOwner(input.state, target.owner, input.controlPolicy) && isAttackableEntityTarget(target)) {
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
  const pumpJack = pumpJackBuildIntentForResource(resource, map);
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

function pumpJackOilUnderFriendlyUnit(input, target, workers) {
  if (
    workers.length === 0 ||
    !target ||
    !isUnit(target.kind) ||
    !friendlyOwner(input.state, target.owner, input.controlPolicy)
  ) {
    return null;
  }
  const map = input.state?.map;
  const tileSize = map?.tileSize || DEFAULT_TILE_SIZE;
  const stat = STATS[KIND.PUMP_JACK];
  if (!stat?.footW || !stat?.footH) return null;

  const matches = [];
  for (const candidate of input._selectionEntities?.() || []) {
    if (candidate.kind !== KIND.OIL || candidate.remaining === 0) continue;
    const intent = pumpJackBuildIntentForResource(candidate, map);
    if (!intent) continue;
    const minX = intent.tileX * tileSize;
    const minY = intent.tileY * tileSize;
    const maxX = minX + stat.footW * tileSize;
    const maxY = minY + stat.footH * tileSize;
    if (!entityIntersectsRect(target, minX, minY, maxX, maxY, tileSize)) continue;
    matches.push(candidate);
  }
  matches.sort((a, b) => {
    const ax = a.x - target.x;
    const ay = a.y - target.y;
    const bx = b.x - target.x;
    const by = b.y - target.y;
    return ax * ax + ay * ay - (bx * bx + by * by) || a.id - b.id;
  });
  return matches[0] || null;
}

function issueNormalRightClickAction(input, action, queued) {
  if (!action) return;
  switch (action.kind) {
    case "setRally":
      for (const building of action.producers) {
        input.commandInteraction.issueCommand(cmd.setRally(building, action.x, action.y, queued, action.stage));
      }
      break;
    case "build":
      input.commandInteraction.issueCommand(cmd.build(action.units, action.building, action.tileX, action.tileY, queued));
      break;
    case "gather":
      input.commandInteraction.issueCommand(cmd.gather(action.units, action.target.id, queued));
      break;
    case "deconstruct":
      input.commandInteraction.issueCommand(cmd.deconstruct(action.units, action.target.id, queued));
      break;
    case "attack":
      if (action.units.length > 0) {
        input.commandInteraction.issueCommand(cmd.attack(action.units, action.target.id, queued));
      }
      break;
    case "move":
      input.commandInteraction.issueCommand(cmd.move(action.units, action.x, action.y, queued));
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
    input.commandInteraction.issueCommand(cmd.attack(landUnits, target.id, queued));
    return true;
  }
  return false;
}

function issueAttackMoveForLandUnits(input, landUnits, x, y, queued) {
  if (landUnits.length > 0) {
    input.commandInteraction.issueCommand(cmd.attackMove(landUnits, x, y, queued));
    return true;
  }
  return false;
}

function rightClickFeedback(kind, x, y) {
  return { kind, x, y };
}

function explicitAttackCommandTarget(state, target, controlPolicy = null) {
  return !!target &&
    isAttackableEntityTarget(target) &&
    (ownOwner(state, target.owner, controlPolicy) || enemyOwner(state, target.owner, controlPolicy));
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
    .filter((e) => ownOwner(this.state, e.owner, this.controlPolicy) && isUnit(e.kind) && e.kind !== KIND.SCOUT_PLANE)
    .map((e) => e.id);
}

export function _selectedOwnLandUnitIds() {
  return selectedOwnLandUnitIds(this, this._selectedOwnUnitIds?.() || []);
}

export function _selectedProducerBuildingIds() {
  return selectedEntities(this.state)
    .filter((e) => ownOwner(this.state, e.owner, this.controlPolicy) && isBuilding(e.kind) && isProducerBuilding(e.kind))
    .map((e) => e.id);
}

export function _selectedWorkerIds() {
  return selectedEntities(this.state)
    .filter((e) => ownOwner(this.state, e.owner, this.controlPolicy) && e.kind === KIND.WORKER)
    .map((e) => e.id);
}

export function _selectedGathererIds() {
  return selectedEntities(this.state)
    .filter((e) =>
      ownOwner(this.state, e.owner, this.controlPolicy) &&
      (e.kind === KIND.WORKER || e.kind === KIND.GOLEM))
    .map((e) => e.id);
}

export function _selectedOwnAntiTankGunIds() {
  return selectedOwnSupportWeaponEntities(this).map((e) => e.id);
}

function selectedOwnSupportWeaponEntities(input) {
  return selectedEntities(input.state)
    .filter((e) => ownOwner(input.state, e.owner, input.controlPolicy) && (
      e.kind === KIND.ANTI_TANK_GUN ||
      e.kind === KIND.MORTAR_TEAM ||
      e.kind === KIND.ARTILLERY));
}

function selectedEntities(state) {
  return typeof state?.selectedEntities === "function" ? state.selectedEntities() || [] : [];
}

function selectedOwnUnitEntities(input) {
  return selectedEntities(input.state).filter((e) =>
    ownOwner(input.state, e.owner, input.controlPolicy) && isUnit(e.kind) && e.kind !== KIND.SCOUT_PLANE);
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

export function _refreshAbilityTargetPreview() {
  const intent = clientIntent(this);
  const target = intent?.commandTarget;
  if (!target || target.kind !== "ability" || !this.mouse) {
    intent?.updateAbilityTargetPreview?.(null);
    return;
  }
  const definition = ABILITIES[target.ability];
  const scoutPlaneTravelRangePx = target.ability === ABILITY.SCOUT_PLANE
    ? SCOUT_PLANE_SPEED_PX_PER_TICK * (definition?.durationTicks || 0)
    : 0;
  if (
    !definition ||
    !Array.isArray(definition.carriers) ||
    (!definition.rangeTiles && !(scoutPlaneTravelRangePx > 0))
  ) {
    intent?.updateAbilityTargetPreview?.(null);
    return;
  }
  const carriers = this.state
    .selectedEntities()
    .filter((e) => ownOwner(this.state, e.owner, this.controlPolicy) && definition.carriers.includes(e.kind))
    .map((e) => plannedEntityForIntent(intent, e));
  if (carriers.length === 0) {
    intent?.updateAbilityTargetPreview?.(null);
    return;
  }
  const tileSize = this.state.map?.tileSize || DEFAULT_TILE_SIZE;
  const rangePx = scoutPlaneTravelRangePx || definition.rangeTiles * tileSize;
  const minRangePx = (definition.minRangeTiles || 0) * tileSize;
  const locksRangeBand = isArtilleryFireAbility(target.ability);
  // Cursor feedback is rendered after this frame's camera update. Map its
  // hover target through that same current projection so it does not trail a
  // panning camera. Ground-command clicks still use SelectionScene geometry.
  const world = cursorPreviewGroundAtScreen(this, this.mouse);
  if (!world) {
    intent?.updateAbilityTargetPreview?.(null);
    return;
  }
  let hoverInRange = false;
  let hoverInsideMinRange = false;
  let artilleryLocks = [];
  if (locksRangeBand) {
    const targetWorld = target.ability === ABILITY.POINT_FIRE && intent.artilleryFireCenter
      ? intent.artilleryFireCenter
      : world;
    artilleryLocks = buildArtilleryTargetLocks({
      ability: target.ability,
      carriers,
      rawX: targetWorld.x,
      rawY: targetWorld.y,
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
      ownOwner(this.state, object.owner, this.controlPolicy) &&
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
        ownOwner(this.state, object.owner, this.controlPolicy) &&
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
  const selectingArtilleryRadius = target.ability === ABILITY.POINT_FIRE && !!intent.artilleryFireCenter;
  const radiusTiles = selectingArtilleryRadius
    ? artilleryFireRadiusTiles(
        intent.artilleryFireCenter,
        world,
        tileSize,
        artilleryMinFireRadiusTiles(commandUpgrades(this.state, this.controlPolicy)),
      )
    : abilityTargetRadiusTiles(definition, target.ability, this.state, this.controlPolicy);
  intent?.updateAbilityTargetPreview?.({
    ability: target.ability,
    mouseX: primaryLock?.x ?? world.x,
    mouseY: primaryLock?.y ?? world.y,
    rawMouseX: world.x,
    rawMouseY: world.y,
    radiusCursorX: selectingArtilleryRadius ? world.x : null,
    radiusCursorY: selectingArtilleryRadius ? world.y : null,
    artilleryRadiusSelection: selectingArtilleryRadius,
    carriers,
    artilleryLocks,
    rangeOrigins: carrierOrigins,
    pathOrigins: target.ability === ABILITY.EKAT_LINE_SHOT
      || target.ability === ABILITY.SCOUT_PLANE
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
    .filter((e) => ownOwner(this.state, e.owner, this.controlPolicy) && (
      e.kind === KIND.ANTI_TANK_GUN ||
      e.kind === KIND.MORTAR_TEAM ||
      e.kind === KIND.ARTILLERY))
    .map((e) => supportWeaponSetupPreviewEntity(plannedEntityForIntent(intent, e), setupPreviewQueued(this, intent)));
  if (guns.length === 0) {
    intent?.updateAntiTankGunSetupPreview?.(null);
    return;
  }
  // The cursor cone is rendered after this frame's camera update.  Map its
  // target through that same current projection instead of the prior presented
  // SelectionScene, otherwise it visibly trails the cursor while panning.
  const world = cursorPreviewGroundAtScreen(this, this.mouse);
  if (!world) {
    intent?.updateAntiTankGunSetupPreview?.(null);
    return;
  }
  const previewGuns = supportWeaponsWithSetupTargets(
    guns,
    world,
    this.state.map?.tileSize || DEFAULT_TILE_SIZE,
  );
  intent?.updateAntiTankGunSetupPreview?.({
    mouseX: world.x,
    mouseY: world.y,
    guns: previewGuns,
  });
}

function cursorPreviewGroundAtScreen(input, screen) {
  let projection;
  try {
    projection = input?.camera?.projectionSnapshot?.();
  } catch {
    return input._groundAtScreen(screen.x, screen.y);
  }
  const groundAtScreen = projection?.groundAtScreen;
  if (typeof groundAtScreen === "function") {
    try {
      const point = groundAtScreen({ x: screen.x, y: screen.y });
      // A valid current projection returning no hit must clear the preview,
      // rather than resurrecting a position from a stale SelectionScene.
      if (point == null) return null;
      const clamped = clampCursorPreviewPoint(point, input?.state?.map);
      if (clamped) return clamped;
    } catch {
      // A malformed fresh projection must not break the existing input path.
    }
  }
  return input._groundAtScreen(screen.x, screen.y);
}

function clampCursorPreviewPoint(point, map) {
  if (!Number.isFinite(point?.x) || !Number.isFinite(point?.y)) return null;
  const mapWidthPx = Number(map?.width) * Number(map?.tileSize);
  const mapHeightPx = Number(map?.height) * Number(map?.tileSize);
  if (!(mapWidthPx > 0) || !(mapHeightPx > 0)) return { x: point.x, y: point.y };
  return {
    x: Math.max(0, Math.min(mapWidthPx - 1, point.x)),
    y: Math.max(0, Math.min(mapHeightPx - 1, point.y)),
  };
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

  const nearest = this._nearestCompletedMiningAnchor(
    target.x,
    target.y,
    target.kind === KIND.OIL,
  );
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

export function _nearestCompletedMiningAnchor(x, y, includeAllies = false) {
  let best = null;
  for (const e of this._selectionEntities()) {
    if (
      !(includeAllies
        ? friendlyOwner(this.state, e.owner, this.controlPolicy)
        : ownOwner(this.state, e.owner, this.controlPolicy)) ||
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

function ownOwner(state, owner, controlPolicy = null) {
  if (controlPolicy?.kind === "lab") {
    if (typeof controlPolicy.isCommandOwner === "function") {
      return controlPolicy.isCommandOwner(owner, state);
    }
    return controlPolicy.canControlOwner(owner, state);
  }
  return typeof state?.isOwnOwner === "function"
    ? state.isOwnOwner(owner)
    : Number(owner) === state?.playerId;
}

function enemyOwner(state, owner, controlPolicy = null) {
  if (controlPolicy?.kind === "lab") {
    if (typeof controlPolicy.isCommandEnemyOwner === "function") {
      return controlPolicy.isCommandEnemyOwner(owner, state);
    }
    const commandOwner = typeof controlPolicy.commandOwner === "function"
      ? controlPolicy.commandOwner(state)
      : controlPolicy.issueAsOwnerForSelection?.(state.selectedEntities?.() || []);
    return fallbackEnemyOwner(commandOwner, owner);
  }
  if (typeof state?.isEnemyOwner === "function") return state.isEnemyOwner(owner);
  return fallbackEnemyOwner(state?.playerId, owner);
}

function friendlyOwner(state, owner, controlPolicy = null) {
  if (controlPolicy?.kind === "lab") {
    return ownOwner(state, owner, controlPolicy) || !!controlPolicy.isCommandAllyOwner?.(owner, state);
  }
  return ownOwner(state, owner, controlPolicy) || (
    typeof state?.isAllyOwner === "function" && state.isAllyOwner(owner)
  );
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

function abilityTargetRadiusTiles(definition, ability, state, controlPolicy = null) {
  const baseRadius = definition?.radiusTiles || 0;
  if (ability === ABILITY.SMOKE && commandUpgrades(state, controlPolicy).includes(UPGRADE.SMOKE_PLUS)) {
    return definition?.upgradedRadiusTiles || baseRadius;
  }
  return baseRadius;
}

function commandUpgrades(state, controlPolicy = null) {
  if (typeof controlPolicy?.commandUpgrades === "function") {
    const upgrades = controlPolicy.commandUpgrades(state);
    return Array.isArray(upgrades) ? upgrades : [];
  }
  return Array.isArray(state?.upgrades) ? state.upgrades : [];
}

export function _activateCommandHotkey(ev) {
  const code = commandHotkeyCodeFromEvent(ev);
  if (!code) return false;
  const card = document.getElementById("command-card");
  if (!card) return false;
  for (const btn of card.querySelectorAll("button[data-hotkey-code]")) {
    if (btn.dataset.hotkeyCode !== code) continue;
    if (ev.repeat && btn.dataset.repeatable !== "true") return false;
    ev.preventDefault();
    const contextAction = commandContextActionRequested(btn, ev);
    if (contextAction) {
      dispatchCommandButtonMouseEvent(btn, "contextmenu", ev);
    } else if (!btn.disabled) {
      // Alt-click is a pointer-only secondary affordance. A hotkey whose descriptor does not
      // declare Alt must reach the primary click handler instead of imitating an Alt-click.
      dispatchCommandButtonMouseEvent(btn, "click", ev, false);
    }
    return {
      handled: true,
      commandId: btn.dataset.commandId || null,
      hotkey: btn.dataset.hotkey || null,
      hotkeyCode: btn.dataset.hotkeyCode || null,
      slotIndex: btn.dataset.slotIndex != null ? Number(btn.dataset.slotIndex) : null,
      contextAction,
      armed: clientIntent(this)?.lastCommandTargetArm || null,
    };
  }
  return false;
}

function commandContextActionRequested(btn, ev) {
  if (btn.dataset.contextAction !== "true") return false;
  const modifiers = new Set((btn.dataset.contextHotkeyModifiers || "").split(/\s+/).filter(Boolean));
  return (ev.altKey && modifiers.has("alt")) ||
    (ev.ctrlKey && modifiers.has("ctrl")) ||
    (ev.metaKey && modifiers.has("meta")) ||
    (ev.shiftKey && modifiers.has("shift"));
}

function dispatchCommandButtonMouseEvent(btn, type, ev, altKey = !!ev.altKey) {
  if (typeof MouseEvent === "function" && typeof btn.dispatchEvent === "function") {
    btn.dispatchEvent(new MouseEvent(type, {
      bubbles: true,
      cancelable: true,
      altKey,
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
      altKey,
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
  if (this._issueTargetedCommand(this.mouse, ev) === false) return true;
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
  if (this._cancelFormationGesture?.()) return;
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

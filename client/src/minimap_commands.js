import { ABILITY, KIND, ORDER_STAGE, cmd, isUnit } from "./protocol.js";
import { ABILITIES, abilityCarriedByEntity, isProducerBuilding } from "./config.js";
import { artilleryFireRadiusTiles, artilleryMinFireRadiusTiles,
  buildArtilleryTargetLocks, isArtilleryFireAbility } from "./input/artillery_targeting.js";
import { vectorFieldTargetCommand } from "./input/two_stage_target.js";
import { abilityTargetRadiusTiles, commandUpgrades, ownOwner,
  plannedEntityForIntent, resourceRallyTargetAt } from "./minimap_targeting.js";

export function issueMinimapOrder(wx, wy, queued = false) {
  const commandTarget = this._intent()?.commandTarget;
  if (ABILITIES[commandTarget?.ability]?.targetMode === "friendlyUnit") return false;
  const sel = this.state.selectedEntities() || [];
  if (commandTarget === "pointTanks") {
    const tanks = sel.filter((e) => ownOwner(this.state, e.owner, this.controlPolicy) && e.kind === KIND.TANK);
    if (tanks.length > 0) {
      this.commandInteraction.issueCommand(cmd.pointTanks(tanks.map((e) => e.id), wx, wy, queued));
      this._addCommandFeedback("move", wx, wy, queued);
    }
    return;
  }
  if (commandTarget === "setupAntiTankGuns") {
    const supportWeaponEntities = this._selectedOwnSupportWeapons();
    const supportWeapons = supportWeaponEntities.map((e) => e.id);
    if (supportWeapons.length > 0) {
      this.commandInteraction.issueCommand(cmd.setupAntiTankGuns(
        supportWeapons,
        wx,
        wy,
        queued,
      ));
      this._addCommandFeedback("move", wx, wy, queued);
    }
    return;
  }
  const landUnitIds = [];
  for (const e of sel) {
    // Only own, controllable units take move orders (skip buildings/resources/enemies).
    if (ownOwner(this.state, e.owner, this.controlPolicy) && isUnit(e.kind) && e.kind !== KIND.SCOUT_PLANE) {
      landUnitIds.push(e.id);
    }
  }
  if (landUnitIds.length === 0) {
    const producers = sel
      .filter((e) => ownOwner(this.state, e.owner, this.controlPolicy) && isProducerBuilding(e.kind))
      .map((e) => e.id);
    if (producers.length === 0) return;
    const resource = resourceRallyTargetAt(this.state.map, wx, wy);
    if (resource) return;
    const kind = commandTarget === "attack" ? ORDER_STAGE.ATTACK_MOVE : ORDER_STAGE.MOVE;
    for (const building of producers) {
      this.commandInteraction.issueCommand(cmd.setRally(building, wx, wy, queued, kind));
    }
    this._addCommandFeedback(kind === ORDER_STAGE.ATTACK_MOVE ? "attack" : "move", wx, wy, queued);
    return;
  }
  if (commandTarget === "attack") {
    if (landUnitIds.length > 0) {
      this.commandInteraction.issueCommand(cmd.attackMove(landUnitIds, wx, wy, queued));
    }
    if (landUnitIds.length > 0) {
      this._addCommandFeedback("attack", wx, wy, queued);
    }
    return;
  }
  if (commandTarget?.kind === "ability") {
    const ability = commandTarget.ability;
    const definition = ABILITIES[ability];
    const carriers = definition?.carriers;
    const abilityUnits = Array.isArray(carriers)
      ? sel
          .filter((e) => ownOwner(this.state, e.owner, this.controlPolicy) && abilityCarriedByEntity(definition, e))
          .map((e) => e.id)
      : landUnitIds;
    if (abilityUnits.length === 0) return;
    const selectedCarriers = sel
      .filter((e) => abilityUnits.includes(e.id))
      .map((e) => plannedEntityForIntent(this._intent(), e));
    const intent = this._intent();
    if (ability === ABILITY.VECTOR_FIELD) {
      const command = vectorFieldTargetCommand(intent, abilityUnits, { x: wx, y: wy });
      if (!command) return false;
      this.commandInteraction.issueCommand(command);
      return;
    }
    const firstFireClick = ability === ABILITY.POINT_FIRE && !intent?.artilleryFireCenter;
    if (firstFireClick) {
      const locks = buildArtilleryTargetLocks({
        ability,
        carriers: selectedCarriers,
        rawX: wx,
        rawY: wy,
        map: this.state.map,
        tileSize: this.state.map?.tileSize,
        definition,
        queued,
      });
      if (locks.length > 0) intent?.beginArtilleryFireRadiusSelection?.(wx, wy);
      return false;
    }
    const fireCenter = ability === ABILITY.POINT_FIRE ? intent?.artilleryFireCenter : null;
    const fireRadiusTiles = fireCenter
      ? artilleryFireRadiusTiles(
          fireCenter,
          { x: wx, y: wy },
          this.state.map?.tileSize,
          artilleryMinFireRadiusTiles(commandUpgrades(this.state, this.controlPolicy)),
        )
      : null;
    const resolvedAbility = fireCenter ? ABILITY.BLANKET_FIRE : ability;
    const target = fireCenter || { x: wx, y: wy };
    const artilleryLocks = isArtilleryFireAbility(resolvedAbility)
      ? buildArtilleryTargetLocks({
        ability: resolvedAbility,
        carriers: selectedCarriers,
        rawX: target.x,
        rawY: target.y,
        map: this.state.map,
        tileSize: this.state.map?.tileSize,
        definition,
        queued,
      })
      : [];
    const radiusTiles = fireRadiusTiles ?? abilityTargetRadiusTiles(
      definition,
      ability,
      this.state,
      this.controlPolicy,
    );
    const command = fireCenter
      ? cmd.blanketFire(abilityUnits, target.x, target.y, fireRadiusTiles, queued)
      : cmd.useAbility(resolvedAbility, abilityUnits, target.x, target.y, queued);
    this.commandInteraction.issueCommand(command);
    if (fireCenter) intent.endArtilleryFireRadiusSelection?.();
    if (isArtilleryFireAbility(resolvedAbility)) {
      for (const lock of artilleryLocks) {
        this._addCommandFeedback("artillery", lock.x, lock.y, queued, radiusTiles);
      }
      return;
    }
    this._addCommandFeedback("attack", wx, wy, queued, radiusTiles);
    return;
  }
  this.commandInteraction.issueCommand(cmd.move(landUnitIds, wx, wy, queued));
  this._addCommandFeedback("move", wx, wy, queued);
}

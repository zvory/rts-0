import { CommandComposer } from "./command_composer.js";
import { ABILITY, CMD, KIND, ORDER_STAGE } from "./protocol.js";

const ARTILLERY_TERMINAL_STAGES = new Set([
  ORDER_STAGE.POINT_FIRE,
  ORDER_STAGE.BLANKET_FIRE,
]);
const QUEUE_TERMINAL_STAGES = new Set([
  ...ARTILLERY_TERMINAL_STAGES,
  ORDER_STAGE.HOLD_POSITION,
]);
const PLAN_XY_EPSILON = 0.5;

/**
 * Browser-local intent state shared by HUD, input, minimap, and renderer feedback.
 *
 * This helper owns transient client decisions that are not part of the
 * authoritative server snapshot.
 */
export class ClientIntent {
  constructor({ now = defaultNow } = {}) {
    this._now = now;
    /** @type {null | {building:string, tileX:number, tileY:number, valid:boolean, lineSites?:Array<{tileX:number,tileY:number,valid:boolean}>}} */
    this.placement = null;
    /** @type {null | "workerBuild"} */
    this.commandCardMode = null;
    /** @type {null | "move" | "attack" | "setupAntiTankGuns" | {kind:"ability",ability:string}} */
    this.commandTarget = null;
    this.commandComposer = new CommandComposer();
    /** @type {null | {id:string,kind:string,payload?:object,label?:string,keepArmedOnWorldClick?:boolean,paintOnDrag?:boolean,consumeBoxSelection?:boolean,keepArmedOnBoxSelection?:boolean}} */
    this.activeLabTool = null;
    /** @type {null | {toolId:string,kind:string,payload?:object,x:number,y:number}} */
    this.labToolPreview = null;
    this._nextLabToolId = 1;
    /** @type {null | {quickCast:boolean,target:string|object,queued:boolean}} */
    this.lastCommandTargetArm = null;
    /** @type {Array<{kind:string,x:number,y:number,append:boolean,radiusTiles:number|null,createdAt:number,ownerId:number|null}>} */
    this.commandFeedback = [];
    /** @type {null | {points:Array<{x:number,y:number}>,slots:Array<{unitId:number,x:number,y:number,radius:number}>}} */
    this.formationMovePreview = null;
    /** @type {null | {targetId:number, kind:string, x:number, y:number}} */
    this.attackTargetPreview = null;
    /** @type {null | {resourceId:number, resourceX:number, resourceY:number, ccId:number, ccX:number, ccY:number, inRange:boolean}} */
    this.resourceMiningPreview = null;
    /** @type {null | {source?:string, mouseX:number, mouseY:number, guns:Array<object>}} */
    this.antiTankGunSetupPreview = null;
    /** @type {null | {ability:string, source?:string, mouseX?:number, mouseY?:number, carriers:Array<object>, areaOrigins?:Array<object>, rangeOrigins?:Array<object>, pathOrigins?:Array<object>, returnMarkers?:Array<object>, rangePx?:number, hoverInRange:boolean, hoverInsideMinRange?:boolean}} */
    this.abilityTargetPreview = null;
    /** @type {Map<number, Array<{kind:string,x?:number,y?:number,clientSeq:number|null,createdAt:number,replacesAuthority?:boolean}>>} */
    this._plannedOrderStagesByUnit = new Map();
  }

  /** Open the worker build command-card submenu. */
  openWorkerBuildMenu() {
    this._clearActiveLabTool();
    this.placement = null;
    this.commandTarget = null;
    this.lastCommandTargetArm = null;
    this.antiTankGunSetupPreview = null;
    this.attackTargetPreview = null;
    this.commandCardMode = "workerBuild";
  }

  /**
   * Close any command-card submenu.
   * @returns {boolean} true if a submenu was open.
   */
  closeCommandCardMenu() {
    const hadMenu = this.commandCardMode != null;
    this.commandCardMode = null;
    return hadMenu;
  }

  /**
   * Start previewing placement of a building. Position/validity are filled in
   * by updatePlacement as the cursor moves.
   * @param {string} buildingKind a building EntityKind.
   */
  beginPlacement(buildingKind) {
    this._clearActiveLabTool();
    this.commandTarget = null;
    this.lastCommandTargetArm = null;
    this.attackTargetPreview = null;
    this.closeCommandCardMenu();
    this.placement = { building: buildingKind, tileX: 0, tileY: 0, valid: false };
  }

  /**
   * Update the placement preview's tile and validity. No-op if no placement
   * is in progress.
   * @param {number} tileX
   * @param {number} tileY
   * @param {boolean} valid
   */
  updatePlacement(tileX, tileY, valid, options = {}) {
    if (!this.placement) return;
    this.placement.tileX = tileX;
    this.placement.tileY = tileY;
    this.placement.valid = !!valid;
    if (Array.isArray(options.lineSites)) {
      this.placement.lineSites = options.lineSites;
    } else if ("lineSites" in this.placement) {
      delete this.placement.lineSites;
    }
  }

  /** Stop previewing placement. */
  endPlacement() {
    this.placement = null;
  }

  /**
   * Arm a one-click command target mode from the HUD.
   * @param {"move"|"attack"|"setupAntiTankGuns"|{kind:"ability",ability:string}} kind
   */
  beginCommandTarget(kind, options = {}) {
    this._clearActiveLabTool();
    this.placement = null;
    this.closeCommandCardMenu();
    const armed = this.commandComposer.arm(kind, options);
    this.lastCommandTargetArm = armed;
    this._syncCommandTargetFromComposer();
    return armed;
  }

  /** Clear any armed command target mode. */
  endCommandTarget() {
    this.commandComposer.cancel();
    this.lastCommandTargetArm = null;
    this._syncCommandTargetFromComposer();
  }

  /** Mark a physical key as holding the current command target alive. */
  holdCommandTarget(kind, key, shiftKey = false, options = {}) {
    this.commandComposer.hold(kind, key, { ...options, shiftKey });
    this._syncCommandTargetFromComposer();
  }

  /**
   * Record a click issue and return whether the target remains armed.
   * @param {{shiftKey?: boolean}} ev
   * @returns {{target:null|string|object,queued:boolean,keepArmed:boolean}}
   */
  issueCommandTarget(ev = {}) {
    const issued = this.commandComposer.issue(ev);
    this._syncCommandTargetFromComposer();
    return issued;
  }

  /** Release a physical command key. */
  releaseCommandTargetKey(key, shiftKey = false) {
    this.commandComposer.releaseKey(key, { shiftKey });
    this._syncCommandTargetFromComposer();
  }

  /** Release Shift preservation for a tapped command. */
  releaseCommandTargetShift() {
    this.commandComposer.releaseShift();
    this._syncCommandTargetFromComposer();
  }

  _syncCommandTargetFromComposer() {
    this.commandTarget = this.commandComposer.target;
    this.attackTargetPreview = null;
    this.antiTankGunSetupPreview = null;
    this.abilityTargetPreview = null;
  }

  /**
   * Add a short-lived local command marker at a world point.
   * @param {"move"|"attack"|string} kind
   * @param {number} x
   * @param {number} y
   * @param {boolean=} append
   * @param {number|null=} radiusTiles
   * @param {number=} now
   * @param {number|null=} ownerId
   */
  addCommandFeedback(kind, x, y, append = false, radiusTiles = null, now = this._now(), ownerId = null) {
    this.commandFeedback.push({
      kind,
      x,
      y,
      append: !!append,
      radiusTiles,
      createdAt: now,
      ownerId: normalizeOwnerId(ownerId),
    });
    if (this.commandFeedback.length > 12) {
      this.commandFeedback.splice(0, this.commandFeedback.length - 12);
    }
  }

  /**
   * Return live command feedback markers, pruning expired ones.
   * @param {number} now
   * @returns {Array<{kind:string,x:number,y:number,append:boolean,createdAt:number,ownerId:number|null}>}
   */
  liveCommandFeedback(now) {
    const ttlMs = 650;
    this.commandFeedback = this.commandFeedback.filter((f) => now - f.createdAt <= ttlMs);
    return this.commandFeedback;
  }

  /** Set or clear the live freehand formation line and provisional unit slots. */
  updateFormationMovePreview(preview) {
    if (!preview || !Array.isArray(preview.points) || preview.points.length < 2) {
      this.formationMovePreview = null;
      return null;
    }
    this.formationMovePreview = {
      points: preview.points.map((point) => ({ x: point.x, y: point.y })),
      slots: (Array.isArray(preview.slots) ? preview.slots : []).map((slot) => ({ ...slot })),
    };
    return this.formationMovePreview;
  }

  clearFormationMovePreview() {
    this.formationMovePreview = null;
  }

  /**
   * Set or clear the enemy unit/entity under the cursor that a normal right-click would attack.
   * @param {null | {targetId:number, kind:string, x:number, y:number}} preview
   */
  updateAttackTargetPreview(preview) {
    this.attackTargetPreview = preview;
  }

  /**
   * Set or clear the hovered resource-to-City-Centre mining preview.
   * @param {null | {resourceId:number, resourceX:number, resourceY:number, ccId:number, ccX:number, ccY:number, inRange:boolean}} preview
   */
  updateResourceMiningPreview(preview) {
    this.resourceMiningPreview = preview;
  }

  /**
   * Set or clear the anti-tank gun manual setup cone preview.
   * @param {null | {source?:string, mouseX:number, mouseY:number, guns:Array<object>}} preview
   */
  updateAntiTankGunSetupPreview(preview) {
    this.antiTankGunSetupPreview = preview;
  }

  /**
   * Set or clear the armed-ability targeting preview.
   * @param {null | {ability:string, source?:string, mouseX?:number, mouseY?:number, carriers:Array<object>, areaOrigins?:Array<object>, rangeOrigins?:Array<object>, pathOrigins?:Array<object>, returnMarkers?:Array<object>, rangePx?:number, hoverInRange:boolean, hoverInsideMinRange?:boolean}} preview
   */
  updateAbilityTargetPreview(preview) {
    this.abilityTargetPreview = preview;
  }

  /**
   * Record the local queued-order shape of a command that was accepted for send.
   * This is intentionally small and client-only: it exists to keep previews stable
   * until authoritative orderPlan snapshots confirm or replace the plan.
   * @param {object} command
   * @param {Array<object>} selectedEntities
   * @param {object|boolean|null} result
   */
  recordPlannedCommand(command, selectedEntities = [], result = null) {
    if (!command || typeof command !== "object") return;
    if (result === false || isPromiseLike(result)) return;
    if (result && typeof result === "object" && result.sent === false) return;
    const units = normalizeUnitIds(command.units);
    if (units.length === 0) return;
    const clientSeq = normalizeClientSeq(result?.clientSeq);
    const stage = commandOrderStage(command, clientSeq, this._now());
    if (!stage) {
      if (!command.queued || clearsPlannedStages(command.c)) {
        this.clearPlannedOrdersForUnits(units);
      }
      return;
    }

    const selectedById = new Map(
      (Array.isArray(selectedEntities) ? selectedEntities : [])
        .filter((entity) => Number.isInteger(entity?.id))
        .map((entity) => [entity.id, entity]),
    );
    for (const unitId of units) {
      const entity = selectedById.get(unitId) || null;
      if (command.queued) {
        this._appendPlannedStage(unitId, stage, entity);
      } else {
        this._plannedOrderStagesByUnit.set(unitId, [cloneStage(stage, { replacesAuthority: true })]);
      }
    }
  }

  /**
   * Return the authoritative orderPlan plus unconfirmed local stages for an entity.
   * @param {object} entity
   * @returns {Array<object>}
   */
  plannedOrderPlanForEntity(entity) {
    const authority = Array.isArray(entity?.orderPlan)
      ? entity.orderPlan.map((stage) => ({ ...stage }))
      : [];
    const local = this._plannedOrderStagesByUnit.get(entity?.id) || [];
    const base = local[0]?.replacesAuthority ? [] : authority;
    const merged = [];
    for (const stage of base.concat(local)) {
      merged.push(publicOrderStage(stage));
      if (stageIsTerminalForEntity(stage, entity)) break;
    }
    return merged;
  }

  /**
   * Return an entity clone whose orderPlan includes pending local stages.
   * @param {object} entity
   * @returns {object}
   */
  entityWithPlannedOrder(entity) {
    if (!entity || !this._plannedOrderStagesByUnit.has(entity.id)) return entity;
    return { ...entity, orderPlan: this.plannedOrderPlanForEntity(entity) };
  }

  /** Clear local planned stages for specific unit ids. */
  clearPlannedOrdersForUnits(unitIds) {
    for (const id of normalizeUnitIds(unitIds)) {
      this._plannedOrderStagesByUnit.delete(id);
    }
  }

  /** Clear local planned stages that came from a rejected client command. */
  clearPlannedOrdersForClientSeq(clientSeq) {
    const seq = normalizeClientSeq(clientSeq);
    if (seq == null) return;
    for (const [unitId, stages] of this._plannedOrderStagesByUnit.entries()) {
      const rejectedIndex = stages.findIndex((stage) => stage.clientSeq === seq);
      if (rejectedIndex < 0) continue;
      const kept = stages.slice(0, rejectedIndex);
      if (kept.length > 0) this._plannedOrderStagesByUnit.set(unitId, kept);
      else this._plannedOrderStagesByUnit.delete(unitId);
    }
  }

  /** Clear local planned stages for units no longer selected. */
  clearPlannedOrdersOutsideSelection(selectedIds) {
    const selected = new Set(normalizeUnitIds(selectedIds));
    for (const id of this._plannedOrderStagesByUnit.keys()) {
      if (!selected.has(id)) this._plannedOrderStagesByUnit.delete(id);
    }
  }

  /** Clear all local planned stages. */
  clearPlannedOrders() {
    this._plannedOrderStagesByUnit.clear();
  }

  /**
   * Reconcile local stages with selected authoritative entity views.
   * @param {Array<object>} entities
   * @param {{acknowledgedClientSeq?: number|null}} options
   */
  reconcilePlannedOrders(entities = [], options = {}) {
    const visibleSelected = new Map(
      (Array.isArray(entities) ? entities : [])
        .filter((entity) => Number.isInteger(entity?.id))
        .map((entity) => [entity.id, entity]),
    );
    const ackSeq = normalizeClientSeq(options.acknowledgedClientSeq);
    for (const [unitId, stages] of this._plannedOrderStagesByUnit.entries()) {
      const entity = visibleSelected.get(unitId);
      if (!entity) {
        this._plannedOrderStagesByUnit.delete(unitId);
        continue;
      }
      const authority = Array.isArray(entity.orderPlan) ? entity.orderPlan : [];
      const pending = [];
      let stale = false;
      for (const stage of stages) {
        if (stage.clientSeq != null && ackSeq != null && stage.clientSeq <= ackSeq) {
          if (!stageConfirmedByAuthority(stage, authority)) stale = true;
          continue;
        }
        if (stale) continue;
        pending.push(stage);
      }
      if (pending.length > 0) this._plannedOrderStagesByUnit.set(unitId, pending);
      else this._plannedOrderStagesByUnit.delete(unitId);
    }
  }

  _appendPlannedStage(unitId, stage, entity = null) {
    const current = this._plannedOrderStagesByUnit.get(unitId) || [];
    const authorityPlan = current[0]?.replacesAuthority
      ? []
      : Array.isArray(entity?.orderPlan)
        ? entity.orderPlan
        : [];
    if (planHasTerminal(authorityPlan, entity)) return;
    if (planHasTerminal(current, entity)) return;
    const next = replaceContradictoryLocalStages(current, stage);
    next.push(queuedStageForEntity(stage, authorityPlan.concat(next), entity));
    this._plannedOrderStagesByUnit.set(unitId, next);
  }

  /**
   * Arm a lab setup tool for world clicks.
   * @param {{kind:string,payload?:object,label?:string,id?:string,keepArmedOnWorldClick?:boolean,paintOnDrag?:boolean,consumeBoxSelection?:boolean,keepArmedOnBoxSelection?:boolean}} tool
   */
  beginLabTool(tool) {
    const kind = typeof tool?.kind === "string" && tool.kind ? tool.kind : "unknown";
    const id = typeof tool?.id === "string" && tool.id
      ? tool.id
      : `lab-tool-${this._nextLabToolId++}`;
    this.placement = null;
    this.commandComposer.cancel();
    this.lastCommandTargetArm = null;
    this._syncCommandTargetFromComposer();
    this.commandCardMode = null;
    this.attackTargetPreview = null;
    this.resourceMiningPreview = null;
    const active = { id, kind };
    if (tool?.payload && typeof tool.payload === "object") active.payload = { ...tool.payload };
    if (typeof tool?.label === "string" && tool.label) active.label = tool.label;
    if (tool?.keepArmedOnWorldClick) active.keepArmedOnWorldClick = true;
    if (tool?.paintOnDrag) active.paintOnDrag = true;
    if (tool?.consumeBoxSelection) active.consumeBoxSelection = true;
    if (tool?.keepArmedOnBoxSelection) active.keepArmedOnBoxSelection = true;
    this.activeLabTool = active;
    this.labToolPreview = null;
    return active;
  }

  /** Replace the active Lab tool payload without interrupting its current interaction. */
  updateLabToolPayload(payload) {
    const active = this.activeLabTool;
    if (!active || !payload || typeof payload !== "object") return null;
    active.payload = { ...payload };
    if (this.labToolPreview?.toolId === active.id) {
      this.labToolPreview = {
        ...this.labToolPreview,
        payload: { ...active.payload },
      };
    }
    return active;
  }

  /** Update the renderer-facing cursor ghost for the active lab tool. */
  updateLabToolPreview(preview) {
    const tool = this.activeLabTool;
    if (!tool || preview?.toolId !== tool.id || !Number.isFinite(preview?.x) || !Number.isFinite(preview?.y)) {
      this.labToolPreview = null;
      return null;
    }
    const next = {
      toolId: tool.id,
      kind: tool.kind,
      x: preview.x,
      y: preview.y,
    };
    if (tool.payload) next.payload = { ...tool.payload };
    this.labToolPreview = next;
    return next;
  }


  /** Clear the active lab setup tool, if any. */
  cancelLabTool(reason = "cancelled") {
    const active = this.activeLabTool;
    this.activeLabTool = null;
    this.labToolPreview = null;
    return active ? { ...active, reason } : null;
  }

  _clearActiveLabTool() {
    this.activeLabTool = null;
    this.labToolPreview = null;
  }
}

function commandOrderStage(command, clientSeq, createdAt) {
  const base = { clientSeq, createdAt };
  switch (command.c) {
    case CMD.MOVE:
      return finitePointStage(ORDER_STAGE.MOVE, command, base);
    case CMD.FORMATION_MOVE:
      return finiteFormationStage(command, base);
    case CMD.ATTACK_MOVE:
      return finitePointStage(ORDER_STAGE.ATTACK_MOVE, command, base);
    case CMD.SETUP_ANTI_TANK_GUNS:
      return finitePointStage(ORDER_STAGE.SETUP_ANTI_TANK_GUNS, command, base);
    case CMD.HOLD_POSITION:
      return command.queued ? { kind: ORDER_STAGE.HOLD_POSITION, ...base } : null;
    case CMD.USE_ABILITY:
      if (command.ability === ABILITY.POINT_FIRE) {
        return finitePointStage(ORDER_STAGE.POINT_FIRE, command, base);
      }
      if (command.ability === ABILITY.BLANKET_FIRE) {
        return finitePointStage(ORDER_STAGE.BLANKET_FIRE, command, base);
      }
      return null;
    default:
      return null;
  }
}

function finiteFormationStage(command, base) {
  const points = Array.isArray(command.points) ? command.points : [];
  const point = points[points.length - 1];
  return finitePointStage(ORDER_STAGE.MOVE, point || {}, base);
}

function finitePointStage(kind, command, base) {
  if (!Number.isFinite(command.x) || !Number.isFinite(command.y)) return null;
  return { kind, x: command.x, y: command.y, ...base };
}

function clearsPlannedStages(commandKind) {
  return commandKind === CMD.STOP ||
    commandKind === CMD.ATTACK ||
    commandKind === CMD.GATHER ||
    commandKind === CMD.BUILD ||
    commandKind === CMD.DECONSTRUCT ||
    commandKind === CMD.TEAR_DOWN_ANTI_TANK_GUNS ||
    commandKind === CMD.RECAST_ABILITY ||
    commandKind === CMD.CHARGE;
}

function replaceContradictoryLocalStages(stages, nextStage) {
  const out = stages.map(cloneStage);
  if (nextStage.kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS) {
    const index = out.findIndex((stage) =>
      stage.kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS || ARTILLERY_TERMINAL_STAGES.has(stage.kind));
    return index >= 0 ? out.slice(0, index) : out;
  }
  if (ARTILLERY_TERMINAL_STAGES.has(nextStage.kind)) {
    const index = out.findIndex((stage) => ARTILLERY_TERMINAL_STAGES.has(stage.kind));
    return index >= 0 ? out.slice(0, index) : out;
  }
  return out;
}

function planHasTerminal(plan, entity = null) {
  return Array.isArray(plan) && plan.some((stage) => stageIsTerminalForEntity(stage, entity));
}

function stageIsTerminalForEntity(stage, entity = null) {
  return QUEUE_TERMINAL_STAGES.has(stage?.kind) ||
    (entity?.kind === KIND.MORTAR_TEAM &&
      stage?.kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS &&
      !stage?.replacesAuthority);
}

function queuedStageForEntity(stage, precedingStages, entity = null) {
  if (
    entity?.kind !== KIND.MORTAR_TEAM ||
    stage?.kind !== ORDER_STAGE.SETUP_ANTI_TANK_GUNS
  ) {
    return cloneStage(stage);
  }
  const precedingPoint = [...precedingStages]
    .reverse()
    .find((candidate) => Number.isFinite(candidate?.x) && Number.isFinite(candidate?.y));
  const x = Number.isFinite(precedingPoint?.x) ? precedingPoint.x : entity?.x;
  const y = Number.isFinite(precedingPoint?.y) ? precedingPoint.y : entity?.y;
  return Number.isFinite(x) && Number.isFinite(y)
    ? cloneStage(stage, { x, y })
    : cloneStage(stage);
}

function stageConfirmedByAuthority(stage, authorityPlan) {
  if (!Array.isArray(authorityPlan)) return false;
  return authorityPlan.some((authority) => {
    if (authority?.kind !== stage.kind) return false;
    if (Number.isFinite(stage.x) && Number.isFinite(stage.y)) {
      return closePoint(authority, stage);
    }
    return true;
  });
}

function closePoint(a, b) {
  return Number.isFinite(a?.x) &&
    Number.isFinite(a?.y) &&
    Number.isFinite(b?.x) &&
    Number.isFinite(b?.y) &&
    Math.abs(a.x - b.x) <= PLAN_XY_EPSILON &&
    Math.abs(a.y - b.y) <= PLAN_XY_EPSILON;
}

function cloneStage(stage, extra = null) {
  return extra ? { ...stage, ...extra } : { ...stage };
}

function publicOrderStage(stage) {
  const out = { kind: stage.kind };
  if (Number.isFinite(stage.x)) out.x = stage.x;
  if (Number.isFinite(stage.y)) out.y = stage.y;
  return out;
}

function normalizeUnitIds(units) {
  if (
    !Array.isArray(units) &&
    (typeof units === "string" || !units || typeof units[Symbol.iterator] !== "function")
  ) {
    return [];
  }
  const out = [];
  const seen = new Set();
  for (const unit of units) {
    const id = Number(unit);
    if (!Number.isInteger(id) || id <= 0 || seen.has(id)) continue;
    seen.add(id);
    out.push(id);
  }
  return out;
}

function normalizeClientSeq(clientSeq) {
  const seq = Number(clientSeq);
  return Number.isInteger(seq) && seq >= 0 ? seq : null;
}

function isPromiseLike(value) {
  return !!value && typeof value === "object" && typeof value.then === "function";
}

function defaultNow() {
  return typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
}

function normalizeOwnerId(ownerId) {
  const value = Number(ownerId);
  return Number.isInteger(value) && value > 0 ? value : null;
}

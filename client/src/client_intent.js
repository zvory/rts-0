import { CommandComposer } from "./command_composer.js";

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
    /** @type {null | {id:string,kind:string,payload?:object,label?:string,keepArmedOnWorldClick?:boolean}} */
    this.activeLabTool = null;
    this._nextLabToolId = 1;
    /** @type {null | {quickCast:boolean,target:string|object,queued:boolean}} */
    this.lastCommandTargetArm = null;
    /** @type {Array<{kind:string,x:number,y:number,append:boolean,radiusTiles:number|null,createdAt:number}>} */
    this.commandFeedback = [];
    /** @type {null | {resourceId:number, resourceX:number, resourceY:number, ccId:number, ccX:number, ccY:number, inRange:boolean}} */
    this.resourceMiningPreview = null;
    /** @type {null | {mouseX:number, mouseY:number, guns:Array<object>}} */
    this.antiTankGunSetupPreview = null;
    /** @type {null | {ability:string, source?:string, mouseX?:number, mouseY?:number, carriers:Array<object>, areaOrigins?:Array<object>, rangeOrigins?:Array<object>, pathOrigins?:Array<object>, returnMarkers?:Array<object>, rangePx?:number, hoverInRange:boolean, hoverInsideMinRange?:boolean}} */
    this.abilityTargetPreview = null;
  }

  /** Open the worker build command-card submenu. */
  openWorkerBuildMenu() {
    this._clearActiveLabTool();
    this.placement = null;
    this.commandTarget = null;
    this.lastCommandTargetArm = null;
    this.antiTankGunSetupPreview = null;
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
  holdCommandTarget(kind, key, shiftKey = false) {
    this.commandComposer.hold(kind, key, { shiftKey });
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
   */
  addCommandFeedback(kind, x, y, append = false, radiusTiles = null, now = this._now()) {
    this.commandFeedback.push({
      kind,
      x,
      y,
      append: !!append,
      radiusTiles,
      createdAt: now,
    });
    if (this.commandFeedback.length > 12) {
      this.commandFeedback.splice(0, this.commandFeedback.length - 12);
    }
  }

  /**
   * Return live command feedback markers, pruning expired ones.
   * @param {number} now
   * @returns {Array<{kind:string,x:number,y:number,append:boolean,createdAt:number}>}
   */
  liveCommandFeedback(now) {
    const ttlMs = 650;
    this.commandFeedback = this.commandFeedback.filter((f) => now - f.createdAt <= ttlMs);
    return this.commandFeedback;
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
   * @param {null | {mouseX:number, mouseY:number, guns:Array<object>}} preview
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
   * Arm a lab setup tool for world clicks.
   * @param {{kind:string,payload?:object,label?:string,id?:string,keepArmedOnWorldClick?:boolean}} tool
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
    this.resourceMiningPreview = null;
    const active = { id, kind };
    if (tool?.payload && typeof tool.payload === "object") active.payload = { ...tool.payload };
    if (typeof tool?.label === "string" && tool.label) active.label = tool.label;
    if (tool?.keepArmedOnWorldClick) active.keepArmedOnWorldClick = true;
    this.activeLabTool = active;
    return active;
  }

  /** Clear the active lab setup tool, if any. */
  cancelLabTool(reason = "cancelled") {
    const active = this.activeLabTool;
    this.activeLabTool = null;
    return active ? { ...active, reason } : null;
  }

  _clearActiveLabTool() {
    this.activeLabTool = null;
  }
}

function defaultNow() {
  return typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
}

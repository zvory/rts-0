// Narrow, launch-gated page bridge for the local Interact driver.
// It deliberately exposes typed scene operations only; callers never receive App,
// Match, transport, renderer, or GameState references.

import { ABILITY, CMD, LAB_ROLE, isUnit } from "./protocol.js";
import { factionCatalog } from "./config.js";
import {
  labBuildingSpawnFactionOptions,
  labSpawnBuildingKindsForFaction,
  labSpawnFactionOptions,
  labSpawnUnitKindsForFaction,
} from "./lab_spawn_catalog.js";
import {
  applyInteractSelection,
  selectedInteractEntityIds,
} from "./interact_selection.js";

export const INTERACT_BRIDGE_KEY = "__rtsInteract";
export const INTERACT_BRIDGE_VERSION = 6;
export const INTERACT_LIMITS = Object.freeze({
  inspectEntities: 400,
  inspectPlayers: 16,
  inspectKinds: 32,
  removeEntities: 400,
  mutationEntities: 400,
  focusEntities: 400,
  stepTicks: 100,
  seekTick: 1_000_000,
  waitMs: 8_000,
  captureSubjects: 400,
  selectionEntities: 400,
});

const INTERACT_DEFAULT_FOCUS_PADDING = 48;
const INTERACT_SINGLE_SUBJECT_FOCUS_PADDING = 32;

export function interactLaunchEnabled(locationLike = globalThis.location) {
  try {
    const pathname = locationLike?.pathname || "";
    return (pathname === "/lab" || pathname === "/lab/") &&
      new URLSearchParams(locationLike?.search || "").get("interact") === "lab";
  } catch {
    return false;
  }
}

export class InteractBridge {
  constructor({ app, windowLike = globalThis.window, enabled = interactLaunchEnabled(), sleep = delay } = {}) {
    this.app = app;
    this.windowLike = windowLike;
    this.enabled = !!enabled;
    this.sleep = sleep;
    this.destroyed = false;
    this.launchError = "";
    this.surface = Object.freeze({
      version: INTERACT_BRIDGE_VERSION,
      status: () => this.status(),
      call: (method, input) => this.call(method, input),
    });
    if (this.enabled && this.windowLike) this.windowLike[INTERACT_BRIDGE_KEY] = this.surface;
  }

  status() {
    const match = this.app?.match || null;
    const labClient = this.app?.labClient || null;
    const roomTime = match?.roomTimeControls?.roomTimeState || null;
    const websocketConnected = this.app?.net?.ws?.readyState === 1;
    const startReceived = !!match && !!labClient;
    const operator = labClient?.state?.role === LAB_ROLE.OPERATOR;
    const snapshotApplied = match?.state?.currRecvTime != null;
    const roomTimeKnown = !match?.capabilities?.roomTime?.available || !!roomTime;
    const reason = this.destroyed
      ? "bridgeClosed"
      : !this.enabled
        ? "launchGateDisabled"
        : this.launchError
          ? "launchError"
          : !websocketConnected
            ? "websocketDisconnected"
            : !startReceived
              ? "waitingForStart"
              : !operator
                ? "labOperatorRequired"
                : !snapshotApplied
                  ? "waitingForSnapshot"
                  : !roomTimeKnown
                    ? "waitingForRoomTime"
                    : "ready";
    return {
      version: INTERACT_BRIDGE_VERSION,
      enabled: this.enabled && !this.destroyed,
      ready: reason === "ready",
      reason,
      launchError: this.launchError,
      websocketConnected,
      startReceived,
      labRole: labClient?.state?.role || "",
      room: labClient?.state?.room || "",
      snapshotTick: snapshotApplied ? match.state.tick : null,
      roomTime: projectRoomTime(roomTime),
      camera: projectCamera(match?.camera),
      cameraViewport: projectCameraViewport(match?.camera),
      cameraWorldBounds: projectCameraWorldBounds(match?.camera),
      selection: selectedInteractEntityIds(match?.state, INTERACT_LIMITS.selectionEntities),
    };
  }

  noteLaunchError(message) {
    if (this.destroyed || this.status().ready) return;
    this.launchError = String(message || "Interact Lab launch failed.");
  }

  async call(method, input = {}) {
    try {
      const value = await this.dispatch(method, input);
      return { ok: true, value };
    } catch (error) {
      return {
        ok: false,
        error: {
          code: error?.code || "bridgeError",
          message: error?.message || "Interact bridge request failed.",
          details: error?.details && typeof error.details === "object" ? error.details : undefined,
        },
      };
    }
  }

  async dispatch(method, input) {
    switch (method) {
      case "status": return this.status();
      case "catalog": return this.catalog();
      case "spawn": return this.spawn(input);
      case "update": return this.update(input);
      case "remove": return this.remove(input);
      case "order": return this.order(input);
      case "time": return this.time(input);
      case "inspect": return this.inspect(input);
      case "select": return this.select(input);
      case "camera": return this.camera(input);
      case "reset": return this.reset();
      case "exportSetup": return this.exportSetup(input);
      case "importSetup": return this.importSetup(input);
      case "presentation": return this.presentation(input);
      case "captureReadiness": return this.captureReadiness(input);
      case "captureFixedEnter": return this.captureFixedEnter();
      case "captureFixedFrame": return this.captureFixedFrame(input);
      case "captureFixedExit": return this.captureFixedExit();
      default: throw bridgeError("unknownMethod", `Unknown Interact bridge method ${JSON.stringify(method)}.`);
    }
  }

  captureFixedEnter() {
    const { match } = this.session();
    if (!isPaused(match)) throw bridgeError("roomTimeNotPaused", "Fixed capture requires paused authoritative room time.");
    return match.enterFixedCapture();
  }

  captureFixedFrame(input) {
    const { match } = this.session();
    const visualTimeMs = finiteNumber(input?.visualTimeMs, "captureFixedFrame.visualTimeMs");
    return match.renderFixedCaptureFrame(visualTimeMs);
  }

  captureFixedExit() {
    const match = this.app?.match;
    return match?.exitFixedCapture?.() || { resumed: false };
  }

  session() {
    const status = this.status();
    if (!status.ready) throw bridgeError(status.reason, `Interact is not ready: ${status.reason}.`);
    return { match: this.app.match, labClient: this.app.labClient };
  }

  catalog() {
    const { match } = this.session();
    const players = match.state.players.slice(0, INTERACT_LIMITS.inspectPlayers).map(projectPlayer);
    const factions = labSpawnFactionOptions().map((faction) => ({
      id: faction.id,
      label: faction.label,
      units: labSpawnUnitKindsForFaction(faction.id),
      buildings: labSpawnBuildingKindsForFaction(faction.id),
      upgrades: factionUpgrades(faction.id),
    }));
    return {
      maps: [projectMap(match.state.map)],
      players,
      factions,
      supportedCommandKinds: Object.values(CMD),
      abilities: Object.values(ABILITY),
    };
  }

  async spawn(input) {
    const { labClient } = this.session();
    if (!Array.isArray(input?.spawns) || input.spawns.length < 1 || input.spawns.length > INTERACT_LIMITS.mutationEntities) {
      throw bridgeError("invalidInput", "spawn.spawns must contain 1-400 items.");
    }
    const spawns = input.spawns.map((spec, index) => ({
      owner: positiveInt(spec?.owner, `spawn.spawns[${index}].owner`),
      kind: safeKind(spec?.kind, `spawn.spawns[${index}].kind`),
      x: finiteNumber(spec?.x, `spawn.spawns[${index}].x`),
      y: finiteNumber(spec?.y, `spawn.spawns[${index}].y`),
      completed: spec?.completed !== false,
    }));
    const result = await this.mutate(
      () => labClient.spawnEntities(spawns),
      (outcome) => batchOutcomes(outcome).length === spawns.length,
    );
    const entities = batchOutcomes(result.outcome)
      .map((item) => this.app.match.state.entityById(item?.outcome?.entityId))
      .map((entity) => entity ? projectEntity(entity) : null);
    return { result: projectLabResult(result), entities };
  }

  async update(input) {
    const { labClient } = this.session();
    if (!Array.isArray(input?.updates) || input.updates.length < 1 || input.updates.length > INTERACT_LIMITS.mutationEntities) {
      throw bridgeError("invalidInput", "update.updates must contain 1-400 items.");
    }
    const updates = input.updates.map((value, index) => normalizeBridgeUpdate(value, index));
    const result = await this.mutate(
      () => labClient.applyUpdates(updates),
      (outcome) => batchOutcomes(outcome).every((item) => this.outcomeMatchesProjection(item?.outcome)),
    );
    return { result: projectLabResult(result) };
  }

  async remove(input) {
    const { labClient } = this.session();
    const ids = boundedIds(input?.entityIds, "remove.entityIds", INTERACT_LIMITS.mutationEntities);
    const result = await this.mutate(
      () => labClient.deleteEntities(ids),
      () => ids.every((entityId) => !this.app.match.state.entityById(entityId)),
    );
    return { result: projectLabResult(result) };
  }

  async order(input) {
    const { labClient } = this.session();
    const playerId = positiveInt(input?.playerId, "order.playerId");
    const command = boundedCommand(input?.command);
    const result = await this.mutate(
      () => labClient.request({
        op: "issueCommandAs",
        playerId,
        cmd: command,
        ignoreCommandLimits: input?.ignoreCommandLimits === true,
      }),
      () => true,
      { advancePaused: true },
    );
    return { result: projectLabResult(result) };
  }

  async time(input) {
    const { match } = this.session();
    const action = String(input?.action || "");
    if (action === "pause") {
      match.net.setRoomTimeSpeed(0);
      await this.waitFor(() => isPaused(match), "room time to pause");
    } else if (action === "resume") {
      const speed = boundedSpeed(input?.speed ?? 1);
      match.net.setRoomTimeSpeed(speed);
      await this.waitFor(() => !isPaused(match), "room time to resume");
    } else if (action === "speed") {
      const speed = boundedSpeed(input?.speed);
      match.net.setRoomTimeSpeed(speed);
      await this.waitFor(() => Number(match.roomTimeControls?.roomTimeState?.speed) === speed, "room time speed");
    } else if (action === "step") {
      const ticks = boundedPositiveInt(input?.ticks ?? 1, "time.ticks", INTERACT_LIMITS.stepTicks);
      for (let index = 0; index < ticks; index += 1) {
        const previous = snapshotSequence(match);
        match.net.stepRoomTime();
        await this.waitFor(() => snapshotSequence(match) > previous, "room time step");
      }
    } else if (action === "seek") {
      const tick = boundedNonNegativeInt(input?.tick, "time.tick", INTERACT_LIMITS.seekTick);
      match.net.seekRoomTimeTo(tick);
      // Lab seek rebuilds the authoritative game and intentionally sends a fresh start payload.
      // Follow the app-owned replacement Match instead of retaining the pre-seek instance. The
      // server clamps a valid target to retained history, so report its observed tick rather than
      // timing out waiting for the caller's unclamped value.
      await this.waitFor(() => {
        const active = this.app?.match;
        return active && active !== match && active.state?.currRecvTime != null;
      }, `room time seek to ${tick}`);
    } else {
      throw bridgeError("invalidTime", "time.action must be pause, resume, speed, step, or seek.");
    }
    const active = this.app?.match || match;
    return { roomTime: projectRoomTime(active.roomTimeControls?.roomTimeState), snapshotTick: active.state.tick };
  }

  inspect(query) {
    const { match } = this.session();
    const normalized = normalizeInspectionQuery(query);
    const entities = match.state.entitiesInterpolated(1, { includePrediction: false })
      .filter((entity) => inspectionIncludesEntity(entity, normalized, match.camera))
      .slice(0, normalized.limit)
      .map(projectEntity);
    const allMatching = match.state.entitiesInterpolated(1, { includePrediction: false })
      .filter((entity) => inspectionIncludesEntity(entity, normalized, match.camera)).length;
    return {
      entities,
      truncated: allMatching > entities.length,
      totalMatching: allMatching,
      players: match.state.players.slice(0, INTERACT_LIMITS.inspectPlayers).map(projectPlayer),
      room: {
        tick: match.state.tick,
        roomTime: projectRoomTime(match.roomTimeControls?.roomTimeState),
        map: projectMap(match.state.map),
      },
      camera: projectCamera(match.camera),
      cameraViewport: projectCameraViewport(match.camera),
      cameraWorldBounds: projectCameraWorldBounds(match.camera),
      selection: selectedInteractEntityIds(match.state, INTERACT_LIMITS.selectionEntities),
    };
  }

  async select(input = {}) {
    const { match } = this.session();
    const entityIds = optionalBoundedIds(
      input?.entityIds,
      "select.entityIds",
      INTERACT_LIMITS.selectionEntities,
    );
    const entities = entityIds.map((id) => match.state.entityById(id));
    if (entities.some((entity) => !isSelectableEntity(entity))) {
      throw bridgeError("unknownEntity", "select contains an entity that is not selectable in the current snapshot.");
    }
    applyInteractSelection(match, entityIds);
    await animationFrames(2);
    const selection = selectedInteractEntityIds(match.state, INTERACT_LIMITS.selectionEntities);
    return {
      selection,
      entities: selection.map((id) => projectEntity(match.state.entityById(id))).filter(Boolean),
    };
  }

  camera(input) {
    const { match } = this.session();
    const action = String(input?.action || "");
    if (action === "set") {
      if (!match.camera.restore(input?.snapshot)) {
        throw bridgeError("invalidCamera", "camera.snapshot must be a valid CameraSnapshotV1.");
      }
    } else if (action === "focus") {
      const ids = boundedIds(input?.entityIds, "camera.entityIds", INTERACT_LIMITS.focusEntities);
      const entities = ids.map((id) => match.state.entityById(id)).filter(Boolean);
      if (entities.length !== ids.length) throw bridgeError("unknownEntity", "camera.focus contains an entity that is not in the current snapshot.");
      const defaultPadding = entities.length === 1 && isUnit(entities[0].kind)
        ? INTERACT_SINGLE_SUBJECT_FOCUS_PADDING
        : INTERACT_DEFAULT_FOCUS_PADDING;
      const padding = boundedNonNegativeNumber(input?.padding ?? defaultPadding, "camera.padding", 1024);
      const minX = Math.min(...entities.map((entity) => entity.x));
      const maxX = Math.max(...entities.map((entity) => entity.x));
      const minY = Math.min(...entities.map((entity) => entity.y));
      const maxY = Math.max(...entities.map((entity) => entity.y));
      match.camera.fitWorldPoints([
        { x: minX - padding, y: minY - padding },
        { x: maxX + padding, y: maxY + padding },
      ]);
    } else {
      throw bridgeError("invalidCamera", "camera.action must be set or focus.");
    }
    return {
      camera: projectCamera(match.camera),
      cameraViewport: projectCameraViewport(match.camera),
      cameraWorldBounds: projectCameraWorldBounds(match.camera),
    };
  }

  reset() {
    return this.time({ action: "seek", tick: 0 });
  }

  async exportSetup(input = {}) {
    const { labClient } = this.session();
    const name = typeof input.name === "string" ? input.name.slice(0, 80) : "";
    const result = await labClient.exportScenario(name);
    if (!result?.ok || !result?.outcome?.scenario) {
      throw bridgeError("setupExportRejected", result?.error || "The server rejected setup export.");
    }
    return { scenario: result.outcome.scenario, result: projectLabResult(result) };
  }

  async importSetup(input = {}) {
    const { labClient } = this.session();
    if (!input.scenario || typeof input.scenario !== "object" || Array.isArray(input.scenario)) {
      throw bridgeError("invalidSetup", "importSetup.scenario must be a checkpoint setup object.");
    }
    const result = await this.mutate(
      () => labClient.importScenario(input.scenario),
      () => true,
    );
    return { result: projectLabResult(result), entityIdMap: result.outcome?.entityIdMap || [] };
  }

  async presentation(input = {}) {
    const mode = String(input?.mode || "");
    if (mode !== "clean" && mode !== "default") {
      throw bridgeError("invalidPresentation", "presentation.mode must be clean or default.");
    }
    const { match } = this.session();
    if (typeof this.app?.setCleanPresentation === "function") {
      this.app.setCleanPresentation(mode === "clean");
    } else {
      match.handleResize?.();
    }
    await animationFrames(2);
    return {
      mode,
      viewport: projectViewport(),
      camera: projectCamera(match.camera),
      cameraViewport: projectCameraViewport(match.camera),
      cameraWorldBounds: projectCameraWorldBounds(match.camera),
    };
  }

  captureReadiness(input = {}) {
    const { match } = this.session();
    const subjectIds = optionalBoundedIds(
      input?.subjectIds,
      "captureReadiness.subjectIds",
      INTERACT_LIMITS.captureSubjects,
    );
    const subjectEntities = subjectIds.map((id) => match.state.entityById(id)).filter(Boolean);
    if (subjectEntities.length !== subjectIds.length) {
      throw bridgeError("unknownEntity", "captureReadiness contains an entity that is not in the current snapshot.");
    }
    const renderer = match.renderer;
    const rendererReadiness = renderer?.captureReadiness?.({
      subjectIds,
      subjectKinds: subjectEntities.map((entity) => entity.kind),
    }) || {
      frame: 0,
      assets: [],
      ready: false,
      failedAssets: [],
      pendingAssets: [],
      renderErrors: [{ label: "rendererUnavailable", count: 1, message: "Renderer is unavailable." }],
      missingTextureSubjectIds: [],
    };
    const fonts = documentFontsStatus();
    const frameErrors = Number(match.frameErrors?.count) || 0;
    const ready = rendererReadiness.ready && fonts.status === "ready" &&
      frameErrors === 0 && rendererReadiness.renderErrors.length === 0 &&
      rendererReadiness.missingTextureSubjectIds.length === 0;
    return {
      ...rendererReadiness,
      ready,
      frame: rendererReadiness.frame,
      snapshotTick: match.state.tick,
      roomTime: projectRoomTime(match.roomTimeControls?.roomTimeState),
      viewport: projectViewport(),
      camera: projectCamera(match.camera),
      cameraViewport: projectCameraViewport(match.camera),
      cameraWorldBounds: projectCameraWorldBounds(match.camera),
      selection: selectedInteractEntityIds(match.state, INTERACT_LIMITS.selectionEntities),
      visualProfileId: match.visualProfile?.id || null,
      subjects: subjectEntities.map(projectEntity),
      fonts,
      frameErrors: frameErrors > 0 ? [{ count: frameErrors, message: match.frameErrors?.lastMessage || "" }] : [],
    };
  }

  async mutate(send, observed, { advancePaused = false } = {}) {
    const { match } = this.session();
    const before = snapshotSequence(match);
    const result = await send();
    if (!result?.ok) {
      throw bridgeError("labRejected", result?.error || "The server rejected the lab operation.", {
        failedIndex: result?.failedIndex ?? null,
        ...(result?.details || {}),
      });
    }
    // Successful setup mutations fan out their current authoritative state without advancing
    // paused simulation. Commands still need one tick so the queued order can be consumed.
    if (advancePaused && isPaused(match)) match.net.stepRoomTime();
    await this.waitFor(
      () => snapshotSequence(match) > before && observed(result.outcome || null),
      `authoritative snapshot for ${result.op || "lab operation"}`,
    );
    return { ...result, snapshotTick: match.state.tick };
  }

  outcomeMatchesProjection(outcome) {
    if (Number.isInteger(outcome?.entityId) && Number.isFinite(outcome?.x) && Number.isFinite(outcome?.y)) {
      const entity = this.app.match.state.entityById(outcome.entityId);
      return !entity || (Math.abs(entity.x - outcome.x) < 0.01 && Math.abs(entity.y - outcome.y) < 0.01);
    }
    if (Number.isInteger(outcome?.entityId) && Number.isInteger(outcome?.owner)) {
      const entity = this.app.match.state.entityById(outcome.entityId);
      return !entity || entity.owner === outcome.owner;
    }
    if (Number.isInteger(outcome?.playerId) && Number.isInteger(outcome?.steel) && Number.isInteger(outcome?.oil)) {
      const row = this.app.match.state.playerResources.find((player) => Number(player?.id) === outcome.playerId);
      return !row || (row.steel === outcome.steel && row.oil === outcome.oil);
    }
    if (Number.isInteger(outcome?.playerId) && typeof outcome?.enabled === "boolean") {
      return this.app.labClient.state?.godModePlayers?.includes(outcome.playerId) === outcome.enabled;
    }
    return true;
  }

  async waitFor(predicate, detail, timeoutMs = INTERACT_LIMITS.waitMs) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      if (predicate()) return;
      await this.sleep(25);
    }
    throw bridgeError("snapshotTimeout", `Timed out waiting for ${detail}.`);
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    if (this.windowLike?.[INTERACT_BRIDGE_KEY] === this.surface) delete this.windowLike[INTERACT_BRIDGE_KEY];
  }
}

export function normalizeInspectionQuery(query = {}) {
  const ids = optionalBoundedIds(query.ids, "inspect.ids", INTERACT_LIMITS.inspectEntities);
  const owners = optionalBoundedIds(query.owners, "inspect.owners", INTERACT_LIMITS.inspectPlayers);
  let kinds = [];
  if (query.kinds != null) {
    if (!Array.isArray(query.kinds) || query.kinds.length > INTERACT_LIMITS.inspectKinds) {
      throw bridgeError(
        "invalidInput",
        `inspect.kinds must contain at most ${INTERACT_LIMITS.inspectKinds} kind tokens.`,
      );
    }
    kinds = [...new Set(query.kinds.map((kind) => safeKind(kind, "inspect.kinds")))];
  }
  return {
    ids: new Set(ids),
    owners: new Set(owners),
    kinds: new Set(kinds),
    cameraViewport: query.cameraViewport === true,
    limit: boundedPositiveInt(query.limit ?? 25, "inspect.limit", INTERACT_LIMITS.inspectEntities),
  };
}

function inspectionIncludesEntity(entity, query, camera) {
  if (query.ids.size > 0 && !query.ids.has(entity.id)) return false;
  if (query.owners.size > 0 && !query.owners.has(entity.owner)) return false;
  if (query.kinds.size > 0 && !query.kinds.has(entity.kind)) return false;
  if (query.cameraViewport && !entityInCameraViewport(entity, camera)) return false;
  return true;
}

function isSelectableEntity(entity) {
  return !!entity && entity.shotReveal !== true && entity.visionOnly !== true;
}

function entityInCameraViewport(entity, camera) {
  return camera?.containsProjected?.({ x: entity.x, y: entity.y, heightPx: 0 }) === true;
}

function boundedCommand(command) {
  if (!command || typeof command !== "object" || Array.isArray(command)) {
    throw bridgeError("invalidCommand", "order.command must be a normal protocol command object.");
  }
  const encoded = JSON.stringify(command);
  if (encoded.length > 16_384) throw bridgeError("invalidCommand", "order.command exceeds the 16 KiB bridge limit.");
  return command;
}

function factionUpgrades(factionId) {
  const research = factionCatalog(factionId).research || {};
  return [...new Set(Object.values(research).flat())].sort();
}

function projectEntity(entity) {
  if (!entity) return null;
  const targetId = Number.isInteger(entity.targetId) ? entity.targetId : null;
  const state = typeof entity.state === "string" ? entity.state : "";
  return {
    id: entity.id,
    kind: entity.kind,
    owner: entity.owner,
    x: finiteOrNull(entity.x),
    y: finiteOrNull(entity.y),
    hp: finiteOrNull(entity.hp),
    maxHp: finiteOrNull(entity.maxHp),
    state,
    activity: targetId != null ? "engaging" : state,
    targetId,
    weaponFacing: finiteOrNull(entity.weaponFacing),
    setupState: typeof entity.setupState === "string" ? entity.setupState : null,
    orderPlan: Array.isArray(entity.orderPlan) ? entity.orderPlan.slice(0, 8).map(projectOrderStage) : [],
  };
}

function projectOrderStage(stage) {
  return {
    kind: typeof stage?.kind === "string" ? stage.kind : "",
    x: finiteOrNull(stage?.x),
    y: finiteOrNull(stage?.y),
    target: Number.isInteger(stage?.target) ? stage.target : null,
  };
}

function projectPlayer(player) {
  return {
    id: player.id,
    teamId: player.teamId,
    factionId: player.factionId,
    name: player.name || "",
    color: player.color || "",
  };
}

function projectMap(map) {
  return {
    name: map?.name || "",
    width: finiteOrNull(map?.width),
    height: finiteOrNull(map?.height),
    tileSize: finiteOrNull(map?.tileSize),
  };
}

function projectCamera(camera) {
  const snapshot = camera?.snapshot?.();
  return snapshot?.version === 1 ? snapshot : null;
}

function projectCameraViewport(camera) {
  const viewport = camera?.projectionSnapshot?.()?.viewport;
  return Number.isFinite(viewport?.widthCssPx) && Number.isFinite(viewport?.heightCssPx)
    ? viewport
    : null;
}

function projectCameraWorldBounds(camera) {
  const bounds = camera?.viewportGroundBounds?.();
  return Number.isFinite(bounds?.minX) && Number.isFinite(bounds?.minY) &&
    Number.isFinite(bounds?.maxX) && Number.isFinite(bounds?.maxY)
    ? bounds
    : null;
}

function projectRoomTime(roomTime) {
  if (!roomTime || typeof roomTime !== "object") return null;
  return {
    currentTick: finiteOrNull(roomTime.currentTick),
    durationTicks: finiteOrNull(roomTime.durationTicks),
    speed: finiteOrNull(roomTime.speed),
    paused: roomTime.paused === true || roomTime.speed === 0,
  };
}

function projectViewport() {
  const viewport = typeof document !== "undefined" ? document.getElementById("viewport") : null;
  const rect = viewport?.getBoundingClientRect?.();
  return {
    x: finiteOrNull(rect?.x),
    y: finiteOrNull(rect?.y),
    width: finiteOrNull(rect?.width),
    height: finiteOrNull(rect?.height),
    devicePixelRatio: finiteOrNull(globalThis.devicePixelRatio),
  };
}

function documentFontsStatus() {
  const fonts = typeof document !== "undefined" ? document.fonts : null;
  if (!fonts) return { status: "ready", supported: false };
  return {
    status: fonts.status === "loaded" ? "ready" : "pending",
    supported: true,
  };
}

function projectLabResult(result) {
  return {
    op: result.op || "",
    outcome: result.outcome || null,
    failedIndex: Number.isInteger(result.failedIndex) ? result.failedIndex : null,
    details: result.details || null,
    snapshotTick: finiteOrNull(result.snapshotTick),
  };
}

function batchOutcomes(outcome) {
  return Array.isArray(outcome?.items) ? outcome.items : [];
}

function normalizeBridgeUpdate(value, index) {
  const operation = String(value?.operation || "");
  const label = `update.updates[${index}]`;
  if (operation === "move") return {
    operation,
    entityId: positiveInt(value?.entityId, `${label}.entityId`),
    x: finiteNumber(value?.x, `${label}.x`),
    y: finiteNumber(value?.y, `${label}.y`),
  };
  if (operation === "reassign") return {
    operation,
    entityId: positiveInt(value?.entityId, `${label}.entityId`),
    owner: positiveInt(value?.owner, `${label}.owner`),
  };
  if (operation === "resources") return {
    operation,
    playerId: positiveInt(value?.playerId, `${label}.playerId`),
    steel: nonNegativeInt(value?.steel, `${label}.steel`),
    oil: nonNegativeInt(value?.oil, `${label}.oil`),
  };
  if (operation === "research") return {
    operation,
    playerId: positiveInt(value?.playerId, `${label}.playerId`),
    upgrade: safeKind(value?.upgrade, `${label}.upgrade`),
    completed: value?.completed !== false,
  };
  if (operation === "godMode") return {
    operation,
    playerId: positiveInt(value?.playerId, `${label}.playerId`),
    enabled: value?.enabled !== false,
  };
  throw bridgeError("invalidUpdate", `${label}.operation is unsupported.`);
}

function snapshotSequence(match) {
  return Number(match?.state?.currRecvTime) || 0;
}

function isPaused(match) {
  const roomTime = match?.roomTimeControls?.roomTimeState;
  return roomTime?.paused === true || roomTime?.speed === 0;
}

function positiveInt(value, label) {
  const number = Number(value);
  if (!Number.isInteger(number) || number <= 0 || number > 0xffffffff) {
    throw bridgeError("invalidInput", `${label} must be a positive u32.`);
  }
  return number;
}

function nonNegativeInt(value, label) {
  const number = Number(value);
  if (!Number.isInteger(number) || number < 0 || number > 0xffffffff) {
    throw bridgeError("invalidInput", `${label} must be a non-negative u32.`);
  }
  return number;
}

function boundedPositiveInt(value, label, maximum) {
  const number = Number(value);
  if (!Number.isInteger(number) || number <= 0 || number > maximum) {
    throw bridgeError("invalidInput", `${label} must be an integer from 1 to ${maximum}.`);
  }
  return number;
}

function boundedNonNegativeInt(value, label, maximum) {
  const number = Number(value);
  if (!Number.isInteger(number) || number < 0 || number > maximum) {
    throw bridgeError("invalidInput", `${label} must be an integer from 0 to ${maximum}.`);
  }
  return number;
}

function boundedIds(values, label, maximum) {
  if (!Array.isArray(values) || values.length === 0 || values.length > maximum) {
    throw bridgeError("invalidInput", `${label} must contain 1 to ${maximum} entity ids.`);
  }
  return [...new Set(values.map((value) => positiveInt(value, label)))];
}

function optionalBoundedIds(values, label, maximum) {
  if (values == null) return [];
  if (!Array.isArray(values) || values.length > maximum) {
    throw bridgeError("invalidInput", `${label} must contain at most ${maximum} positive ids.`);
  }
  return [...new Set(values.map((value) => positiveInt(value, label)))];
}

function boundedSpeed(value) {
  const speed = Number(value);
  if (!Number.isFinite(speed) || speed < 0 || speed > 16) {
    throw bridgeError("invalidInput", "time.speed must be a number from 0 to 16.");
  }
  return speed;
}

function boundedNonNegativeNumber(value, label, maximum) {
  const number = Number(value);
  if (!Number.isFinite(number) || number < 0 || number > maximum) {
    throw bridgeError("invalidInput", `${label} must be a number from 0 to ${maximum}.`);
  }
  return number;
}

function safeKind(value, label) {
  const kind = String(value || "").trim();
  if (!/^[A-Za-z0-9_]{1,64}$/.test(kind)) {
    throw bridgeError("invalidInput", `${label} must be a known kind token.`);
  }
  return kind;
}

function finiteNumber(value, label) {
  const number = Number(value);
  if (!Number.isFinite(number)) throw bridgeError("invalidInput", `${label} must be finite.`);
  return number;
}

function finiteOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function bridgeError(code, message, details = undefined) {
  const error = new Error(message);
  error.code = code;
  if (details) error.details = details;
  return error;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function animationFrames(count) {
  if (typeof requestAnimationFrame !== "function") return Promise.resolve();
  let remaining = count;
  return new Promise((resolve) => {
    const next = () => {
      remaining -= 1;
      if (remaining <= 0) resolve();
      else requestAnimationFrame(next);
    };
    requestAnimationFrame(next);
  });
}

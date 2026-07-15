// Narrow, launch-gated automation for one isolated human-vs-AI match.
// This surface intentionally supports observation, move orders, and surrender only.

import { cmd, isUnit } from "./protocol.js";
import { INTERACT_BRIDGE_KEY } from "./interact_bridge.js";

export const INTERACT_GAME_BRIDGE_VERSION = 2;
export const INTERACT_GAME_LIMITS = Object.freeze({
  inspectEntities: 400,
  inspectKinds: 32,
  moveUnits: 100,
  focusEntities: 400,
  captureSubjects: 400,
  waitMs: 8_000,
});

const GAME_ROOM_PREFIX = "interact-game-";
const DEV_SCENARIO_TOKEN_RE = /^[a-z0-9_]+$/;
const DEFAULT_FOCUS_PADDING = 48;
const SINGLE_UNIT_FOCUS_PADDING = 32;

export function interactGameLaunchEnabled(locationLike = globalThis.location) {
  try {
    const params = new URLSearchParams(locationLike?.search || "");
    const pathname = locationLike?.pathname || "";
    return (pathname === "/" || pathname === "") &&
      params.get("interact") === "game" &&
      params.get("rtsLaunch") === "match" &&
      ["player", "spectator"].includes(params.get("rtsRole")) &&
      String(params.get("rtsRoom") || "").startsWith(GAME_ROOM_PREFIX);
  } catch {
    return false;
  }
}

export function interactScenarioLaunchEnabled(locationLike = globalThis.location) {
  try {
    const params = new URLSearchParams(locationLike?.search || "");
    const pathname = locationLike?.pathname || "";
    const blocker = params.get("blocker") || "";
    const scenarioCase = params.get("case") || "";
    return (pathname === "/" || pathname === "") &&
      params.get("interact") === "scenario" &&
      params.get("watchScenario") === "1" &&
      DEV_SCENARIO_TOKEN_RE.test(params.get("id") || "") &&
      DEV_SCENARIO_TOKEN_RE.test(params.get("unit") || "") &&
      /^[1-9][0-9]*$/.test(params.get("count") || "") &&
      (!blocker || DEV_SCENARIO_TOKEN_RE.test(blocker)) &&
      (!scenarioCase || DEV_SCENARIO_TOKEN_RE.test(scenarioCase));
  } catch {
    return false;
  }
}

export class InteractGameBridge {
  constructor({
    app,
    windowLike = globalThis.window,
    enabled = interactGameLaunchEnabled() || interactScenarioLaunchEnabled(),
    mode = interactScenarioLaunchEnabled() ? "scenario" : "game",
    sleep = delay,
  } = {}) {
    this.app = app;
    this.windowLike = windowLike;
    this.enabled = !!enabled;
    this.mode = mode === "scenario" ? "scenario" : "game";
    this.sleep = sleep;
    this.destroyed = false;
    this.surface = Object.freeze({
      version: INTERACT_GAME_BRIDGE_VERSION,
      status: () => this.status(),
      call: (method, input) => this.call(method, input),
    });
    if (this.enabled && this.windowLike) this.windowLike[INTERACT_BRIDGE_KEY] = this.surface;
  }

  status() {
    const match = this.app?.match || null;
    const websocketConnected = this.app?.net?.ws?.readyState === 1;
    const snapshotApplied = match?.state?.currRecvTime != null;
    const phase = gamePhase(match);
    const launchFailed = this.app?.matchLaunchFailed === true;
    const reason = this.destroyed
      ? "bridgeClosed"
      : !this.enabled
        ? "launchGateDisabled"
        : launchFailed
          ? "matchLaunchFailed"
          : !websocketConnected
            ? "websocketDisconnected"
            : !match
              ? "waitingForStart"
              : !snapshotApplied
                  ? "waitingForSnapshot"
                  : "ready";
    return {
      version: INTERACT_GAME_BRIDGE_VERSION,
      mode: this.mode,
      enabled: this.enabled && !this.destroyed,
      ready: reason === "ready",
      reason,
      websocketConnected,
      startReceived: !!match,
      room: this.app?.matchLaunch?.room || this.app?.devWatch?.room || "",
      snapshotTick: snapshotApplied ? match.state.tick : null,
      playerId: match?.state?.playerId ?? null,
      role: match?.state?.spectator ? "spectator" : "player",
      phase,
      roomTime: projectRoomTime(match?.roomTimeControls?.roomTimeState),
      matchRunId: match?.matchRunId || "",
      camera: projectCamera(match?.camera),
      cameraViewport: projectCameraViewport(match?.camera),
      cameraWorldBounds: projectCameraWorldBounds(match?.camera),
      ui: projectUi(),
    };
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
          message: error?.message || "Interact game bridge request failed.",
          details: error?.details && typeof error.details === "object" ? error.details : undefined,
        },
      };
    }
  }

  async dispatch(method, input) {
    switch (method) {
      case "status": return this.status();
      case "inspect": return this.inspect(input);
      case "move": return this.move(input);
      case "giveUp": return this.giveUp();
      case "time": return this.time(input);
      case "camera": return this.camera(input);
      case "presentation": return this.presentation(input);
      case "captureReadiness": return this.captureReadiness(input);
      default: throw bridgeError("unknownMethod", `Unknown Interact game bridge method ${JSON.stringify(method)}.`);
    }
  }

  session({ activeMatch = false, playerSeat = false, spectator = false } = {}) {
    const status = this.status();
    if (!status.ready) throw bridgeError(status.reason, `Interact game is not ready: ${status.reason}.`);
    if (activeMatch && status.phase !== "active") {
      throw bridgeError("matchConcluded", "The isolated match has already concluded.");
    }
    if (playerSeat && status.role !== "player") throw bridgeError("playerSeatRequired", "This command requires the controlled player seat.");
    if (spectator && status.role !== "spectator") throw bridgeError("spectatorRequired", "This command requires an AI-vs-AI spectator session.");
    return { match: this.app.match, status };
  }

  inspect(input = {}) {
    const { match, status } = this.session();
    const query = normalizeInspect(input);
    const all = match.state.entitiesInterpolated(1, { includePrediction: false })
      .filter((entity) => inspectionIncludes(entity, query, match));
    return {
      entities: all.slice(0, query.limit).map((entity) => projectEntity(entity, match.state.playerId)),
      totalMatching: all.length,
      truncated: all.length > query.limit,
      player: projectLocalPlayer(match.state),
      players: match.state.players.slice(0, 16).map(projectPlayer),
      room: {
        name: status.room,
        tick: match.state.tick,
        map: projectMap(match.state.map),
        phase: status.phase,
        matchRunId: status.matchRunId,
      },
      camera: projectCamera(match.camera),
      cameraViewport: projectCameraViewport(match.camera),
      cameraWorldBounds: projectCameraWorldBounds(match.camera),
      ui: projectUi(),
    };
  }

  async move(input = {}) {
    const { match } = this.session({ activeMatch: true, playerSeat: true });
    const units = boundedIds(input.units, "move.units", INTERACT_GAME_LIMITS.moveUnits);
    const x = finiteNumber(input.x, "move.x");
    const y = finiteNumber(input.y, "move.y");
    if (!match.state.worldInBounds(x, y)) throw bridgeError("outOfBounds", "move destination must be inside the current map.");
    const entities = units.map((id) => match.state.entityById(id));
    if (entities.some((entity) => !entity)) throw bridgeError("unknownEntity", "move.units contains an entity outside the current fog-filtered snapshot.");
    if (entities.some((entity) => !isControllableUnit(entity, match.state.playerId))) {
      throw bridgeError("notControllable", "move.units may contain only the local player's units.");
    }
    const before = snapshotSequence(match);
    const issued = match.commandIssuer?.issueCommand(cmd.move(units, x, y, input.queued === true));
    if (!issued?.sent) {
      throw bridgeError("commandRejected", "The normal client command surface rejected the move order.", {
        blocked: issued?.blocked || null,
      });
    }
    await this.waitFor(() => snapshotSequence(match) > before || gamePhase(match) !== "active", "a snapshot after the move order");
    return {
      accepted: true,
      admission: "clientSent",
      clientSeq: issued.clientSeq ?? null,
      units,
      destination: { x, y },
      queued: input.queued === true,
      snapshotTick: match.state.tick,
    };
  }

  async giveUp() {
    const { match } = this.session({ activeMatch: true, playerSeat: true });
    match.requestGiveUp();
    await this.waitFor(() => gamePhase(match) === "concluded", "the authoritative score screen after giving up");
    return {
      accepted: true,
      phase: "concluded",
      snapshotTick: match.state.tick,
      ui: projectUi(),
    };
  }

  camera(input = {}) {
    const { match } = this.session();
    const action = String(input.action || "");
    if (action === "set") {
      if (!match.camera.restore(input.snapshot)) throw bridgeError("invalidCamera", "camera.snapshot must be a valid CameraSnapshotV1.");
    } else if (action === "focus") {
      const ids = boundedIds(input.entityIds, "camera.entityIds", INTERACT_GAME_LIMITS.focusEntities);
      const entities = ids.map((id) => match.state.entityById(id)).filter(isInspectableEntity);
      if (entities.length !== ids.length) throw bridgeError("unknownEntity", "camera.focus contains an unavailable entity.");
      const defaultPadding = entities.length === 1 && isUnit(entities[0].kind)
        ? SINGLE_UNIT_FOCUS_PADDING
        : DEFAULT_FOCUS_PADDING;
      const padding = boundedNumber(input.padding ?? defaultPadding, "camera.padding", 0, 1024);
      const xs = entities.map((entity) => entity.x);
      const ys = entities.map((entity) => entity.y);
      match.camera.fitWorldPoints([
        { x: Math.min(...xs) - padding, y: Math.min(...ys) - padding },
        { x: Math.max(...xs) + padding, y: Math.max(...ys) + padding },
      ]);
    } else if (action === "overview") {
      const padding = boundedNumber(input.padding ?? 24, "camera.padding", 0, 1024);
      const tileSize = finiteOrNull(match.state?.map?.tileSize);
      const width = finiteOrNull(match.state?.map?.width);
      const height = finiteOrNull(match.state?.map?.height);
      if (tileSize == null || width == null || height == null) throw bridgeError("mapUnavailable", "The current map bounds are unavailable.");
      match.setAutoSpectatorEnabled?.(false);
      match.camera.fitWorldPoints([
        { x: 0, y: 0 },
        { x: width * tileSize, y: height * tileSize },
      ], { paddingCssPx: padding });
    } else {
      throw bridgeError("invalidCamera", "camera.action must be set or focus.");
    }
    return {
      camera: projectCamera(match.camera),
      cameraViewport: projectCameraViewport(match.camera),
      cameraWorldBounds: projectCameraWorldBounds(match.camera),
    };
  }

  async time(input = {}) {
    const { match } = this.session({ activeMatch: true, spectator: true });
    if (input.action !== "speed") throw bridgeError("invalidTime", "AI-vs-AI game time supports only action=speed.");
    if (match.capabilities?.roomTime?.setSpeed !== true) throw bridgeError("roomTimeUnavailable", "This AI-only room does not expose speed control.");
    const speed = boundedNumber(input.speed, "time.speed", 0.125, 8);
    if (match.net?.setRoomTimeSpeed?.(speed) !== true) throw bridgeError("roomTimeRejected", "The room-time speed command was not sent.");
    await this.waitFor(() => Number(match.roomTimeControls?.roomTimeState?.speed) === speed || gamePhase(match) === "concluded", "AI-only room speed confirmation");
    return { roomTime: projectRoomTime(match.roomTimeControls?.roomTimeState), snapshotTick: match.state.tick };
  }

  async presentation(input = {}) {
    const mode = String(input.mode || "");
    if (mode !== "clean" && mode !== "default") throw bridgeError("invalidPresentation", "presentation.mode must be clean or default.");
    const { match } = this.session();
    if (typeof this.app?.setCleanPresentation === "function") this.app.setCleanPresentation(mode === "clean");
    else match.handleResize?.();
    await animationFrames(2);
    return { mode, viewport: projectViewport(), camera: projectCamera(match.camera) };
  }

  captureReadiness(input = {}) {
    const { match } = this.session();
    const subjectIds = optionalBoundedIds(input.subjectIds, "captureReadiness.subjectIds", INTERACT_GAME_LIMITS.captureSubjects);
    const subjects = subjectIds.map((id) => match.state.entityById(id)).filter(isInspectableEntity);
    if (subjects.length !== subjectIds.length) throw bridgeError("unknownEntity", "captureReadiness contains an unavailable entity.");
    const rendererReadiness = match.renderer?.captureReadiness?.({
      subjectIds,
      subjectKinds: subjects.map((entity) => entity.kind),
    }) || {
      frame: 0, assets: [], ready: false, failedAssets: [], pendingAssets: [],
      renderErrors: [{ label: "rendererUnavailable", count: 1, message: "Renderer is unavailable." }],
      missingTextureSubjectIds: [],
    };
    const fonts = documentFontsStatus();
    const frameErrors = Number(match.frameErrors?.count) || 0;
    return {
      ...rendererReadiness,
      ready: rendererReadiness.ready && fonts.status === "ready" && frameErrors === 0 &&
        rendererReadiness.renderErrors.length === 0 && rendererReadiness.missingTextureSubjectIds.length === 0,
      phase: gamePhase(match),
      snapshotTick: match.state.tick,
      roomTime: null,
      viewport: projectViewport(),
      camera: projectCamera(match.camera),
      cameraViewport: projectCameraViewport(match.camera),
      cameraWorldBounds: projectCameraWorldBounds(match.camera),
      visualProfileId: match.visualProfile?.id || null,
      subjects: subjects.map((entity) => projectEntity(entity, match.state.playerId)),
      fonts,
      frameErrors: frameErrors > 0 ? [{ count: frameErrors, message: match.frameErrors?.lastMessage || "" }] : [],
    };
  }

  async waitFor(predicate, detail) {
    const deadline = Date.now() + INTERACT_GAME_LIMITS.waitMs;
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

function normalizeInspect(input) {
  const ids = optionalBoundedIds(input.ids, "inspect.ids", INTERACT_GAME_LIMITS.inspectEntities);
  const ownership = String(input.ownership || "owned");
  if (ownership !== "owned" && ownership !== "visible") throw bridgeError("invalidInput", "inspect.ownership must be owned or visible.");
  let kinds = [];
  if (input.kinds != null) {
    if (!Array.isArray(input.kinds) || input.kinds.length > INTERACT_GAME_LIMITS.inspectKinds) {
      throw bridgeError("invalidInput", "inspect.kinds must contain at most 32 kind tokens.");
    }
    kinds = [...new Set(input.kinds.map((kind) => safeKind(kind, "inspect.kinds")))];
  }
  return {
    ids: new Set(ids),
    kinds: new Set(kinds),
    ownership,
    cameraViewport: input.cameraViewport === true,
    limit: boundedInteger(input.limit ?? 25, "inspect.limit", 1, INTERACT_GAME_LIMITS.inspectEntities),
  };
}

function inspectionIncludes(entity, query, match) {
  if (!isInspectableEntity(entity)) return false;
  if (query.ids.size && !query.ids.has(entity.id)) return false;
  if (query.kinds.size && !query.kinds.has(entity.kind)) return false;
  if (query.ownership === "owned" && entity.owner !== match.state.playerId) return false;
  if (query.cameraViewport && match.camera?.containsProjected?.({ x: entity.x, y: entity.y, heightPx: 0 }) !== true) return false;
  return true;
}

function projectEntity(entity, playerId = null) {
  const targetId = Number.isInteger(entity?.targetId) ? entity.targetId : null;
  const state = typeof entity?.state === "string" ? entity.state : "";
  return {
    id: entity.id,
    kind: entity.kind,
    owner: entity.owner,
    controllable: isControllableUnit(entity, playerId),
    x: finiteOrNull(entity.x),
    y: finiteOrNull(entity.y),
    hp: finiteOrNull(entity.hp),
    maxHp: finiteOrNull(entity.maxHp),
    state,
    activity: targetId != null ? "engaging" : state,
    targetId,
    orderPlan: Array.isArray(entity.orderPlan) ? entity.orderPlan.slice(0, 8).map((stage) => ({
      kind: typeof stage?.kind === "string" ? stage.kind : "",
      x: finiteOrNull(stage?.x),
      y: finiteOrNull(stage?.y),
      target: Number.isInteger(stage?.target) ? stage.target : null,
    })) : [],
  };
}

function isInspectableEntity(entity) {
  return !!entity && entity.shotReveal !== true && entity.visionOnly !== true;
}

function isControllableUnit(entity, playerId) {
  return isInspectableEntity(entity) && entity.owner === playerId && isUnit(entity.kind);
}

function projectLocalPlayer(state) {
  const player = state.localPlayer || {};
  return {
    ...projectPlayer(player),
    resources: {
      steel: finiteOrNull(state.resources?.steel),
      oil: finiteOrNull(state.resources?.oil),
      supplyUsed: finiteOrNull(state.resources?.supplyUsed),
      supplyCap: finiteOrNull(state.resources?.supplyCap),
    },
  };
}

function projectPlayer(player) {
  return {
    id: player?.id ?? null,
    teamId: player?.teamId ?? null,
    factionId: player?.factionId || "",
    name: player?.name || "",
    color: player?.color || "",
    isAi: player?.isAi === true,
  };
}

function projectMap(map) {
  return { name: map?.name || "", width: finiteOrNull(map?.width), height: finiteOrNull(map?.height), tileSize: finiteOrNull(map?.tileSize) };
}

function projectRoomTime(state) {
  if (!state) return null;
  return {
    currentTick: finiteOrNull(state.currentTick),
    speed: finiteOrNull(state.speed),
    paused: state.paused === true,
    ended: state.ended === true,
  };
}

function projectUi() {
  const gameOver = element("game-over");
  const commandCard = element("command-card");
  return {
    gameVisible: visible(element("game-screen")),
    hudVisible: visible(element("hud")),
    resources: {
      steel: text("res-steel"),
      oil: text("res-oil"),
      supply: text("res-supply"),
    },
    timer: text("game-timer"),
    idleWorkers: text("idle-workers-count"),
    selection: text("selected-panel", 512),
    commandCard: commandCard
      ? [...commandCard.querySelectorAll("button")].slice(0, 24).map((button) => String(button.textContent || "").trim().slice(0, 80)).filter(Boolean)
      : [],
    giveUpDialogVisible: visible(element("give-up-confirm")),
    scoreScreenVisible: visible(gameOver),
    scoreTitle: text("game-over-text"),
    scoreSummary: text("game-over-scores", 1024),
  };
}

function gamePhase(match) {
  if (!match) return "loading";
  if (visible(element("game-over"))) return "concluded";
  return match.giveUpSent ? "givingUp" : "active";
}

function projectCamera(camera) {
  const snapshot = camera?.snapshot?.();
  return snapshot?.version === 1 ? snapshot : null;
}

function projectCameraViewport(camera) {
  const viewport = camera?.projectionSnapshot?.()?.viewport;
  return Number.isFinite(viewport?.widthCssPx) && Number.isFinite(viewport?.heightCssPx) ? viewport : null;
}

function projectCameraWorldBounds(camera) {
  const bounds = camera?.viewportGroundBounds?.();
  return Number.isFinite(bounds?.minX) && Number.isFinite(bounds?.minY) && Number.isFinite(bounds?.maxX) && Number.isFinite(bounds?.maxY) ? bounds : null;
}

function projectViewport() {
  const rect = element("viewport")?.getBoundingClientRect?.();
  return {
    x: finiteOrNull(rect?.x), y: finiteOrNull(rect?.y), width: finiteOrNull(rect?.width), height: finiteOrNull(rect?.height),
    devicePixelRatio: finiteOrNull(globalThis.devicePixelRatio),
  };
}

function documentFontsStatus() {
  const fonts = typeof document !== "undefined" ? document.fonts : null;
  return fonts ? { status: fonts.status === "loaded" ? "ready" : "pending", supported: true } : { status: "ready", supported: false };
}

function element(id) {
  return typeof document !== "undefined" ? document.getElementById(id) : null;
}

function visible(node) {
  return !!node && node.hidden !== true;
}

function text(id, maximum = 128) {
  return String(element(id)?.textContent || "").replace(/\s+/g, " ").trim().slice(0, maximum);
}

function boundedIds(values, label, maximum) {
  if (!Array.isArray(values) || values.length < 1 || values.length > maximum) throw bridgeError("invalidInput", `${label} must contain 1-${maximum} ids.`);
  const ids = values.map((value) => boundedInteger(value, label, 1, 0xffffffff));
  if (new Set(ids).size !== ids.length) throw bridgeError("invalidInput", `${label} must not contain duplicate ids.`);
  return ids;
}

function optionalBoundedIds(values, label, maximum) {
  if (values == null) return [];
  if (!Array.isArray(values) || values.length > maximum) throw bridgeError("invalidInput", `${label} must contain at most ${maximum} ids.`);
  const ids = values.map((value) => boundedInteger(value, label, 1, 0xffffffff));
  if (new Set(ids).size !== ids.length) throw bridgeError("invalidInput", `${label} must not contain duplicate ids.`);
  return ids;
}

function finiteNumber(value, label) {
  const number = Number(value);
  if (!Number.isFinite(number)) throw bridgeError("invalidInput", `${label} must be finite.`);
  return number;
}

function boundedNumber(value, label, minimum, maximum) {
  const number = finiteNumber(value, label);
  if (number < minimum || number > maximum) throw bridgeError("invalidInput", `${label} must be from ${minimum} to ${maximum}.`);
  return number;
}

function boundedInteger(value, label, minimum, maximum) {
  const number = Number(value);
  if (!Number.isInteger(number) || number < minimum || number > maximum) throw bridgeError("invalidInput", `${label} must be an integer from ${minimum} to ${maximum}.`);
  return number;
}

function safeKind(value, label) {
  const kind = String(value || "");
  if (!/^[A-Za-z0-9_]{1,64}$/.test(kind)) throw bridgeError("invalidInput", `${label} contains an invalid kind token.`);
  return kind;
}

function snapshotSequence(match) {
  return Number(match?.state?.currRecvTime) || 0;
}

function finiteOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function bridgeError(code, message, details = undefined) {
  return Object.assign(new Error(message), { code, ...(details ? { details } : {}) });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function animationFrames(count) {
  return new Promise((resolve) => {
    const step = () => count-- <= 0 ? resolve() : requestAnimationFrame(step);
    step();
  });
}

// Narrow, launch-gated page bridge for the local Agent Lab driver.
// It deliberately exposes typed scene operations only; callers never receive App,
// Match, transport, renderer, or GameState references.

import { ABILITY, CMD, LAB_ROLE } from "./protocol.js";
import { factionCatalog } from "./config.js";
import {
  labBuildingSpawnFactionOptions,
  labSpawnBuildingKindsForFaction,
  labSpawnFactionOptions,
  labSpawnUnitKindsForFaction,
} from "./lab_spawn_catalog.js";

export const AGENT_LAB_BRIDGE_KEY = "__rtsAgentLab";
export const AGENT_LAB_BRIDGE_VERSION = 1;
export const AGENT_LAB_LIMITS = Object.freeze({
  inspectEntities: 100,
  inspectPlayers: 16,
  removeEntities: 100,
  focusEntities: 20,
  stepTicks: 100,
  seekTick: 1_000_000,
  waitMs: 8_000,
});

export function agentLabLaunchEnabled(locationLike = globalThis.location) {
  try {
    const pathname = locationLike?.pathname || "";
    return (pathname === "/lab" || pathname === "/lab/") &&
      new URLSearchParams(locationLike?.search || "").get("agentLab") === "1";
  } catch {
    return false;
  }
}

export class AgentLabBridge {
  constructor({ app, windowLike = globalThis.window, enabled = agentLabLaunchEnabled(), sleep = delay } = {}) {
    this.app = app;
    this.windowLike = windowLike;
    this.enabled = !!enabled;
    this.sleep = sleep;
    this.destroyed = false;
    this.surface = Object.freeze({
      version: AGENT_LAB_BRIDGE_VERSION,
      status: () => this.status(),
      call: (method, input) => this.call(method, input),
    });
    if (this.enabled && this.windowLike) this.windowLike[AGENT_LAB_BRIDGE_KEY] = this.surface;
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
      version: AGENT_LAB_BRIDGE_VERSION,
      enabled: this.enabled && !this.destroyed,
      ready: reason === "ready",
      reason,
      websocketConnected,
      startReceived,
      labRole: labClient?.state?.role || "",
      room: labClient?.state?.room || "",
      snapshotTick: snapshotApplied ? match.state.tick : null,
      roomTime: projectRoomTime(roomTime),
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
          message: error?.message || "Agent Lab bridge request failed.",
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
      case "camera": return this.camera(input);
      case "reset": return this.reset();
      default: throw bridgeError("unknownMethod", `Unknown Agent Lab bridge method ${JSON.stringify(method)}.`);
    }
  }

  session() {
    const status = this.status();
    if (!status.ready) throw bridgeError(status.reason, `Agent Lab is not ready: ${status.reason}.`);
    return { match: this.app.match, labClient: this.app.labClient };
  }

  catalog() {
    const { match } = this.session();
    const players = match.state.players.slice(0, AGENT_LAB_LIMITS.inspectPlayers).map(projectPlayer);
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

  async spawn(spec) {
    const { labClient } = this.session();
    const owner = positiveInt(spec?.owner, "spawn.owner");
    const kind = safeKind(spec?.kind, "spawn.kind");
    const x = finiteNumber(spec?.x, "spawn.x");
    const y = finiteNumber(spec?.y, "spawn.y");
    const result = await this.mutate(
      () => labClient.spawnEntity({ owner, kind, x, y, completed: spec?.completed !== false }),
      (outcome) => this.entityPresent(outcome?.entityId),
    );
    return { result: projectLabResult(result), entity: projectEntity(this.app.match.state.entityById(result.outcome.entityId)) };
  }

  async update(input) {
    const { labClient } = this.session();
    const operation = String(input?.operation || "");
    if (operation === "move") {
      const entityId = positiveInt(input?.entityId, "update.entityId");
      const x = finiteNumber(input?.x, "update.x");
      const y = finiteNumber(input?.y, "update.y");
      const result = await this.mutate(
        () => labClient.moveEntity(entityId, x, y),
        () => this.entityAt(entityId, x, y),
      );
      return { result: projectLabResult(result) };
    }
    if (operation === "reassign") {
      const entityId = positiveInt(input?.entityId, "update.entityId");
      const owner = positiveInt(input?.owner, "update.owner");
      const result = await this.mutate(
        () => labClient.setEntityOwner(entityId, owner),
        () => this.app.match.state.entityById(entityId)?.owner === owner,
      );
      return { result: projectLabResult(result) };
    }
    if (operation === "resources") {
      const playerId = positiveInt(input?.playerId, "update.playerId");
      const steel = nonNegativeInt(input?.steel, "update.steel");
      const oil = nonNegativeInt(input?.oil, "update.oil");
      const result = await this.mutate(
        () => labClient.setPlayerResources(playerId, steel, oil),
        () => this.playerResourcesMatch(playerId, steel, oil),
      );
      return { result: projectLabResult(result) };
    }
    if (operation === "research") {
      const playerId = positiveInt(input?.playerId, "update.playerId");
      const upgrade = safeKind(input?.upgrade, "update.upgrade");
      const completed = input?.completed !== false;
      const result = await this.mutate(
        () => labClient.setCompletedResearch(playerId, upgrade, completed),
        () => true,
      );
      return { result: projectLabResult(result) };
    }
    if (operation === "godMode") {
      const playerId = positiveInt(input?.playerId, "update.playerId");
      const enabled = input?.enabled !== false;
      const result = await this.mutate(
        () => labClient.setPlayerGodMode(playerId, enabled),
        () => this.app.labClient.state?.godModePlayers?.includes(playerId) === enabled,
      );
      return { result: projectLabResult(result) };
    }
    throw bridgeError("invalidUpdate", "update.operation must be move, reassign, resources, research, or godMode.");
  }

  async remove(input) {
    const { labClient } = this.session();
    const ids = boundedIds(input?.entityIds, "remove.entityIds", AGENT_LAB_LIMITS.removeEntities);
    const results = [];
    for (const entityId of ids) {
      const result = await this.mutate(
        () => labClient.deleteEntity(entityId),
        () => !this.app.match.state.entityById(entityId),
      );
      results.push(projectLabResult(result));
    }
    return { results };
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
      const ticks = boundedPositiveInt(input?.ticks ?? 1, "time.ticks", AGENT_LAB_LIMITS.stepTicks);
      for (let index = 0; index < ticks; index += 1) {
        const previous = snapshotSequence(match);
        match.net.stepRoomTime();
        await this.waitFor(() => snapshotSequence(match) > previous, "room time step");
      }
    } else if (action === "seek") {
      const tick = boundedNonNegativeInt(input?.tick, "time.tick", AGENT_LAB_LIMITS.seekTick);
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
      players: match.state.players.slice(0, AGENT_LAB_LIMITS.inspectPlayers).map(projectPlayer),
      room: {
        tick: match.state.tick,
        roomTime: projectRoomTime(match.roomTimeControls?.roomTimeState),
        map: projectMap(match.state.map),
      },
      camera: projectCamera(match.camera),
    };
  }

  camera(input) {
    const { match } = this.session();
    const action = String(input?.action || "");
    if (action === "set") {
      match.camera.setView({
        x: optionalFiniteNumber(input?.x),
        y: optionalFiniteNumber(input?.y),
        zoom: optionalFiniteNumber(input?.zoom),
        centerX: optionalFiniteNumber(input?.centerX),
        centerY: optionalFiniteNumber(input?.centerY),
      });
    } else if (action === "focus") {
      const ids = boundedIds(input?.entityIds, "camera.entityIds", AGENT_LAB_LIMITS.focusEntities);
      const entities = ids.map((id) => match.state.entityById(id)).filter(Boolean);
      if (entities.length !== ids.length) throw bridgeError("unknownEntity", "camera.focus contains an entity that is not in the current snapshot.");
      const padding = boundedNonNegativeNumber(input?.padding ?? 48, "camera.padding", 1024);
      const minX = Math.min(...entities.map((entity) => entity.x));
      const maxX = Math.max(...entities.map((entity) => entity.x));
      const minY = Math.min(...entities.map((entity) => entity.y));
      const maxY = Math.max(...entities.map((entity) => entity.y));
      const width = Math.max(1, maxX - minX + padding * 2);
      const height = Math.max(1, maxY - minY + padding * 2);
      const zoom = Math.min(match.camera.viewW / width, match.camera.viewH / height);
      if (Number.isFinite(zoom) && zoom > 0) match.camera.setZoom(zoom);
      const centerX = (minX + maxX) / 2;
      const centerY = (minY + maxY) / 2;
      match.camera.centerOn(centerX, centerY);
    } else {
      throw bridgeError("invalidCamera", "camera.action must be set or focus.");
    }
    return { camera: projectCamera(match.camera) };
  }

  reset() {
    return this.time({ action: "seek", tick: 0 });
  }

  async mutate(send, observed) {
    const { match } = this.session();
    const before = snapshotSequence(match);
    const result = await send();
    if (!result?.ok) throw bridgeError("labRejected", result?.error || "The server rejected the lab operation.");
    // Paused rooms do not naturally produce a new snapshot. Advance one authoritative
    // tick after an accepted setup/command operation so success always carries observed state.
    if (isPaused(match)) match.net.stepRoomTime();
    await this.waitFor(
      () => snapshotSequence(match) > before && observed(result.outcome || null),
      `authoritative snapshot for ${result.op || "lab operation"}`,
    );
    return { ...result, snapshotTick: match.state.tick };
  }

  entityPresent(entityId) {
    return Number.isInteger(entityId) && !!this.app.match.state.entityById(entityId);
  }

  entityAt(entityId, x, y) {
    const entity = this.app.match.state.entityById(entityId);
    return !!entity && Math.abs(entity.x - x) < 0.01 && Math.abs(entity.y - y) < 0.01;
  }

  playerResourcesMatch(playerId, steel, oil) {
    const row = this.app.match.state.playerResources.find((player) => Number(player?.id) === playerId);
    return row?.steel === steel && row?.oil === oil;
  }

  async waitFor(predicate, detail, timeoutMs = AGENT_LAB_LIMITS.waitMs) {
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
    if (this.windowLike?.[AGENT_LAB_BRIDGE_KEY] === this.surface) delete this.windowLike[AGENT_LAB_BRIDGE_KEY];
  }
}

export function normalizeInspectionQuery(query = {}) {
  const ids = optionalBoundedIds(query.ids, AGENT_LAB_LIMITS.inspectEntities);
  const owners = optionalBoundedIds(query.owners, AGENT_LAB_LIMITS.inspectPlayers);
  const kinds = Array.isArray(query.kinds)
    ? [...new Set(query.kinds.filter((kind) => typeof kind === "string" && kind.length > 0 && kind.length <= 64))]
    : [];
  return {
    ids: new Set(ids),
    owners: new Set(owners),
    kinds: new Set(kinds),
    cameraViewport: query.cameraViewport === true,
    limit: boundedPositiveInt(query.limit ?? 25, "inspect.limit", AGENT_LAB_LIMITS.inspectEntities),
  };
}

function inspectionIncludesEntity(entity, query, camera) {
  if (query.ids.size > 0 && !query.ids.has(entity.id)) return false;
  if (query.owners.size > 0 && !query.owners.has(entity.owner)) return false;
  if (query.kinds.size > 0 && !query.kinds.has(entity.kind)) return false;
  if (query.cameraViewport && !entityInCameraViewport(entity, camera)) return false;
  return true;
}

function entityInCameraViewport(entity, camera) {
  const screen = camera?.worldToScreen?.(entity.x, entity.y);
  return Number.isFinite(screen?.x) && Number.isFinite(screen?.y) &&
    screen.x >= 0 && screen.x <= camera.viewW && screen.y >= 0 && screen.y <= camera.viewH;
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
  return {
    id: entity.id,
    kind: entity.kind,
    owner: entity.owner,
    x: finiteOrNull(entity.x),
    y: finiteOrNull(entity.y),
    hp: finiteOrNull(entity.hp),
    maxHp: finiteOrNull(entity.maxHp),
    state: typeof entity.state === "string" ? entity.state : "",
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
  const topLeft = camera?.screenToWorld?.(0, 0);
  const bottomRight = camera?.screenToWorld?.(camera?.viewW, camera?.viewH);
  return {
    x: finiteOrNull(camera?.x),
    y: finiteOrNull(camera?.y),
    zoom: finiteOrNull(camera?.zoom),
    worldBounds: Number.isFinite(topLeft?.x) && Number.isFinite(topLeft?.y) &&
      Number.isFinite(bottomRight?.x) && Number.isFinite(bottomRight?.y)
      ? {
        minX: Math.min(topLeft.x, bottomRight.x),
        minY: Math.min(topLeft.y, bottomRight.y),
        maxX: Math.max(topLeft.x, bottomRight.x),
        maxY: Math.max(topLeft.y, bottomRight.y),
      }
      : null,
  };
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

function projectLabResult(result) {
  return {
    op: result.op || "",
    outcome: result.outcome || null,
    snapshotTick: finiteOrNull(result.snapshotTick),
  };
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

function optionalBoundedIds(values, maximum) {
  if (values == null) return [];
  if (!Array.isArray(values) || values.length > maximum) return [];
  return [...new Set(values.filter((value) => Number.isInteger(Number(value)) && Number(value) > 0).map(Number))];
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

function optionalFiniteNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : undefined;
}

function finiteOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function bridgeError(code, message) {
  const error = new Error(message);
  error.code = code;
  return error;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

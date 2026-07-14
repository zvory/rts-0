import { FIXED_CAPTURE_LIMITS } from "./fixed_capture.ts";
import { GAME_TIMELAPSE_LIMITS, timelapseFrameBound } from "./game_timelapse.ts";
import { RECORDING_LIMITS } from "./recording.ts";

export const INTERACT_LIMITS = Object.freeze({
  maxSessions: 1,
  maxSpawnBatch: 400,
  maxMutationBatch: 400,
  maxAliases: 400,
  maxCommandUnits: 100,
  maxInspectRefs: 400,
  maxInspectResults: 400,
  maxFocusRefs: 400,
  maxScreenshotSubjects: 400,
  maxArtifactBytes: 8 * 1024 * 1024,
  maxAliasSidecarBytes: 64 * 1024,
  maxResponseDetails: 12,
  maxRecordingOperations: RECORDING_LIMITS.maxOperations,
  defaultRecordingDurationMs: RECORDING_LIMITS.defaultDurationMs,
  maxRecordingDurationMs: RECORDING_LIMITS.maxDurationMs,
  maxFixedCaptureFrames: FIXED_CAPTURE_LIMITS.maxFrames,
});

export const ALL_CATALOG_CATEGORIES = Object.freeze([
  "maps", "players", "factions", "units", "buildings", "upgrades", "commands", "abilities",
]);
export const ALIAS_RE = /^[A-Za-z][A-Za-z0-9_-]{0,31}$/;

const TOKEN_RE = /^[A-Za-z0-9_]{1,64}$/;
const SESSION_RE = /^(?:lab|game)_[a-f0-9]{32}$/;
const U32_MAX = 0xffff_ffff;
const COMMAND_FIELDS = Object.freeze({
  move: ["c", "units", "x", "y", "queued"],
  attackMove: ["c", "units", "x", "y", "queued"],
  attack: ["c", "units", "target", "queued"],
  deconstruct: ["c", "units", "target", "queued"],
  setupAntiTankGuns: ["c", "units", "x", "y", "queued"],
  tearDownAntiTankGuns: ["c", "units"],
  charge: ["c", "units"],
  useAbility: ["c", "ability", "units", "x", "y", "queued"],
  recastAbility: ["c", "ability", "units", "targetObjectId", "queued"],
  setAutocast: ["c", "ability", "units", "enabled"],
  gather: ["c", "units", "node", "queued"],
  build: ["c", "units", "building", "tileX", "tileY", "queued"],
  train: ["c", "building", "unit"],
  adjustProductionRepeat: ["c", "buildings", "unit", "delta"],
  research: ["c", "building", "upgrade"],
  cancel: ["c", "building"],
  stop: ["c", "units"],
  holdPosition: ["c", "units"],
  setRally: ["c", "building", "x", "y", "kind", "queued"],
});

export type CommandInput = Record<string, unknown>;

export function validatorFor(command: string): (value: unknown) => CommandInput {
  return (value: unknown) => validateCommandInput(command, value);
}

export function validateCommandInput(command: string, value: unknown): CommandInput {
  record(value, "input");
  const session = () => sessionId(value.sessionId);
  if (command.startsWith("game-")) return validateGameNamespaceInput(command, value, session);
  if (command === "shutdown") return exact(value, [], "shutdown");
  if (command === "status") {
    exact(value, ["sessionId"], "status");
    if (value.sessionId != null) session();
    return value;
  }
  if (command === "open") {
    exact(value, ["workspaceRoot", "map", "seed", "scenario", "renderer", "viewport"], "open");
    if (value.workspaceRoot != null && (typeof value.workspaceRoot !== "string" || !value.workspaceRoot)) invalid("open.workspaceRoot", "must be a non-empty string");
    if (value.map != null) token(value.map, "open.map", 48);
    if (value.scenario != null) token(value.scenario, "open.scenario", 48);
    if (value.renderer != null && (typeof value.renderer !== "string" || !["pixi", "babylon"].includes(value.renderer))) invalid("open.renderer", "must be pixi or babylon");
    if (value.seed != null && !((typeof value.seed === "string" && value.seed.length <= 64) || isInteger(value.seed, 0, U32_MAX))) invalid("open.seed", "must be a bounded string or unsigned integer");
    if (value.viewport != null) viewport(value.viewport, 4096, "open.viewport");
    return value;
  }
  session();
  if (command === "close" || command === "reset") return exact(value, ["sessionId"], command);
  if (command === "catalog") {
    exact(value, ["sessionId", "categories"], command);
    if (value.categories != null) array(value.categories, "catalog.categories", 0, ALL_CATALOG_CATEGORIES.length, (entry) => {
      if (typeof entry !== "string" || !ALL_CATALOG_CATEGORIES.includes(entry)) invalid("catalog.categories", "contains an unknown category");
    });
  } else if (command === "spawn") {
    exact(value, ["sessionId", "spawns", "details"], command);
    optionalBoolean(value.details, "spawn.details");
    array(value.spawns, "spawn.spawns", 1, INTERACT_LIMITS.maxSpawnBatch, (spec, index) => {
      record(spec, `spawn.spawns[${index}]`);
      exact(spec, ["owner", "kind", "x", "y", "completed", "alias"], "spawn spec");
      u32(spec.owner, "spawn.owner");
      token(spec.kind, "spawn.kind");
      finite(spec.x, "spawn.x");
      finite(spec.y, "spawn.y");
      optionalBoolean(spec.completed, "spawn.completed");
      if (spec.alias != null) alias(spec.alias);
    });
  } else if (command === "update") {
    exact(value, ["sessionId", "update", "updates"], command);
    if ((value.update == null) === (value.updates == null)) invalid("update", "requires exactly one of update or updates");
    if (value.update != null) validateUpdate(value.update);
    if (value.updates != null) array(value.updates, "update.updates", 1, INTERACT_LIMITS.maxMutationBatch, validateUpdate);
  } else if (command === "remove") {
    exact(value, ["sessionId", "refs"], command);
    refs(value.refs, "remove.refs", 1, INTERACT_LIMITS.maxMutationBatch);
  } else if (command === "order") {
    exact(value, ["sessionId", "playerId", "command", "ignoreCommandLimits"], command);
    u32(value.playerId, "order.playerId");
    optionalBoolean(value.ignoreCommandLimits, "order.ignoreCommandLimits");
    validateGameCommand(value.command);
  } else if (command === "time") {
    exact(value, ["sessionId", "control"], command);
    validateTime(value.control);
  } else if (command === "inspect") {
    exact(value, ["sessionId", "refs", "kinds", "owners", "cameraViewport", "limit"], command);
    if (value.refs != null) refs(value.refs, "inspect.refs", 0, INTERACT_LIMITS.maxInspectRefs);
    if (value.kinds != null) array(value.kinds, "inspect.kinds", 0, 32, (entry: unknown) => token(entry, "inspect.kind"));
    if (value.owners != null) array(value.owners, "inspect.owners", 0, 16, (entry: unknown) => u32(entry, "inspect.owner"));
    optionalBoolean(value.cameraViewport, "inspect.cameraViewport");
    if (value.limit != null) integer(value.limit, "inspect.limit", 1, INTERACT_LIMITS.maxInspectResults);
  } else if (command === "camera") {
    exact(value, ["sessionId", "camera"], command);
    validateCamera(value.camera);
  } else if (command === "screenshot") {
    exact(value, ["sessionId", "name", "presentation", "viewport", "subjects"], command);
    if (value.name != null && (typeof value.name !== "string" || !/^[A-Za-z0-9_-]{1,48}$/.test(value.name))) invalid("screenshot.name", "must be a safe artifact token");
    if (value.presentation != null && (typeof value.presentation !== "string" || !["clean", "normal"].includes(value.presentation))) invalid("screenshot.presentation", "must be clean or normal");
    if (value.viewport != null) viewport(value.viewport, 2048, "screenshot.viewport");
    if (value.subjects != null) refs(value.subjects, "screenshot.subjects", 0, INTERACT_LIMITS.maxScreenshotSubjects);
  } else if (command === "export") {
    exact(value, ["sessionId", "kind", "name", "reproduction"], command);
    artifactKind(value.kind, "export.kind");
    const maxNameBytes = value.kind === "setup" ? 80 : 120;
    if (value.name != null && (typeof value.name !== "string" || Buffer.byteLength(value.name) > maxNameBytes)) invalid("export.name", `must be at most ${maxNameBytes} UTF-8 bytes`);
    optionalBoolean(value.reproduction, "export.reproduction");
  } else if (command === "import") {
    exact(value, ["sessionId", "kind", "artifactId", "path", "details"], command);
    artifactKind(value.kind, "import.kind");
    artifactSelector(value, "import");
    optionalBoolean(value.details, "import.details");
  } else if (command === "artifact-inspect") {
    exact(value, ["sessionId", "kind", "artifactId", "path"], command);
    if (value.kind != null) artifactKind(value.kind, "artifact-inspect.kind");
    artifactSelector(value, "artifact-inspect");
  } else if (command === "record-start") {
    exact(value, ["sessionId", "name", "maxDurationMs", "viewport", "crop", "scale", "resumeSpeed"], command);
    if (value.name != null && (typeof value.name !== "string" || !/^[A-Za-z0-9_-]{1,48}$/.test(value.name))) invalid("record-start.name", "must be a safe artifact token");
    if (value.maxDurationMs != null) integer(value.maxDurationMs, "record-start.maxDurationMs", 1_000, INTERACT_LIMITS.maxRecordingDurationMs);
    if (value.viewport != null) viewport(value.viewport, 2048, "record-start.viewport");
    if (value.crop != null) recordingCrop(value.crop);
    if (value.scale != null) boundedNumber(value.scale, "record-start.scale", 0.25, 1);
    if (value.resumeSpeed != null) boundedNumber(value.resumeSpeed, "record-start.resumeSpeed", 0.01, 16);
  } else if (command === "record-stop" || command === "record-wait") {
    exact(value, ["sessionId"], command);
  } else if (command === "capture-fixed") {
    exact(value, ["sessionId", "name", "fps", "frameCount", "viewport"], command);
    if (value.name != null && (typeof value.name !== "string" || !/^[A-Za-z0-9_-]{1,48}$/.test(value.name))) invalid("capture-fixed.name", "must be a safe artifact token");
    if (value.fps != null) integer(value.fps, "capture-fixed.fps", FIXED_CAPTURE_LIMITS.minFps, FIXED_CAPTURE_LIMITS.maxFps);
    if (value.frameCount != null) integer(value.frameCount, "capture-fixed.frameCount", 1, FIXED_CAPTURE_LIMITS.maxFrames);
    if (value.viewport != null) viewport(value.viewport, 2048, "capture-fixed.viewport");
  } else if (command === "capture-cancel") {
    exact(value, ["sessionId"], command);
  } else {
    throw Object.assign(new Error(`Unknown command ${JSON.stringify(command)}.`), { code: "unknownCommand" });
  }
  return value;
}

function validateGameNamespaceInput(command: string, value: CommandInput, session: () => void): CommandInput {
  if (command === "game-open") {
    exact(value, ["workspaceRoot", "map", "opponent", "spectate", "renderer", "viewport"], "game open");
    if (value.workspaceRoot != null && (typeof value.workspaceRoot !== "string" || !value.workspaceRoot)) invalid("game open.workspaceRoot", "must be a non-empty string");
    if (value.map != null) displayName(value.map, "game open.map", 64);
    if (value.opponent != null && !["ai_2_1", "ai_turtle"].includes(String(value.opponent))) invalid("game open.opponent", "must be ai_2_1 or ai_turtle");
    if (value.spectate != null) {
      array(value.spectate, "game open.spectate", 2, 2, (entry: unknown) => {
        if (!["ai_2_1", "ai_turtle"].includes(String(entry))) invalid("game open.spectate", "must contain ai_2_1 or ai_turtle profiles");
      });
      if (value.opponent != null) invalid("game open", "cannot combine opponent with spectate");
    }
    if (value.renderer != null && !["pixi", "babylon"].includes(String(value.renderer))) invalid("game open.renderer", "must be pixi or babylon");
    if (value.viewport != null) viewport(value.viewport, 4096, "game open.viewport");
    return value;
  }
  session();
  if (command === "game-inspect") {
    exact(value, ["sessionId", "ids", "kinds", "ownership", "cameraViewport", "limit"], "game inspect");
    if (value.ids != null) idArray(value.ids, "game inspect.ids", 0, INTERACT_LIMITS.maxInspectRefs);
    if (value.kinds != null) array(value.kinds, "game inspect.kinds", 0, 32, (entry: unknown) => token(entry, "game inspect.kind"));
    if (value.ownership != null && !["owned", "visible"].includes(String(value.ownership))) invalid("game inspect.ownership", "must be owned or visible");
    optionalBoolean(value.cameraViewport, "game inspect.cameraViewport");
    if (value.limit != null) integer(value.limit, "game inspect.limit", 1, INTERACT_LIMITS.maxInspectResults);
  } else if (command === "game-move") {
    exact(value, ["sessionId", "units", "x", "y", "queued"], "game move");
    idArray(value.units, "game move.units", 1, INTERACT_LIMITS.maxCommandUnits);
    finite(value.x, "game move.x");
    finite(value.y, "game move.y");
    optionalBoolean(value.queued, "game move.queued");
  } else if (command === "game-give-up") {
    exact(value, ["sessionId"], "game give-up");
  } else if (command === "game-camera") {
    exact(value, ["sessionId", "camera"], "game camera");
    validateGameCamera(value.camera);
  } else if (command === "game-screenshot") {
    exact(value, ["sessionId", "name", "presentation", "viewport", "region", "subjects"], "game screenshot");
    artifactToken(value.name, "game screenshot.name");
    presentation(value.presentation, "game screenshot.presentation");
    if (value.viewport != null) viewport(value.viewport, 2048, "game screenshot.viewport");
    if (value.region != null) captureRegion(value.region, "game screenshot.region");
    if (value.subjects != null) idArray(value.subjects, "game screenshot.subjects", 0, INTERACT_LIMITS.maxScreenshotSubjects);
  } else if (command === "game-record-start") {
    exact(value, ["sessionId", "name", "maxDurationMs", "viewport", "crop", "region", "scale", "presentation"], "game record-start");
    artifactToken(value.name, "game record-start.name");
    if (value.maxDurationMs != null) integer(value.maxDurationMs, "game record-start.maxDurationMs", 1_000, INTERACT_LIMITS.maxRecordingDurationMs);
    if (value.viewport != null) viewport(value.viewport, 2048, "game record-start.viewport");
    if (value.crop != null) recordingCrop(value.crop);
    if (value.region != null) captureRegion(value.region, "game record-start.region");
    if (value.crop != null && value.region != null) invalid("game record-start", "cannot combine crop with region");
    if (value.scale != null) boundedNumber(value.scale, "game record-start.scale", 0.25, 1);
    presentation(value.presentation, "game record-start.presentation");
  } else if (command === "game-capture-timelapse") {
    exact(value, ["sessionId", "name", "maxDurationMs", "sampleEveryMs", "fps", "speed", "viewport", "region", "presentation"], "game capture-timelapse");
    artifactToken(value.name, "game capture-timelapse.name");
    const duration = value.maxDurationMs == null ? GAME_TIMELAPSE_LIMITS.defaultDurationMs : integer(value.maxDurationMs, "game capture-timelapse.maxDurationMs", 1_000, GAME_TIMELAPSE_LIMITS.maxDurationMs);
    const sampleEvery = value.sampleEveryMs == null ? GAME_TIMELAPSE_LIMITS.defaultSampleEveryMs : integer(value.sampleEveryMs, "game capture-timelapse.sampleEveryMs", GAME_TIMELAPSE_LIMITS.minSampleEveryMs, GAME_TIMELAPSE_LIMITS.maxSampleEveryMs);
    if (timelapseFrameBound(duration, sampleEvery) > GAME_TIMELAPSE_LIMITS.maxFrames) invalid("game capture-timelapse", `may capture at most ${GAME_TIMELAPSE_LIMITS.maxFrames} sampled frames`);
    if (value.fps != null) integer(value.fps, "game capture-timelapse.fps", GAME_TIMELAPSE_LIMITS.minFps, GAME_TIMELAPSE_LIMITS.maxFps);
    if (value.speed != null) boundedNumber(value.speed, "game capture-timelapse.speed", GAME_TIMELAPSE_LIMITS.minSpeed, GAME_TIMELAPSE_LIMITS.maxSpeed);
    if (value.viewport != null) viewport(value.viewport, 2048, "game capture-timelapse.viewport");
    if (value.region != null) captureRegion(value.region, "game capture-timelapse.region");
    presentation(value.presentation, "game capture-timelapse.presentation");
  } else {
    throw Object.assign(new Error(`Unknown command ${JSON.stringify(command)}.`), { code: "unknownCommand" });
  }
  return value;
}

function validateGameCamera(value: unknown) {
  record(value, "game camera.camera");
  if (value.action === "focus") {
    exact(value, ["action", "entities", "padding"], "game camera");
    idArray(value.entities, "game camera.entities", 1, INTERACT_LIMITS.maxFocusRefs);
    if (value.padding != null) boundedNumber(value.padding, "game camera.padding", 0, 1024);
  } else if (value.action === "overview") {
    exact(value, ["action", "padding"], "game camera");
    if (value.padding != null) boundedNumber(value.padding, "game camera.padding", 0, 1024);
  } else if (value.action === "set") {
    validateCamera(value);
  } else invalid("game camera.action", "is unsupported");
}

function validateUpdate(value: unknown) {
  record(value, "update");
  const operation = value.operation;
  const fieldsByOperation: Record<string, readonly string[]> = {
    move: ["operation", "entity", "x", "y"], owner: ["operation", "entity", "owner"],
    resources: ["operation", "playerId", "steel", "oil"], research: ["operation", "playerId", "upgrade", "completed"],
    godMode: ["operation", "playerId", "enabled"],
  };
  const allowed = typeof operation === "string" ? fieldsByOperation[operation] : undefined;
  if (!allowed || typeof operation !== "string") invalid("update.operation", "is unsupported");
  exact(value, allowed, "update");
  if (["move", "owner"].includes(operation)) entityRef(value.entity, "update.entity");
  if (operation === "move") { finite(value.x, "update.x"); finite(value.y, "update.y"); }
  if (operation === "owner") u32(value.owner, "update.owner");
  if (["resources", "research", "godMode"].includes(operation)) u32(value.playerId, "update.playerId");
  if (operation === "resources") { integer(value.steel, "update.steel", 0, U32_MAX); integer(value.oil, "update.oil", 0, U32_MAX); }
  if (operation === "research") { token(value.upgrade, "update.upgrade"); optionalBoolean(value.completed, "update.completed"); }
  if (operation === "godMode") optionalBoolean(value.enabled, "update.enabled");
}

function validateTime(value: unknown) {
  record(value, "time.control");
  const fieldsByAction: Record<string, readonly string[]> = { pause: ["action"], resume: ["action", "speed"], speed: ["action", "speed"], step: ["action", "ticks"], seek: ["action", "tick"] };
  const allowed = typeof value.action === "string" ? fieldsByAction[value.action] : undefined;
  if (!allowed) invalid("time.action", "is unsupported");
  exact(value, allowed, "time.control");
  if (value.action === "resume" && value.speed != null) boundedNumber(value.speed, "time.speed", 0.01, 16);
  if (value.action === "speed") boundedNumber(value.speed, "time.speed", 0, 16);
  if (value.action === "step" && value.ticks != null) integer(value.ticks, "time.ticks", 1, 100);
  if (value.action === "seek") integer(value.tick, "time.tick", 0, 1_000_000);
}

function validateCamera(value: unknown) {
  record(value, "camera.camera");
  if (value.action === "focus") {
    exact(value, ["action", "refs", "padding"], "camera");
    refs(value.refs, "camera.refs", 1, INTERACT_LIMITS.maxFocusRefs);
    if (value.padding != null) boundedNumber(value.padding, "camera.padding", 0, 1024);
  } else if (value.action === "set") {
    exact(value, ["action", "snapshot"], "camera");
    record(value.snapshot, "camera.snapshot");
    exact(value.snapshot, ["version", "focus", "framingScale", "boundsPolicy"], "camera.snapshot");
    if (value.snapshot.version !== 1) invalid("camera.snapshot.version", "must be 1");
    record(value.snapshot.focus, "camera.snapshot.focus");
    exact(value.snapshot.focus, ["x", "y"], "camera.snapshot.focus");
    finite(value.snapshot.focus.x, "camera.snapshot.focus.x");
    finite(value.snapshot.focus.y, "camera.snapshot.focus.y");
    boundedNumber(value.snapshot.framingScale, "camera.snapshot.framingScale", Number.MIN_VALUE, 16);
    if (value.snapshot.boundsPolicy !== "mapOverscroll") invalid("camera.snapshot.boundsPolicy", "must be mapOverscroll");
  } else invalid("camera.action", "is unsupported");
}

function recordingCrop(value: unknown) {
  record(value, "record-start.crop");
  exact(value, ["x", "y", "width", "height"], "record-start.crop");
  boundedNumber(value.x, "record-start.crop.x", 0, 2048);
  boundedNumber(value.y, "record-start.crop.y", 0, 2048);
  boundedNumber(value.width, "record-start.crop.width", 2, 2048);
  boundedNumber(value.height, "record-start.crop.height", 2, 2048);
}

function captureRegion(value: unknown, label: string) {
  if (value === "viewport" || value === "minimap") return;
  record(value, label);
  exact(value, ["x", "y", "width", "height"], label);
  boundedNumber(value.x, `${label}.x`, 0, 2048);
  boundedNumber(value.y, `${label}.y`, 0, 2048);
  boundedNumber(value.width, `${label}.width`, 2, 2048);
  boundedNumber(value.height, `${label}.height`, 2, 2048);
}

function idArray(value: unknown, label: string, minimum: number, maximum: number) {
  array(value, label, minimum, maximum, (entry) => u32(entry, label));
  if (new Set(value).size !== value.length) invalid(label, "must not contain duplicate ids");
}

function artifactToken(value: unknown, label: string) {
  if (value != null && (typeof value !== "string" || !/^[A-Za-z0-9_-]{1,48}$/.test(value))) invalid(label, "must be a safe artifact token");
}

function presentation(value: unknown, label: string) {
  if (value != null && !["clean", "normal"].includes(String(value))) invalid(label, "must be clean or normal");
}

function displayName(value: unknown, label: string, maximumBytes: number) {
  if (typeof value !== "string" || !value.trim() || Buffer.byteLength(value) > maximumBytes || /[\u0000-\u001f\u007f]/.test(value)) {
    invalid(label, `must be a non-empty display name of at most ${maximumBytes} UTF-8 bytes without control characters`);
  }
}

function validateGameCommand(value: unknown) {
  record(value, "order.command");
  const allowed = typeof value.c === "string" ? COMMAND_FIELDS[value.c as keyof typeof COMMAND_FIELDS] : undefined;
  if (!allowed) invalid("order.command.c", "is unsupported");
  exact(value, allowed, "order.command");
  if (allowed.includes("units")) refs(value.units, "order.command.units", 1, INTERACT_LIMITS.maxCommandUnits);
  if (allowed.includes("buildings")) refs(value.buildings, "order.command.buildings", 1, INTERACT_LIMITS.maxCommandUnits);
  for (const field of ["x", "y"]) {
    if (allowed.includes(field) && (value.c !== "useAbility" || value[field] != null)) finite(value[field], `order.command.${field}`);
  }
  for (const field of ["target", "node", "building"]) {
    if (allowed.includes(field) && !(field === "building" && value.c === "build")) entityRef(value[field], `order.command.${field}`);
  }
  for (const field of ["ability", "unit", "upgrade"]) if (allowed.includes(field)) token(value[field], `order.command.${field}`);
  if (value.c === "build") {
    token(value.building, "order.command.building");
    integer(value.tileX, "order.command.tileX", 0, U32_MAX);
    integer(value.tileY, "order.command.tileY", 0, U32_MAX);
  }
  if (value.targetObjectId != null) u32(value.targetObjectId, "order.command.targetObjectId");
  if (allowed.includes("delta") && value.delta !== -1 && value.delta !== 1) invalid("order.command.delta", "must be -1 or 1");
  if (value.kind != null && (typeof value.kind !== "string" || !["move", "attackMove", "attack", "gather", "build"].includes(value.kind))) invalid("order.command.kind", "is unsupported");
  optionalBoolean(value.queued, "order.command.queued");
  if (allowed.includes("enabled")) optionalBoolean(value.enabled, "order.command.enabled", false);
}

function artifactKind(value: unknown, label: string) { if (typeof value !== "string" || !["setup", "replay"].includes(value)) invalid(label, "must be setup or replay"); }
function artifactSelector(value: unknown, label: string) {
  record(value, label);
  const count = Number(value.artifactId != null) + Number(value.path != null);
  if (count !== 1) invalid(label, "must provide exactly one of artifactId or path");
  if (value.artifactId != null && (typeof value.artifactId !== "string" || !/^artifact_[a-f0-9]{32}$/.test(value.artifactId))) invalid(`${label}.artifactId`, "is invalid");
  if (value.path != null && (typeof value.path !== "string" || !value.path || value.path.length > 1024)) invalid(`${label}.path`, "must be a bounded path string");
}

function exact(value: CommandInput, allowed: readonly string[], label: string) { const extras = Object.keys(value).filter((key) => !allowed.includes(key)); if (extras.length) invalid(label, `contains unexpected field ${JSON.stringify(extras[0])}`); return value; }
function record(value: unknown, label: string): asserts value is CommandInput { if (!value || typeof value !== "object" || Array.isArray(value)) invalid(label, "must be a JSON object"); }
function array(value: unknown, label: string, minimum: number, maximum: number, validate: (entry: unknown, index: number) => void): asserts value is unknown[] { if (!Array.isArray(value) || value.length < minimum || value.length > maximum) invalid(label, `must contain ${minimum}-${maximum} items`); value.forEach(validate); }
function refs(value: unknown, label: string, minimum: number, maximum: number) { array(value, label, minimum, maximum, (entry) => entityRef(entry, label)); }
function entityRef(value: unknown, label: string) { if (typeof value === "string") alias(value); else u32(value, label); }
function alias(value: unknown): asserts value is string { if (typeof value !== "string" || !ALIAS_RE.test(value)) invalid("alias", "must start with a letter and contain only letters, digits, _ or -"); }
function sessionId(value: unknown): asserts value is string { if (typeof value !== "string" || !SESSION_RE.test(value)) invalid("sessionId", "must be an Interact session id"); }
function token(value: unknown, label: string, maximum = 64): asserts value is string { if (typeof value !== "string" || !TOKEN_RE.test(value) || value.length > maximum) invalid(label, "must be a safe protocol token"); }
function finite(value: unknown, label: string): asserts value is number { if (typeof value !== "number" || !Number.isFinite(value)) invalid(label, "must be a finite number"); }
function boundedNumber(value: unknown, label: string, minimum: number, maximum: number): asserts value is number { finite(value, label); if (value < minimum || value > maximum) invalid(label, `must be from ${minimum} to ${maximum}`); }
function isInteger(value: unknown, minimum: number, maximum: number): value is number { return typeof value === "number" && Number.isInteger(value) && value >= minimum && value <= maximum; }
function integer(value: unknown, label: string, minimum: number, maximum: number): number { if (!isInteger(value, minimum, maximum)) invalid(label, `must be an integer from ${minimum} to ${maximum}`); return value; }
function u32(value: unknown, label: string) { return integer(value, label, 1, U32_MAX); }
function optionalBoolean(value: unknown, label: string, optional = true) { if (value == null && optional) return; if (typeof value !== "boolean") invalid(label, "must be a boolean"); }
function viewport(value: unknown, maximum: number, label: string) { record(value, label); exact(value, ["width", "height", "deviceScaleFactor"], label); integer(value.width, `${label}.width`, 320, maximum); integer(value.height, `${label}.height`, 240, maximum); if (value.deviceScaleFactor != null) boundedNumber(value.deviceScaleFactor, `${label}.deviceScaleFactor`, Number.MIN_VALUE, 4); }
function invalid(label: string, message: string): never { throw Object.assign(new Error(`${label} ${message}.`), { code: "invalidInput" }); }

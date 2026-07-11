// Test-only deterministic driver for the CLI and daemon contract harness.

const CATALOG = Object.freeze({
  maps: [{ name: "Default", width: 64, height: 64, tileSize: 32 }],
  players: [
    { id: 1, teamId: 1, factionId: "kriegsia", name: "North", color: "#fff" },
    { id: 2, teamId: 2, factionId: "kriegsia", name: "South", color: "#000" },
  ],
  factions: [{
    id: "kriegsia",
    label: "Kriegsia",
    units: ["rifleman", "tank"],
    buildings: ["barracks", "factory"],
    upgrades: ["entrenchment"],
  }],
  supportedCommandKinds: [
    "move", "attackMove", "attack", "deconstruct", "setupAntiTankGuns", "tearDownAntiTankGuns",
    "charge", "useAbility", "recastAbility", "setAutocast", "gather", "build", "train",
    "research", "cancel", "stop", "holdPosition", "setRally",
  ],
  abilities: ["charge", "smoke"],
});
const ONE_PIXEL_PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLGCQAAAABJRU5ErkJggg==";

export async function openLabInteractDriver(options) {
  const openDelayMs = Number(process.env.RTS_LAB_INTERACT_FAKE_OPEN_DELAY_MS || 0);
  if (openDelayMs > 0) await new Promise((resolve) => setTimeout(resolve, openDelayMs));
  let nextId = 100;
  let tick = 0;
  let closed = false;
  let entities = [];
  let recording = null;
  let lastRecording = null;
  let lastRecordingCompletion = null;
  let fixedCapture = null;
  const camera = {
    version: 1,
    focus: { x: 0, y: 0 },
    framingScale: 1,
    boundsPolicy: "mapOverscroll",
  };
  const cameraWorldBounds = { minX: 0, minY: 0, maxX: 2048, maxY: 2048 };
  const project = (entity) => ({ ...entity, orderPlan: entity.orderPlan.map((stage) => ({ ...stage })) });
  const inspect = ({ ids = [], kinds = [], owners = [], cameraViewport = false, limit = 25 } = {}) => {
    let rows = entities;
    if (ids.length) rows = rows.filter((entity) => ids.includes(entity.id));
    if (kinds.length) rows = rows.filter((entity) => kinds.includes(entity.kind));
    if (owners.length) rows = rows.filter((entity) => owners.includes(entity.owner));
    if (cameraViewport) rows = rows.filter((entity) => entity.x >= cameraWorldBounds.minX && entity.x <= cameraWorldBounds.maxX && entity.y >= cameraWorldBounds.minY && entity.y <= cameraWorldBounds.maxY);
    return {
      entities: rows.slice(0, limit).map(project),
      truncated: rows.length > limit,
      totalMatching: rows.length,
      players: CATALOG.players.map((player) => ({ ...player })),
      room: { tick, roomTime: { currentTick: tick, speed: 0, paused: true }, map: CATALOG.maps[0] },
      camera: structuredClone(camera),
      cameraWorldBounds: { ...cameraWorldBounds },
    };
  };
  const finishRecording = (reason, { aliases = [] } = {}) => {
    if (!recording) throw Object.assign(new Error("No recording is active."), { code: "recordingInactive" });
    if (recording.finalizing) return recording.finalizing;
    const current = recording;
    current.finalizing = (async () => {
      const finalizeDelayMs = Number(process.env.RTS_LAB_INTERACT_FAKE_RECORD_FINALIZE_DELAY_MS || 0);
      if (finalizeDelayMs > 0) await new Promise((resolve) => setTimeout(resolve, finalizeDelayMs));
      const directory = `${options.workspaceRoot}/target/lab-interact/${current.sessionId}/recordings/${current.name}-fixture`;
      const result = {
        active: false,
        stoppedBy: reason,
        videoPath: `${directory}/${current.name}.mp4`,
        framePaths: [`${directory}/frames/frame-01.png`, `${directory}/frames/frame-02.png`],
        contactSheetPath: `${directory}/${current.name}-contact-sheet.png`,
        manifestPath: `${directory}/${current.name}.json`,
        probe: { codec: "h264", width: 640, height: 480, frameRate: "30/1", durationSeconds: 1 },
        frameDiagnostics: { expectedAt30Fps: 30, captured: 30, encoded: 30, droppedEstimate: 0, duplicatedEstimate: 0 },
        authoritative: { startTick: current.startTick, endTick: tick },
        fixtureMetadata: { operations: current.operations, aliases },
      };
      lastRecording = result;
      return result;
    })().finally(() => {
      clearTimeout(current.watchdog);
      if (recording === current) recording = null;
    });
    void current.finalizing.then(current.completion.resolve, current.completion.reject);
    return current.finalizing;
  };
  return {
    workspace: { root: options.workspaceRoot, branch: "fixture", head: "a".repeat(40) },
    async status() {
      return { ready: !closed, reason: closed ? "closed" : "ready", snapshotTick: tick, room: "labinteract-fixture" };
    },
    async catalog() {
      return structuredClone(CATALOG);
    },
    async spawn(spawns) {
      const delayMs = Number(process.env.RTS_LAB_INTERACT_FAKE_DELAY_MS || 0);
      if (delayMs > 0) await new Promise((resolve) => setTimeout(resolve, delayMs));
      const added = spawns.map((spec) => ({ id: nextId++, kind: spec.kind, owner: spec.owner, x: spec.x, y: spec.y, hp: 100, maxHp: 100, state: "idle", orderPlan: [] }));
      entities.push(...added);
      tick += 1;
      return {
        result: { op: "spawnEntities", outcome: { items: added.map((entity, index) => ({ index, outcome: { entityId: entity.id } })) }, snapshotTick: tick },
        entities: added.map(project),
      };
    },
    async update(operations) {
      for (const operation of operations) {
        if (operation.operation === "move") {
        const entity = entities.find((row) => row.id === operation.entityId);
        if (!entity) throw Object.assign(new Error("unknown entity"), { code: "unknownEntity" });
        entity.x = operation.x;
        entity.y = operation.y;
        }
        if (operation.operation === "reassign") {
        const entity = entities.find((row) => row.id === operation.entityId);
        if (!entity) throw Object.assign(new Error("unknown entity"), { code: "unknownEntity" });
        entity.owner = operation.owner;
        }
      }
      tick += 1;
      return { result: { op: "applyUpdates", snapshotTick: tick } };
    },
    async remove(ids) {
      entities = entities.filter((entity) => !ids.includes(entity.id));
      tick += 1;
      return { result: { op: "deleteEntities", outcome: { items: ids.map((id, index) => ({ index, outcome: { entityId: id } })) }, snapshotTick: tick } };
    },
    async order({ command }) {
      for (const id of command.units || []) {
        const entity = entities.find((row) => row.id === id);
        if (entity) entity.orderPlan = [{ kind: command.c, x: command.x ?? null, y: command.y ?? null, target: command.target ?? null }];
      }
      if (command.c === "deconstruct" && command.target != null) {
        entities = entities.filter((entity) => entity.id !== command.target);
      }
      tick += 1;
      return { result: { op: "issueCommandAs", outcome: { accepted: true }, snapshotTick: tick } };
    },
    async time(control) {
      if (control.action === "step") tick += control.ticks || 1;
      if (control.action === "seek") tick = control.tick;
      return { roomTime: { currentTick: tick, speed: control.action === "resume" ? control.speed || 1 : 0, paused: control.action !== "resume" }, snapshotTick: tick };
    },
    async inspect(query) {
      return inspect(query);
    },
    async camera(command) {
      if (command.action === "set") {
        Object.assign(camera, structuredClone(command.snapshot));
      }
      if (command.action === "focus") {
        const rows = entities.filter((entity) => command.entityIds.includes(entity.id));
        camera.focus.x = rows.reduce((sum, entity) => sum + entity.x, 0) / rows.length;
        camera.focus.y = rows.reduce((sum, entity) => sum + entity.y, 0) / rows.length;
      }
      return { camera: structuredClone(camera), cameraWorldBounds: { ...cameraWorldBounds } };
    },
    async screenshot({ sessionId, name, presentation, viewport, subjectSummaries, request }) {
      const width = viewport?.width || 1;
      const height = viewport?.height || 1;
      const subjects = Array.isArray(subjectSummaries) ? subjectSummaries : [];
      return {
        pngPath: `${options.workspaceRoot}/target/lab-interact/${sessionId}/captures/${name}.png`,
        manifestPath: `${options.workspaceRoot}/target/lab-interact/${sessionId}/captures/${name}.json`,
        image: {
          mimeType: "image/png",
          data: ONE_PIXEL_PNG,
          bytes: Buffer.from(ONE_PIXEL_PNG, "base64").length,
          width,
          height,
        },
        presentation,
        readiness: {
          ready: true,
          frame: 2,
          snapshotTick: tick,
          subjects: { count: subjects.length, details: subjects.slice(0, 24), truncated: subjects.length > 24 },
          missingTextureSubjectIds: [],
          missingTextureSubjectCount: 0,
          missingTextureSubjectsTruncated: false,
          request,
        },
      };
    },
    recordingStatus() {
      return recording ? { active: true, name: recording.name, maxDurationMs: recording.maxDurationMs } : { active: false, last: lastRecording };
    },
    async recordStart({ sessionId, name = "recording", maxDurationMs = 10_000, viewport = null, crop = null, scale = 1 }) {
      if (recording) throw Object.assign(new Error("A recording is already active."), { code: "recordingActive" });
      const completion = deferred();
      lastRecording = null;
      lastRecordingCompletion = completion;
      recording = { sessionId, name, maxDurationMs, viewport, crop, scale, startTick: tick, operations: [], completion, finalizing: null };
      recording.watchdog = setTimeout(() => { void finishRecording("watchdog").catch(() => {}); }, maxDurationMs);
      recording.watchdog.unref?.();
      return { active: true, name, maxDurationMs, authoritativeStartTick: tick };
    },
    recordAcceptedOperation(operation) {
      if (!recording) return false;
      recording.operations.push(operation);
      return true;
    },
    async recordStop({ aliases = [] } = {}) {
      return finishRecording("explicit", { aliases });
    },
    recordWait() {
      const completion = recording?.completion || lastRecordingCompletion;
      if (!completion) return Promise.reject(Object.assign(new Error("No recording has been started."), { code: "recordingInactive" }));
      return completion.promise;
    },
    settleRecording(reason, metadata = {}) {
      return recording ? finishRecording(reason, metadata) : null;
    },
    async captureFixed({ sessionId, name = "fixed", fps = 30, frameCount = 30, sceneIdentity = null, sceneRevision = 0, aliases = [] }) {
      const startTick = tick;
      const representativeFramePaths = Array.from({ length: Math.min(frameCount, 6) }, (_, index) => `${options.workspaceRoot}/target/lab-interact/${sessionId}/fixed/${name}/frames/frame-${String(index).padStart(4, "0")}.png`);
      tick = startTick + Math.floor((frameCount - 1) * 30 / fps);
      return {
        videoPath: `${options.workspaceRoot}/target/lab-interact/${sessionId}/fixed/${name}/${name}.mp4`,
        contactSheetPath: `${options.workspaceRoot}/target/lab-interact/${sessionId}/fixed/${name}/${name}-contact-sheet.png`,
        manifestPath: `${options.workspaceRoot}/target/lab-interact/${sessionId}/fixed/${name}/${name}.json`,
        frameSummary: { count: frameCount, uniqueHashes: frameCount, representativeFramePaths, detailsInManifest: true },
        authoritative: { startTick, endTick: tick },
        mapping: { simulationHz: 30, outputFps: fps }, fixtureMetadata: { sceneIdentity, sceneRevision, aliases },
      };
    },
    fixedCaptureStatus() { return fixedCapture || { active: false }; },
    cancelFixedCapture() {
      if (!fixedCapture) throw Object.assign(new Error("No fixed capture is active."), { code: "captureInactive" });
      fixedCapture.cancelled = true;
      return { cancelling: true };
    },
    async reset() {
      entities = [];
      tick = 0;
      return { roomTime: { currentTick: tick, speed: 0, paused: true }, snapshotTick: tick };
    },
    async exportSetup(name = "") {
      return { scenario: checkpointScenario(name, tick, entities) };
    },
    async importSetup(scenario) {
      const restored = JSON.parse(scenario.checkpointPayload).entities || [];
      const entityIdMap = restored.map((entity) => ({ oldId: entity.id, newId: entity.id + 1000 }));
      entities = restored.map((entity) => ({ ...entity, id: entity.id + 1000, orderPlan: entity.orderPlan || [] }));
      tick = scenario.metadata.exportedTick;
      return { entityIdMap, result: { op: "importScenario", snapshotTick: tick } };
    },
    async exportReplay(name = "") {
      const artifact = {
        schema: "rts.labReplay", schemaVersion: 1, kind: "labReplay", serverBuildSha: "a".repeat(40),
        authoring: { name }, initialSetup: checkpointScenario(name, 0, entities),
        timeline: { initialTick: 0, durationTicks: tick, keyframeIntervalTicks: 2000 }, operations: [],
      };
      return { bytes: Buffer.from(JSON.stringify(artifact)), transfer: { artifactId: "transfer_fixture" } };
    },
    async importReplay(bytes) {
      const artifact = JSON.parse(bytes);
      entities = (JSON.parse(artifact.initialSetup.checkpointPayload).entities || []).map((entity) => ({ ...entity, orderPlan: entity.orderPlan || [] }));
      tick = artifact.timeline.durationTicks;
      return { imported: true };
    },
    async close() {
      if (recording) await finishRecording("sessionClose").catch(() => {});
      closed = true;
    },
  };
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  void promise.catch(() => {});
  return { promise, resolve, reject };
}

function checkpointScenario(name, tick, entities) {
  return {
    schemaVersion: 1, kind: "labCheckpointScenario", name: name || "Fixture setup", seed: 1,
    map: { name: "Default", schemaVersion: 1, contentHash: "content", materializedHash: "materialized", data: { size: 64, terrain: [], starts: [], expansionSites: [] } },
    metadata: { exportedTick: tick, lab: { vision: { mode: "fullWorld" } }, sourceEntityIdMap: entities.map((entity) => ({ oldId: entity.id, newId: entity.id })) },
    checkpointPayload: JSON.stringify({ entities }),
  };
}

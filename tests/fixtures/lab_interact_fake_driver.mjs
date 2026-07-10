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
  const camera = {
    x: 0,
    y: 0,
    zoom: 1,
    worldBounds: { minX: 0, minY: 0, maxX: 2048, maxY: 2048 },
  };
  const project = (entity) => ({ ...entity, orderPlan: entity.orderPlan.map((stage) => ({ ...stage })) });
  const inspect = ({ ids = [], kinds = [], owners = [], cameraViewport = false, limit = 25 } = {}) => {
    let rows = entities;
    if (ids.length) rows = rows.filter((entity) => ids.includes(entity.id));
    if (kinds.length) rows = rows.filter((entity) => kinds.includes(entity.kind));
    if (owners.length) rows = rows.filter((entity) => owners.includes(entity.owner));
    if (cameraViewport) rows = rows.filter((entity) => entity.x >= camera.worldBounds.minX && entity.x <= camera.worldBounds.maxX && entity.y >= camera.worldBounds.minY && entity.y <= camera.worldBounds.maxY);
    return {
      entities: rows.slice(0, limit).map(project),
      truncated: rows.length > limit,
      totalMatching: rows.length,
      players: CATALOG.players.map((player) => ({ ...player })),
      room: { tick, roomTime: { currentTick: tick, speed: 0, paused: true }, map: CATALOG.maps[0] },
      camera: { ...camera },
    };
  };
  return {
    workspace: { root: options.workspaceRoot, branch: "fixture", head: "a".repeat(40) },
    async status() {
      return { ready: !closed, reason: closed ? "closed" : "ready", snapshotTick: tick };
    },
    async catalog() {
      return structuredClone(CATALOG);
    },
    async spawn(spec) {
      const delayMs = Number(process.env.RTS_LAB_INTERACT_FAKE_DELAY_MS || 0);
      if (delayMs > 0) await new Promise((resolve) => setTimeout(resolve, delayMs));
      const entity = { id: nextId++, kind: spec.kind, owner: spec.owner, x: spec.x, y: spec.y, hp: 100, maxHp: 100, state: "idle", orderPlan: [] };
      entities.push(entity);
      tick += 1;
      return { result: { op: "spawnEntity", outcome: { entityId: entity.id }, snapshotTick: tick }, entity: project(entity) };
    },
    async update(operation) {
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
      tick += 1;
      return { result: { op: operation.operation, snapshotTick: tick } };
    },
    async remove(ids) {
      entities = entities.filter((entity) => !ids.includes(entity.id));
      tick += 1;
      return { results: ids.map((id) => ({ op: "deleteEntity", outcome: { entityId: id }, snapshotTick: tick })) };
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
        if (command.centerX != null) camera.x = command.centerX;
        if (command.centerY != null) camera.y = command.centerY;
        if (command.zoom != null) camera.zoom = command.zoom;
      }
      if (command.action === "focus") {
        const rows = entities.filter((entity) => command.entityIds.includes(entity.id));
        camera.x = rows.reduce((sum, entity) => sum + entity.x, 0) / rows.length;
        camera.y = rows.reduce((sum, entity) => sum + entity.y, 0) / rows.length;
      }
      return { camera: { ...camera } };
    },
    async screenshot({ sessionId, name, presentation, viewport, subjectSummaries, request }) {
      const width = viewport?.width || 1;
      const height = viewport?.height || 1;
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
          subjects: subjectSummaries || [],
          request,
        },
      };
    },
    async reset() {
      entities = [];
      tick = 0;
      return { roomTime: { currentTick: tick, speed: 0, paused: true }, snapshotTick: tick };
    },
    async close() {
      closed = true;
    },
  };
}

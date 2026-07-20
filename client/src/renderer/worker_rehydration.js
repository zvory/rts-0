export function createWorkerPresentationState() {
  let staticMap = null;
  let visible = null;
  let explored = null;
  let generation = 0;
  const durable = [];
  let lastDurableRevision = 0;

  return {
    reset(nextGeneration) {
      generation = nextGeneration;
      staticMap = null;
      visible = null;
      explored = null;
      durable.length = 0;
      lastDurableRevision = 0;
    },
    map(message) {
      ensureGeneration(message.generation, generation);
      const map = message.payload.map;
      staticMap = Object.freeze({
        ...map,
        generation: message.generation,
        terrain: gridSnapshot(map.terrain),
      });
      return staticMap;
    },
    revisions(message) {
      ensureGeneration(message.generation, generation);
      if (message.payload.revisions.visible) visible = gridSnapshot(message.payload.revisions.visible);
      if (message.payload.revisions.explored) explored = gridSnapshot(message.payload.revisions.explored);
    },
    retainDecals(message) {
      ensureGeneration(message.generation, generation);
      const revision = message.payload.revision;
      if (revision <= lastDurableRevision) return false;
      lastDurableRevision = revision;
      durable.push({ revision, decals: message.payload.decals });
      return true;
    },
    frame(message) {
      ensureGeneration(message.generation, generation);
      const frame = message.payload.frame;
      if (!staticMap || staticMap.revision !== frame.staticMapRevision) {
        throw new Error(`Static map revision ${frame.staticMapRevision} is unavailable in the render worker.`);
      }
      if (!visible || visible.revision !== frame.visible.revision) {
        throw new Error(`Visible-grid revision ${frame.visible.revision} is unavailable in the render worker.`);
      }
      if (!explored || explored.revision !== frame.explored.revision) {
        throw new Error(`Explored-grid revision ${frame.explored.revision} is unavailable in the render worker.`);
      }
      let next = { ...frame, visible, explored };
      if (durable.length) {
        const groundDecals = durable.flatMap((entry) => entry.decals);
        const persistent = (frame.layers?.persistentGroundMark || [])
          .filter((record) => record?.type !== "groundDecal");
        next = {
          ...next,
          groundDecalRevision: durable[durable.length - 1].revision,
          layers: {
            ...frame.layers,
            persistentGroundMark: [...persistent, ...groundDecals],
          },
        };
      }
      return next;
    },
    decalsPresented(revision) {
      const presentedRevision = Number(revision) || 0;
      let retained = 0;
      while (retained < durable.length && durable[retained].revision <= presentedRevision) retained += 1;
      if (retained > 0) durable.splice(0, retained);
    },
    get staticMap() {
      return staticMap;
    },
  };
}

export function compatibilityState(record) {
  const currentById = new Map();
  const previousById = new Map();
  const recoilById = new Map();
  const recoilPhaseById = new Map();
  const recoilKindById = new Map();
  for (const pose of record?.poses || []) {
    if (pose.current) currentById.set(pose.id, pose.current);
    if (pose.previous) previousById.set(pose.id, pose.previous);
    if (pose.recoil) recoilById.set(pose.id, pose.recoil);
    if (pose.recoilPhase) recoilPhaseById.set(pose.id, pose.recoilPhase);
    if (pose.recoilKind) recoilKindById.set(pose.id, pose.recoilKind);
  }
  return {
    resources: { oil: Number.isFinite(record?.oil) ? record.oil : null },
    _curById: currentById,
    _prevById: previousById,
    weaponRecoil: (id) => recoilById.get(id) || 0,
    weaponRecoilPhase: (id) => recoilPhaseById.get(id) || 0,
    weaponRecoilKind: (id) => recoilKindById.get(id),
  };
}

function gridSnapshot(record) {
  const values = new Uint8Array(record.values);
  return Object.freeze({
    version: record.version,
    revision: record.revision,
    width: record.width,
    height: record.height,
    values,
    get(index) {
      return Number.isInteger(index) && index >= 0 && index < values.length ? values[index] : undefined;
    },
    copyInto(target, offset = 0) {
      target.set(values, offset);
      return values.length;
    },
  });
}

function ensureGeneration(actual, expected) {
  if (actual !== expected) throw new Error(`Render-worker generation ${actual} does not match ${expected}.`);
}

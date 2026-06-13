export function summarizeSnapshot(snapshot, playerId) {
  if (!snapshot) return null;
  const entities = (snapshot.entities || []).map(summarizeEntity);
  return {
    tick: snapshot.tick ?? null,
    steel: snapshot.steel ?? null,
    oil: snapshot.oil ?? null,
    supplyUsed: snapshot.supplyUsed ?? null,
    supplyCap: snapshot.supplyCap ?? null,
    playerId,
    netStatus: snapshot.netStatus || null,
    entities,
    owned: entities.filter((entity) => entity.owner === playerId),
  };
}

export function summarizeEntity(entity) {
  return {
    id: entity.id,
    owner: entity.owner,
    kind: entity.kind,
    x: round(entity.x),
    y: round(entity.y),
    hp: entity.hp,
    state: entity.state,
    prodKind: entity.prodKind,
    prodUpgrade: entity.prodUpgrade,
    prodProgress: round(entity.prodProgress),
    prodQueue: entity.prodQueue,
    progressPredicted: entity.progressPredicted === true,
    orderPlan: summarizePlan(entity.orderPlan),
    rallyPlan: summarizePlan(entity.rallyPlan),
  };
}

export function summarizePlan(plan) {
  return (plan || []).map((stage) => ({
    kind: stage.kind,
    x: round(stage.x),
    y: round(stage.y),
  }));
}

export function ownEntityByKind(summary, kind, index = 0) {
  const matches = (summary?.owned || [])
    .filter((entity) => entity.kind === kind)
    .sort((a, b) => a.id - b.id);
  return matches[index] || null;
}

export function compareOwnedPosition({ remote, client, kind, index = 0, tolerancePx = 1 }) {
  const a = ownEntityByKind(remote, kind, index);
  const b = ownEntityByKind(client, kind, index);
  if (!a || !b) {
    return {
      ok: false,
      reason: `missing owned ${kind}[${index}]`,
      remote: a,
      client: b,
    };
  }
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  const distance = Math.hypot(dx, dy);
  return {
    ok: distance <= tolerancePx,
    distance: round(distance),
    tolerancePx,
    remote: { id: a.id, x: a.x, y: a.y, tick: remote.tick },
    client: { id: b.id, x: b.x, y: b.y, tick: client.tick },
  };
}

export function compareOwnedOrderPlan({ remote, client, kind, index = 0 }) {
  const a = ownEntityByKind(remote, kind, index);
  const b = ownEntityByKind(client, kind, index);
  if (!a || !b) {
    return {
      ok: false,
      reason: `missing owned ${kind}[${index}]`,
      remote: a,
      client: b,
    };
  }
  const remotePlan = summarizePlan(a.orderPlan);
  const clientPlan = summarizePlan(b.orderPlan);
  return {
    ok: JSON.stringify(remotePlan) === JSON.stringify(clientPlan),
    remote: remotePlan,
    client: clientPlan,
  };
}

function round(value) {
  return Number.isFinite(value) ? Math.round(value * 100) / 100 : value;
}

const HANDOFF_COLLECTION_URL = "/api/map-handoffs";

export async function createMapHandoff({
  destination,
  authoredMap,
  materializedMap,
  selectedLayoutId,
  fetchImpl = globalThis.fetch?.bind(globalThis),
}) {
  if (!fetchImpl) throw new Error("Map handoffs require fetch support.");
  const response = await fetchImpl(HANDOFF_COLLECTION_URL, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ destination, authoredMap, materializedMap, selectedLayoutId }),
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(payload?.error || `Map handoff failed (HTTP ${response.status}).`);
  if (!/^[a-f0-9]{32}$/.test(payload?.handoffId || "")) {
    throw new Error("Map handoff response did not include a valid id.");
  }
  return payload;
}

export async function consumeMapHandoff(handoffId, {
  fetchImpl = globalThis.fetch?.bind(globalThis),
} = {}) {
  if (!fetchImpl) throw new Error("Map handoffs require fetch support.");
  if (!/^[a-f0-9]{32}$/.test(handoffId || "")) throw new Error("Invalid map handoff id.");
  const response = await fetchImpl(`${HANDOFF_COLLECTION_URL}/${handoffId}`, {
    method: "POST",
    cache: "no-store",
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(payload?.error || `Map handoff failed (HTTP ${response.status}).`);
  return payload;
}

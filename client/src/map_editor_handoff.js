const HANDOFF_COLLECTION_URL = "/api/map-handoffs";

export async function createMapHandoff({
  destination,
  authoredMap,
  materializedMap,
  fetchImpl = globalThis.fetch?.bind(globalThis),
  timeoutMs = 15_000,
}) {
  if (!fetchImpl) throw new Error("Map handoffs require fetch support.");
  const { response, payload } = await boundedJsonFetch(fetchImpl, HANDOFF_COLLECTION_URL, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ destination, authoredMap, materializedMap }),
  }, timeoutMs);
  if (!response.ok) throw new Error(payload?.error || `Map handoff failed (HTTP ${response.status}).`);
  if (!/^[a-f0-9]{32}$/.test(payload?.handoffId || "")) {
    throw new Error("Map handoff response did not include a valid id.");
  }
  return payload;
}

export async function consumeMapHandoff(handoffId, {
  fetchImpl = globalThis.fetch?.bind(globalThis),
  timeoutMs = 15_000,
} = {}) {
  if (!fetchImpl) throw new Error("Map handoffs require fetch support.");
  if (!/^[a-f0-9]{32}$/.test(handoffId || "")) throw new Error("Invalid map handoff id.");
  const { response, payload } = await boundedJsonFetch(fetchImpl, `${HANDOFF_COLLECTION_URL}/${handoffId}`, {
    method: "POST",
    cache: "no-store",
  }, timeoutMs);
  if (!response.ok) throw new Error(payload?.error || `Map handoff failed (HTTP ${response.status}).`);
  return payload;
}

async function boundedJsonFetch(fetchImpl, url, options, timeoutMs) {
  const timeout = Number(timeoutMs);
  if (!Number.isFinite(timeout) || timeout < 1 || timeout > 60_000) throw new RangeError("Map handoff timeout must be 1–60000 ms.");
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(new Error("Map handoff request timed out.")), timeout);
  try {
    const response = await fetchImpl(url, { ...options, signal: controller.signal });
    const payload = await response.json().catch(() => ({}));
    return { response, payload };
  } catch (error) {
    if (controller.signal.aborted) throw new Error("Map handoff request timed out.");
    throw error;
  } finally {
    clearTimeout(timer);
  }
}

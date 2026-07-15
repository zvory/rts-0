export function stressTestLaunchConfig(location = window.location) {
  if (location.pathname !== "/stress-test" && location.pathname !== "/stress-test/") return null;
  const params = new URLSearchParams(location.search);
  const rawLabel = (params.get("label") || "").trim();
  const label = rawLabel
    .replace(/[^A-Za-z0-9 ._-]/g, "_")
    .replace(/\s+/g, " ")
    .slice(0, 64);
  const rawSeconds = params.get("seconds");
  const requestedSeconds = rawSeconds === null ? Number.NaN : Number(rawSeconds);
  const durationSeconds = Number.isFinite(requestedSeconds)
    ? Math.min(25, Math.max(2, Math.round(requestedSeconds)))
    : 15;
  return {
    id: "supply-300-hellhole",
    label,
    durationSeconds,
    warmupSeconds: 3,
    banner: "client-only Hellhole stress test · no WebSocket or live simulation",
  };
}

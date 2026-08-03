const HANDOFF_PATTERN = /^[a-f0-9]{32}$/;

export function mapPreviewLaunchConfig(locationLike = window.location) {
  const pathname = String(locationLike?.pathname || "");
  if (pathname !== "/map-preview" && pathname !== "/map-preview/") return null;
  const params = new URLSearchParams(locationLike?.search || "");
  const handoffId = String(params.get("handoff") || "").trim().toLowerCase();
  return Object.freeze({
    handoffId: HANDOFF_PATTERN.test(handoffId) ? handoffId : "",
    error: HANDOFF_PATTERN.test(handoffId) ? "" : "Map preview requires a valid one-use handoff id.",
  });
}

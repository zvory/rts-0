export function mapEditorLaunchConfig(locationLike = window.location) {
  const pathname = String(locationLike?.pathname || "");
  if (pathname !== "/map-editor" && pathname !== "/map-editor/") return null;
  const params = new URLSearchParams(locationLike?.search || "");
  const handoffId = String(params.get("handoff") || "").trim().toLowerCase();
  const interact = params.get("interact") === "map-editor";
  const mapFile = String(params.get("map") || "").trim();
  const validMapFile = /^[A-Za-z0-9_-]{1,64}\.json$/.test(mapFile);
  return {
    handoffId: /^[a-f0-9]{32}$/.test(handoffId) ? handoffId : "",
    interact,
    mapFile: interact && validMapFile ? mapFile : "",
    error: handoffId && !/^[a-f0-9]{32}$/.test(handoffId)
      ? "Invalid Map Editor handoff id."
      : interact && !validMapFile ? "Invalid Interact Map Editor map selector." : "",
  };
}

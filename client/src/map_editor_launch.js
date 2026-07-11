export function mapEditorLaunchConfig(locationLike = window.location) {
  const pathname = String(locationLike?.pathname || "");
  if (pathname !== "/map-editor" && pathname !== "/map-editor/") return null;
  const params = new URLSearchParams(locationLike?.search || "");
  const handoffId = String(params.get("handoff") || "").trim().toLowerCase();
  const workspace = String(params.get("workspace") || "default").trim();
  const workspaceId = /^[A-Za-z0-9_-]{1,48}$/.test(workspace) ? workspace : "default";
  return {
    handoffId: /^[a-f0-9]{32}$/.test(handoffId) ? handoffId : "",
    workspaceId,
    error: handoffId && !/^[a-f0-9]{32}$/.test(handoffId) ? "Invalid Map Editor handoff id." : "",
  };
}

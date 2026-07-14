export function gameSessionCapabilities(role: unknown, maxSessions: number) {
  const player = role === "player";
  return {
    aliases: false, inspectUi: true, orders: player ? ["move"] : [], giveUp: player,
    media: ["screenshot", "recording", "timelapse"], role, maxSessions,
  };
}

export function gameInspectionOwnership(requested: unknown, role: unknown) {
  return typeof requested === "string" ? requested : role === "spectator" ? "visible" : "owned";
}

export function requireGameSpectator(role: unknown) {
  if (role !== "spectator") {
    throw Object.assign(new Error("capture-timelapse requires a game opened with spectate:[ai,ai]."), { code: "spectatorRequired" });
  }
}

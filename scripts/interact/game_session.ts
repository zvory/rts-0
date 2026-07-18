export function gameSessionCapabilities(role: unknown, maxSessions: number) {
  const player = role === "player";
  const spectator = role === "spectator";
  return {
    aliases: false, inspectUi: true, selection: true, orders: player ? ["move"] : [], giveUp: player,
    media: spectator ? ["screenshot", "recording", "timelapse"] : ["screenshot", "recording"],
    role, maxSessions,
  };
}

export function scenarioSessionCapabilities(maxSessions: number) {
  return {
    aliases: false,
    inspectUi: true,
    selection: true,
    orders: [],
    giveUp: false,
    media: ["screenshot", "recording", "timelapse"],
    role: "observer",
    maxSessions,
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

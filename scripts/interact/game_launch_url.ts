import { DEFAULT_LAB_MAP } from "./session_defaults.ts";

export function interactLaunchUrl({
  mode,
  baseUrl,
  room,
  map,
  opponent,
  spectate,
  autoSpectator = false,
  seed,
  scenario,
  visualProfile,
  devScenario,
}: {
  mode: "lab" | "game" | "scenario" | "map-editor";
  baseUrl: string;
  room: string;
  map: string;
  opponent: string;
  spectate?: readonly string[] | null;
  autoSpectator?: boolean;
  seed: string;
  scenario: string;
  visualProfile?: string;
  devScenario: { id: string; unit: string; count: number; blocker: string; case: string };
}) {
  if (mode === "map-editor") {
    const url = new URL("/map-editor", baseUrl);
    url.searchParams.set("map", map);
    url.searchParams.set("interact", "map-editor");
    url.searchParams.set("rtsNoAutoPointerLock", "1");
    return url.href;
  }
  if (mode === "lab") {
    const url = new URL("/lab", baseUrl);
    url.searchParams.set("room", room);
    url.searchParams.set("map", safeToken(map, DEFAULT_LAB_MAP, 48));
    if (seed) url.searchParams.set("seed", seed);
    if (scenario) url.searchParams.set("scenario", safeToken(scenario, "blank", 48));
    if (visualProfile) url.searchParams.set("visualProfile", safeToken(visualProfile, "", 48));
    url.searchParams.set("interact", "lab");
    url.searchParams.set("rtsNoAutoPointerLock", "1");
    return url.href;
  }
  if (mode === "scenario") {
    const url = new URL("/", baseUrl);
    url.searchParams.set("watchScenario", "1");
    url.searchParams.set("id", devScenario.id);
    url.searchParams.set("unit", devScenario.unit);
    url.searchParams.set("count", String(devScenario.count));
    if (devScenario.blocker) url.searchParams.set("blocker", devScenario.blocker);
    if (devScenario.case) url.searchParams.set("case", devScenario.case);
    url.searchParams.set("interact", "dev-scenario");
    url.searchParams.set("rtsNoAutoPointerLock", "1");
    return url.href;
  }
  const url = new URL("/", baseUrl);
  const aiPlayers = Array.isArray(spectate) && spectate.length === 2 ? spectate : null;
  url.searchParams.set("rtsLaunch", "match");
  url.searchParams.set("rtsRoom", room);
  url.searchParams.set("rtsRole", aiPlayers ? "spectator" : "player");
  if (aiPlayers) {
    url.searchParams.append("rtsAi", `1:${aiPlayers[0]}`);
    url.searchParams.append("rtsAi", `2:${aiPlayers[1]}`);
    if (autoSpectator) url.searchParams.set("rtsAutoSpectator", "1");
  } else {
    url.searchParams.set("rtsName", "Interact");
    url.searchParams.set("rtsAi", `2:${opponent}`);
  }
  url.searchParams.set("rtsStart", "1");
  if (map && map !== "Chokes") url.searchParams.set("rtsMap", map);
  url.searchParams.set("interact", "game");
  url.searchParams.set("rtsNoAutoPointerLock", "1");
  return url.href;
}

function safeToken(value: unknown, fallback: string, maxLength: number) {
  const token = String(value || "").replace(/[^A-Za-z0-9_-]/g, "_").slice(0, maxLength);
  return token || fallback;
}

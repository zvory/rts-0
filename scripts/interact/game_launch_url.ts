import { DEFAULT_LAB_MAP } from "./session_defaults.ts";

export function interactLaunchUrl({
  mode,
  baseUrl,
  room,
  map,
  opponent,
  spectate,
  renderer,
  seed,
  scenario,
  devScenario,
}: {
  mode: "lab" | "game" | "scenario";
  baseUrl: string;
  room: string;
  map: string;
  opponent: string;
  spectate?: readonly string[] | null;
  renderer: string;
  seed: string;
  scenario: string;
  devScenario: { id: string; unit: string; count: number; blocker: string; case: string };
}) {
  if (mode === "lab") {
    const url = new URL("/lab", baseUrl);
    url.searchParams.set("room", room);
    url.searchParams.set("map", safeToken(map, DEFAULT_LAB_MAP, 48));
    if (seed) url.searchParams.set("seed", seed);
    if (scenario) url.searchParams.set("scenario", safeToken(scenario, "blank", 48));
    if (renderer === "babylon") url.searchParams.set("rtsRenderer", "babylon");
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
    if (renderer === "babylon") url.searchParams.set("rtsRenderer", "babylon");
    url.searchParams.set("interact", "scenario");
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
  } else {
    url.searchParams.set("rtsName", "Interact");
    url.searchParams.set("rtsAi", `2:${opponent}`);
  }
  url.searchParams.set("rtsStart", "1");
  if (map && map !== "Default") url.searchParams.set("rtsMap", map);
  if (renderer === "babylon") url.searchParams.set("rtsRenderer", "babylon");
  url.searchParams.set("interact", "game");
  url.searchParams.set("rtsNoAutoPointerLock", "1");
  return url.href;
}

function safeToken(value: unknown, fallback: string, maxLength: number) {
  const token = String(value || "").replace(/[^A-Za-z0-9_-]/g, "_").slice(0, maxLength);
  return token || fallback;
}

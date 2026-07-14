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
}: {
  mode: "lab" | "game";
  baseUrl: string;
  room: string;
  map: string;
  opponent: string;
  spectate?: readonly string[] | null;
  renderer: string;
  seed: string;
  scenario: string;
}) {
  if (mode === "lab") {
    const url = new URL("/lab", baseUrl);
    url.searchParams.set("room", room);
    url.searchParams.set("map", safeToken(map, "Default", 48));
    if (seed) url.searchParams.set("seed", seed);
    if (scenario) url.searchParams.set("scenario", safeToken(scenario, "blank", 48));
    if (renderer === "babylon") url.searchParams.set("rtsRenderer", "babylon");
    url.searchParams.set("interact", "lab");
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

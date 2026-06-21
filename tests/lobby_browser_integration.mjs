// Focused lobby-browser HTTP + join-flow coverage. Expects a running server; use
// `tests/run-all.sh` or start one with `cd server && cargo run` and set RTS_WS if needed.
import {
  addAi,
  closeClients,
  connectClient,
  createAssertions,
  readyPlayers,
  removeAi,
  sleep,
  uniqueRoom,
  URL as WS_URL,
} from "./team_harness.mjs";

const assertions = createAssertions();
const { ok } = assertions;

function lobbiesUrl() {
  const url = new URL(WS_URL);
  url.protocol = url.protocol === "wss:" ? "https:" : "http:";
  url.pathname = "/api/lobbies";
  url.search = "";
  return url;
}

async function createLobby(room) {
  return fetch(lobbiesUrl(), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ room }),
  });
}

async function lobbyRows() {
  const response = await fetch(lobbiesUrl(), { cache: "no-store" });
  ok(response.ok, `GET /api/lobbies returns ${response.status}`);
  return response.json();
}

async function waitForLobbyRow(room, predicate, label) {
  for (let i = 0; i < 30; i++) {
    const row = (await lobbyRows()).find((entry) => entry.room === room);
    if (row && predicate(row)) return row;
    await sleep(100);
  }
  throw new Error(`timeout waiting for lobby row: ${label}`);
}

async function waitForLobbyGone(room, label) {
  for (let i = 0; i < 50; i++) {
    const row = (await lobbyRows()).find((entry) => entry.room === room);
    if (!row) return true;
    await sleep(100);
  }
  throw new Error(`timeout waiting for lobby row removal: ${label}`);
}

async function waitForCreateAvailable(room, label) {
  for (let i = 0; i < 70; i++) {
    const response = await createLobby(room);
    if (response.status === 201) return response;
    if (response.status !== 409) {
      throw new Error(`unexpected create status ${response.status} while waiting for ${label}`);
    }
    await sleep(100);
  }
  throw new Error(`timeout waiting for lobby create availability: ${label}`);
}

async function main() {
  const abandonedRoom = uniqueRoom("browser-abandoned");
  const abandonedName = `alex's ${abandonedRoom}`;
  const abandoned = await createLobby(abandonedName);
  ok(abandoned.status === 201, `POST /api/lobbies accepts apostrophe names (${abandoned.status})`);
  const abandonedDuplicate = await createLobby(abandonedName);
  ok(abandonedDuplicate.status === 409,
    `pending create lease keeps duplicate protection (${abandonedDuplicate.status})`);
  const recreatedAbandoned = await waitForCreateAvailable(
    abandonedName,
    "abandoned pending create lease",
  );
  ok(recreatedAbandoned.status === 201,
    `abandoned pending create lease releases the name (${recreatedAbandoned.status})`);

  const room = uniqueRoom("browser-flow");

  const created = await createLobby(`  ${room}  `);
  ok(created.status === 201, `POST /api/lobbies creates a lobby (${created.status})`);
  const payload = await created.json();
  ok(payload.room === room, `create trims and returns the room name (${payload.room})`);

  const duplicate = await createLobby(room);
  ok(duplicate.status === 409, `duplicate create rejects instead of joining (${duplicate.status})`);
  const duplicatePayload = await duplicate.json();
  ok(
    duplicatePayload.error === "Lobby name is already in use.",
    `duplicate create returns an inline-safe error (${duplicatePayload.error})`,
  );

  const A = await connectClient("browser-A");
  A.send({ t: "join", name: "Alpha", room });
  const lobbyA = await A.waitFor((msg) => msg.t === "lobby", 3000, "browser active join lobby");
  ok(lobbyA.players.find((player) => player.id === A.playerId)?.isSpectator === false,
    "browser open-row join enters as an active player");

  await waitForLobbyRow(
    room,
    (row) => row.joinState === "open" && row.occupiedSlots === 1,
    "open row after active join",
  );

  const B = await connectClient("browser-B");
  const C = await connectClient("browser-C");
  const D = await connectClient("browser-D");
  B.send({ t: "join", name: "Bravo", room });
  C.send({ t: "join", name: "Charlie", room });
  D.send({ t: "join", name: "Delta", room });
  await A.waitFor((msg) => msg.t === "lobby" && msg.players.filter((p) => !p.isSpectator).length === 4,
    3000,
    "browser full lobby",
  );

  const fullRow = await waitForLobbyRow(
    room,
    (row) => row.joinState === "fullSpectatorOnly" && row.occupiedSlots === 4,
    "full spectator-only row",
  );
  ok(fullRow.spectatorCount === 0, "full waiting row does not count spectators as active slots");

  const Observer = await connectClient("browser-observer");
  Observer.send({ t: "join", name: "Observer", room, spectator: true });
  const lobbyWithObserver = await A.waitFor(
    (msg) => msg.t === "lobby" && msg.players.find((p) => p.id === Observer.playerId)?.isSpectator,
    3000,
    "browser spectator join",
  );
  ok(lobbyWithObserver.players.find((player) => player.id === Observer.playerId)?.teamId === 0,
    "browser full-row join enters as a spectator");

  const spectatorRow = await waitForLobbyRow(
    room,
    (row) => row.joinState === "fullSpectatorOnly" && row.spectatorCount === 1,
    "spectator count after full-row join",
  );
  ok(spectatorRow.occupiedSlots === 4, "spectator browser join leaves active slots full");

  closeClients(A, B, C, D, Observer);
  await sleep(200);

  const lifecycleRoom = uniqueRoom("browser-state");
  const Host = await connectClient("browser-host");
  Host.send({ t: "join", name: "Host", room: lifecycleRoom });
  await Host.waitFor((msg) => msg.t === "lobby" && msg.room === lifecycleRoom, 3000, "browser lifecycle host join");

  const hostRow = await waitForLobbyRow(
    lifecycleRoom,
    (row) => row.joinState === "open" && row.hostName === "Host" && row.occupiedSlots === 1,
    "host row after join",
  );
  ok(hostRow.spectatorCount === 0, "host-only row starts with no spectators");

  Host.send({ t: "selectMap", map: "No Terrain" });
  await Host.waitFor((msg) => msg.t === "lobby" && msg.map === "No Terrain", 3000, "browser map selection");
  await waitForLobbyRow(
    lifecycleRoom,
    (row) => row.map === "No Terrain",
    "map refresh after host selection",
  );

  const [firstAi] = await addAi(Host);
  await waitForLobbyRow(
    lifecycleRoom,
    (row) => row.occupiedSlots === 2 && row.joinState === "open",
    "AI add refreshes active slots",
  );
  await removeAi(Host, firstAi.id);
  await waitForLobbyRow(
    lifecycleRoom,
    (row) => row.occupiedSlots === 1 && row.joinState === "open",
    "AI remove refreshes active slots",
  );

  await addAi(Host);
  await readyPlayers([Host]);
  Host.send({ t: "start" });
  await Host.waitFor((msg) => msg.t === "matchCountdown", 3000, "browser countdown start");
  await waitForLobbyRow(
    lifecycleRoom,
    (row) => row.joinState === "starting" && row.occupiedSlots === 2,
    "countdown row is starting",
  );
  await Host.waitFor((msg) => msg.t === "start", 6000, "browser lifecycle match start");
  const inGameRow = await waitForLobbyRow(
    lifecycleRoom,
    (row) => row.joinState === "inGame" && row.map === "No Terrain",
    "in-game row is spectatable",
  );
  ok(inGameRow.occupiedSlots === 2, "in-game browser row keeps active human plus AI slots");

  const StaleJoiner = await connectClient("browser-stale");
  StaleJoiner.send({ t: "join", name: "Stale", room: lifecycleRoom });
  const rejected = await StaleJoiner.waitFor(
    (msg) => msg.t === "error" && /progress/.test(msg.msg || ""),
    3000,
    "stale in-game row join rejection",
  );
  ok(!!rejected, "active stale in-game browser join is rejected by server authority");
  const fallbackRoom = uniqueRoom("browser-after-reject");
  StaleJoiner.send({ t: "join", name: "Stale", room: fallbackRoom });
  await StaleJoiner.waitFor(
    (msg) => msg.t === "lobby" && msg.room === fallbackRoom,
    3000,
    "active late join socket can still join another room",
  );

  const SpectatorJoiner = await connectClient("browser-live-spectator");
  SpectatorJoiner.send({ t: "join", name: "Spectator", room: lifecycleRoom, spectator: true });
  const spectatorStart = await SpectatorJoiner.waitFor(
    (msg) => msg.t === "start" && msg.spectator,
    3000,
    "late spectator receives live start",
  );
  ok(spectatorStart.playerId === SpectatorJoiner.playerId, "late spectator start is stamped with connection id");
  ok(!spectatorStart.predictionBuildId && Number(spectatorStart.predictionVersion || 0) === 0,
    "late spectator start disables prediction metadata");
  const spectatorSnapshot = await SpectatorJoiner.waitFor(
    (msg) => msg.t === "snapshot",
    3000,
    "late spectator receives live snapshot",
  );
  ok(spectatorSnapshot.playerResources?.length >= 2,
    "late spectator snapshot uses spectator resource projection");
  const liveSpectatorRow = await waitForLobbyRow(
    lifecycleRoom,
    (row) => row.joinState === "inGame" && row.spectatorCount >= 1,
    "in-game spectator count refreshes",
  );
  ok(liveSpectatorRow.occupiedSlots === 2, "late spectator does not change active match slots");

  closeClients(Host, StaleJoiner, SpectatorJoiner);
  await waitForLobbyGone(lifecycleRoom, "empty room cleanup hides browser row");
  const recreatedLifecycle = await createLobby(lifecycleRoom);
  ok(recreatedLifecycle.status === 201,
    `empty public room has no reconnect grace and releases the name (${recreatedLifecycle.status})`);

  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
}

main().catch((error) => {
  console.log("TEST ERROR:", error.message);
  process.exit(2);
});

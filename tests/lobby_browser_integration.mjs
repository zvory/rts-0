// Focused lobby-browser HTTP + join-flow coverage. Expects a running server; use
// `tests/run-all.sh` or start one with `cd server && cargo run` and set RTS_WS if needed.
import {
  closeClients,
  connectClient,
  createAssertions,
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
  for (let i = 0; i < 20; i++) {
    const row = (await lobbyRows()).find((entry) => entry.room === room);
    if (row && predicate(row)) return row;
    await sleep(100);
  }
  throw new Error(`timeout waiting for lobby row: ${label}`);
}

async function main() {
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
  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
}

main().catch((error) => {
  console.log("TEST ERROR:", error.message);
  process.exit(2);
});

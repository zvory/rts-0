// Focused lobby faction selection test. Requires a running server.
// Env: RTS_WS (default ws://127.0.0.1:8081/ws).
import { DEFAULT_FACTION_ID } from "../client/src/protocol.js";
import {
  assertCountdownProtocol,
  assertStartProtocol,
  closeClients,
  connectClient,
  createAssertions,
  readyPlayers,
  startMatch,
  uniqueRoom,
} from "./team_harness.mjs";

const ROOM = uniqueRoom("faction-itest");
const assertions = createAssertions();
const { ok } = assertions;

(async () => {
  const A = await connectClient("A");
  A.send({ t: "join", name: "Alpha", room: ROOM });
  await A.waitFor((m) => m.t === "lobby" && m.players.length === 1, 3000, "A lobby");

  const B = await connectClient("B");
  B.send({ t: "join", name: "Bravo", room: ROOM });
  const lobby = await A.waitFor((m) => m.t === "lobby" && m.players.length === 2, 3000, "two-player lobby");
  ok(lobby.players.every((p) => p.factionId === DEFAULT_FACTION_ID),
    `lobby defaults both seats to ${DEFAULT_FACTION_ID}`);

  B.send({ t: "setFaction", factionId: "ekat" });
  const changed = await A.waitFor(
    (m) => m.t === "lobby" && m.players.find((p) => p.id === B.playerId)?.factionId === "ekat",
    3000,
    "B faction selection",
  );
  ok(changed.players.find((p) => p.id === A.playerId)?.factionId === DEFAULT_FACTION_ID,
    `A remains ${DEFAULT_FACTION_ID}`);
  ok(changed.players.find((p) => p.id === B.playerId)?.factionId === "ekat",
    "B selected Ekaterina faction");

  A.send({ t: "setFaction", factionId: "phase2_empty_fixture" });
  await new Promise((resolve) => setTimeout(resolve, 200));
  const afterInvalid = A.msgs.filter((m) => m.t === "lobby").at(-1);
  ok(afterInvalid.players.find((p) => p.id === A.playerId)?.factionId === DEFAULT_FACTION_ID,
    "fixture faction request is ignored for normal lobby players");

  await readyPlayers([A, B]);
  const { countdowns, starts } = await startMatch(A, [A, B]);
  assertCountdownProtocol(ok, countdowns[0]);
  assertCountdownProtocol(ok, countdowns[1]);
  assertStartProtocol(ok, starts[0], { playerId: A.playerId, expectedPlayers: 2, spectator: false });
  assertStartProtocol(ok, starts[1], { playerId: B.playerId, expectedPlayers: 2, spectator: false });
  ok(starts[0].players.find((p) => p.id === A.playerId)?.factionId === DEFAULT_FACTION_ID,
    `start carries A ${DEFAULT_FACTION_ID}`);
  ok(starts[0].players.find((p) => p.id === B.playerId)?.factionId === "ekat",
    "start carries B ekat");

  closeClients(A, B);
  await new Promise((resolve) => setTimeout(resolve, 200));
  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
})().catch((e) => { console.log("TEST ERROR:", e.message); process.exit(2); });

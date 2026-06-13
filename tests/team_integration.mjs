// Team-game integration harness baseline.
//
// This suite does not start the server. Start one first with `cd server && cargo run`, or run via
// `tests/run-all.sh`, which boots a private server and sets RTS_WS for all live Node suites.
// Override the endpoint with RTS_WS (default ws://127.0.0.1:8081/ws).
//
// Phase 0 asserts today's FFA-compatible lobby/start/score fields through reusable helpers. Later
// phases should add 1v2, 1v3, and 2v2 scenarios here by reusing the same room setup helpers and
// extending the protocol assertions with `teamId` checks.
import {
  addAi,
  assertCountdownProtocol,
  assertDistinctStartTiles,
  assertLobbyProtocol,
  assertScoreProtocol,
  assertStartProtocol,
  closeClients,
  connectClient,
  createAssertions,
  readyPlayers,
  removeAi,
  sleep,
  startMatch,
  uniqueRoom,
  waitForGameOver,
} from "./team_harness.mjs";

const ROOM = uniqueRoom("team-itest");
const assertions = createAssertions();
const { ok } = assertions;

async function main() {
  const A = await connectClient("A");
  A.send({ t: "join", name: "Alpha", room: ROOM });
  const lobbyA = await A.waitFor((msg) => msg.t === "lobby", 3000, "A lobby");
  assertLobbyProtocol(ok, lobbyA, { expectedPlayers: 1, hostId: A.playerId });

  const B = await connectClient("B");
  B.send({ t: "join", name: "Bravo", room: ROOM });
  const lobbyB = await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 2, 3000, "two-human lobby");
  assertLobbyProtocol(ok, lobbyB, { expectedPlayers: 2, hostId: A.playerId });
  ok(!("teamId" in lobbyB.players[0]), "TEAMS PHASE 0: lobby players do not expose teamId yet");

  const [ai] = await addAi(A);
  const lobbyWithAi = A.msgs.filter((msg) => msg.t === "lobby").at(-1);
  assertLobbyProtocol(ok, lobbyWithAi, { expectedPlayers: 3, hostId: A.playerId });
  ok(ai?.isAi === true && ai.ready === true, "AI seating helper adds a ready computer player");
  const lobbyAfterRemove = await removeAi(A, ai.id);
  assertLobbyProtocol(ok, lobbyAfterRemove, { expectedPlayers: 2, hostId: A.playerId });

  await readyPlayers([A, B]);
  const { countdowns, starts } = await startMatch(A, [A, B]);
  for (const countdown of countdowns) assertCountdownProtocol(ok, countdown);

  const [startA, startB] = starts;
  assertStartProtocol(ok, startA, { playerId: A.playerId, expectedPlayers: 2, spectator: false });
  assertStartProtocol(ok, startB, { playerId: B.playerId, expectedPlayers: 2, spectator: false });
  assertDistinctStartTiles(ok, startA);
  ok(!startA.players.some((player) => "teamId" in player), "TEAMS PHASE 0: start players do not expose teamId yet");

  const snap = await A.waitFor((msg) => msg.t === "snapshot" && msg.entities.length > 0, 3000, "A first snapshot");
  ok(snap.entities.some((entity) => entity.owner === A.playerId && entity.kind === "city_centre"),
    "snapshot wait helper observes the host City Centre");

  B.send({ t: "giveUp" });
  const [overA, overB] = await waitForGameOver([A, B]);
  assertScoreProtocol(ok, overA, { expectedPlayers: 2 });
  ok(overA.you === "won" && overB.you === "lost", `game-over helper waits for both verdicts (${overA.you}/${overB.you})`);
  ok(!overA.scores.some((score) => "teamId" in score), "TEAMS PHASE 0: scores do not expose teamId yet");

  closeClients(A, B);
  await sleep(200);
  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
}

main().catch((error) => {
  console.log("TEST ERROR:", error.message);
  process.exit(2);
});

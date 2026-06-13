// Scripted lobby team setup integration coverage. This suite expects a running server; use
// `tests/run-all.sh` or start one with `cd server && cargo run` and set RTS_WS if needed.
import {
  addAiToTeam,
  assertCountdownProtocol,
  assertLobbyProtocol,
  assertStartProtocol,
  closeClients,
  connectClient,
  createAssertions,
  readyPlayers,
  setTeamPreset,
  sleep,
  startMatch,
  startMatchDirect,
  uniqueRoom,
} from "./team_harness.mjs";

const assertions = createAssertions();
const { ok } = assertions;

function teamsFor(start) {
  return start.players.map((player) => player.teamId);
}

async function joinNamed(tag, room, name = tag, opts = {}) {
  const client = await connectClient(tag);
  client.send({ t: "join", name, room, ...opts });
  await client.waitFor((msg) => msg.t === "lobby", 3000, `${tag} lobby`);
  return client;
}

async function assertPresetStarts({ preset, aiTeams, expectedTeams }) {
  const room = uniqueRoom(`team-${preset}`);
  const A = await joinNamed(`${preset}-A`, room, "Alpha");
  const lobby = await setTeamPreset(A, preset);
  assertLobbyProtocol(ok, lobby, { expectedPlayers: 1, hostId: A.playerId });

  for (const teamId of aiTeams) {
    await addAiToTeam(A, teamId);
  }
  await readyPlayers([A]);
  const { countdowns, starts } = await startMatch(A, [A]);
  assertCountdownProtocol(ok, countdowns[0]);
  const [start] = starts;
  assertStartProtocol(ok, start, {
    playerId: A.playerId,
    expectedPlayers: expectedTeams.length,
    spectator: false,
  });
  ok(JSON.stringify(teamsFor(start)) === JSON.stringify(expectedTeams),
    `${preset}: start teams are ${expectedTeams.join(",")} (${teamsFor(start).join(",")})`);
  closeClients(A);
}

async function defaultFfaReportsUniqueTeams() {
  const room = uniqueRoom("team-ffa");
  const A = await joinNamed("ffa-A", room, "Alpha");
  const B = await joinNamed("ffa-B", room, "Bravo");
  const lobby = await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 2, 3000, "FFA lobby");
  assertLobbyProtocol(ok, lobby, { expectedPlayers: 2, hostId: A.playerId });
  ok(lobby.teamPreset === "ffa", `default preset is ffa (${lobby.teamPreset})`);
  ok(lobby.players.every((player) => player.teamId === player.id),
    "default FFA assigns every active player a unique singleton team");
  closeClients(A, B);
}

async function soloStartsWithoutForcedAi() {
  const room = uniqueRoom("team-solo");
  const A = await joinNamed("solo-A", room, "Alpha");
  await setTeamPreset(A, "solo");
  await readyPlayers([A]);
  const [start] = await startMatchDirect(A, [A]);
  assertStartProtocol(ok, start, { playerId: A.playerId, expectedPlayers: 1, spectator: false });
  ok(start.players[0]?.id === A.playerId && start.players[0]?.teamId === 1,
    `solo starts only the host on Team 1 (${JSON.stringify(start.players)})`);
  ok(!start.players.some((player) => player.isAi), "solo does not force an AI opponent");
  closeClients(A);
}

async function twoVsTwoStartsWithHumanAndAis() {
  const room = uniqueRoom("team-2v2");
  const A = await joinNamed("2v2-A", room, "Alpha");
  const B = await joinNamed("2v2-B", room, "Bravo");
  await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 2, 3000, "2v2 human lobby");
  await setTeamPreset(A, "2v2");
  await addAiToTeam(A, 2);
  await addAiToTeam(A, 2);
  await readyPlayers([A, B]);
  const { starts } = await startMatch(A, [A, B]);
  const [start] = starts;
  assertStartProtocol(ok, start, { playerId: A.playerId, expectedPlayers: 4, spectator: false });
  ok(JSON.stringify(teamsFor(start)) === JSON.stringify([1, 1, 2, 2]),
    `2v2 seats host+human on Team 1 and AIs on Team 2 (${teamsFor(start).join(",")})`);
  closeClients(A, B);
}

async function hostOnlyAndInvalidMutationsAreIgnored() {
  const room = uniqueRoom("team-invalid");
  const A = await joinNamed("invalid-A", room, "Alpha");
  const B = await joinNamed("invalid-B", room, "Bravo");
  const C = await joinNamed("invalid-C", room, "Spectator", { spectator: true });
  const lobby = await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 3, 3000, "invalid lobby");
  assertLobbyProtocol(ok, lobby, { expectedPlayers: 3, hostId: A.playerId });

  B.send({ t: "setTeamPreset", preset: "1v2" });
  B.send({ t: "setTeam", id: A.playerId, teamId: 2 });
  B.send({ t: "addAi", teamId: 2 });
  await sleep(400);
  let last = A.msgs.filter((msg) => msg.t === "lobby").at(-1);
  ok(last.teamPreset === "ffa" && last.players.length === 3,
    "non-host team preset, team assignment, and addAi(teamId) are ignored");
  ok(last.players.find((player) => player.id === A.playerId)?.teamId === A.playerId,
    "non-host setTeam did not move the host");

  await setTeamPreset(A, "solo");
  await readyPlayers([A, B], { timeoutMs: 3000 }).catch(() => null);
  last = A.msgs.filter((msg) => msg.t === "lobby").at(-1);
  ok(last.canStart === false, "invalid solo composition with two active players leaves canStart false");

  A.send({ t: "setTeam", id: B.playerId, teamId: 0 });
  A.send({ t: "setTeam", id: 999999, teamId: 1 });
  A.send({ t: "setTeam", id: C.playerId, teamId: 1 });
  await sleep(400);
  last = A.msgs.filter((msg) => msg.t === "lobby").at(-1);
  ok(last.players.find((player) => player.id === B.playerId)?.teamId !== 0,
    "team id 0 assignment is ignored");
  ok(last.players.find((player) => player.id === C.playerId)?.teamId === 0,
    "spectator assignment is ignored and spectator remains team 0");

  await setTeamPreset(A, "1v2");
  A.send({ t: "setTeam", id: B.playerId, teamId: 1 });
  A.send({ t: "addAi", teamId: 0 });
  A.send({ t: "addAi", teamId: 1 });
  await sleep(400);
  last = A.msgs.filter((msg) => msg.t === "lobby").at(-1);
  ok(last.players.find((player) => player.id === B.playerId)?.teamId === 2,
    "overfull preset move to Team 1 is ignored in 1v2");
  ok(last.players.length === 3 && !last.players.some((player) => player.isAi),
    "invalid and overfull addAi(teamId) requests are ignored");
  closeClients(A, B, C);
}

async function main() {
  await defaultFfaReportsUniqueTeams();
  await soloStartsWithoutForcedAi();
  await assertPresetStarts({ preset: "1v2", aiTeams: [2, 2], expectedTeams: [1, 2, 2] });
  await assertPresetStarts({ preset: "1v3", aiTeams: [2, 2, 2], expectedTeams: [1, 2, 2, 2] });
  await twoVsTwoStartsWithHumanAndAis();
  await hostOnlyAndInvalidMutationsAreIgnored();

  await sleep(200);
  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
}

main().catch((error) => {
  console.log("TEST ERROR:", error.message);
  process.exit(2);
});

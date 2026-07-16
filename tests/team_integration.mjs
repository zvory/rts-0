// Scripted lobby team setup integration coverage. This suite expects a running server; use
// `tests/run-all.sh` or start one with `cd server && cargo run` and set RTS_WS if needed.
import {
  addAiToTeam,
  assertLobbyProtocol,
  assertScoreProtocol,
  assertStartProtocol,
  closeClients,
  connectClient,
  createAssertions,
  readyPlayers,
  setTeam,
  sleep,
  startMatch,
  startMatchDirect,
  uniqueRoom,
  waitForGameOver,
} from "./team_harness.mjs";

const assertions = createAssertions();
const { ok } = assertions;

function teamsFor(start) {
  return start.players.map((player) => player.teamId);
}

function startDistanceSq(a, b) {
  return (a.startTileX - b.startTileX) ** 2 + (a.startTileY - b.startTileY) ** 2;
}

function tileVisible(snapshot, start, tileX, tileY) {
  const index = tileY * start.map.width + tileX;
  return snapshot.visibleTiles?.[index] === 1;
}

function entityTile(start, entity) {
  const tileSize = start.map.tileSize;
  return {
    x: Math.floor(entity.x / tileSize),
    y: Math.floor(entity.y / tileSize),
  };
}

async function joinNamed(tag, room, name = tag, opts = {}) {
  const client = await connectClient(tag);
  client.send({ t: "join", name, room, ...opts });
  await client.waitFor((msg) => msg.t === "lobby", 3000, `${tag} lobby`);
  return client;
}

async function selectFourPlayerMap(client, label) {
  client.send({ t: "selectMap", map: "Chokes" });
  await client.waitFor(
    (msg) => msg.t === "lobby" && msg.map === "Chokes",
    3000,
    `${label} four-player map selection`,
  );
}

async function defaultLobbyAssignsEmptyTeams() {
  const room = uniqueRoom("team-empty");
  const A = await joinNamed("ffa-A", room, "Alpha");
  const B = await joinNamed("ffa-B", room, "Bravo");
  const lobby = await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 2, 3000, "empty-team lobby");
  assertLobbyProtocol(ok, lobby, { expectedPlayers: 2, hostId: A.playerId });
  ok(lobby.teamPreset === "custom", `lobby reports custom team slots (${lobby.teamPreset})`);
  ok(lobby.players.find((player) => player.id === A.playerId)?.teamId === 1,
    "first active player is assigned Team 1");
  ok(lobby.players.find((player) => player.id === B.playerId)?.teamId === 2,
    "new player joins the first empty team");
  closeClients(A, B);
}

async function soloStartsWithoutForcedAi() {
  const room = uniqueRoom("team-solo");
  const A = await joinNamed("solo-A", room, "Alpha");
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
  await selectFourPlayerMap(A, "2v2");
  const B = await joinNamed("2v2-B", room, "Bravo");
  await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 2, 3000, "2v2 human lobby");
  await setTeam(A, B.playerId, 1);
  await addAiToTeam(A, 2);
  await addAiToTeam(A, 2);
  await readyPlayers([A, B]);
  const { starts } = await startMatch(A, [A, B]);
  const [start] = starts;
  assertStartProtocol(ok, start, { playerId: A.playerId, expectedPlayers: 4, spectator: false });
  ok(JSON.stringify(teamsFor(start)) === JSON.stringify([1, 1, 2, 2]),
    `2v2 seats host+human on Team 1 and AIs on Team 2 (${teamsFor(start).join(",")})`);
  const teamOneDistance = startDistanceSq(start.players[0], start.players[1]);
  const teamTwoDistance = startDistanceSq(start.players[2], start.players[3]);
  const oppositeCornerBaseline = 10000;
  ok(teamOneDistance < oppositeCornerBaseline && teamTwoDistance < oppositeCornerBaseline,
    `2v2 teammates spawn near each other (${teamOneDistance}, ${teamTwoDistance})`);
  closeClients(A, B);
}

async function alliedSnapshotUsesSharedVisionWithoutSharedControl() {
  const room = uniqueRoom("team-shared-vision");
  const A = await joinNamed("vision-A", room, "Alpha");
  await selectFourPlayerMap(A, "shared vision");
  const B = await joinNamed("vision-B", room, "Bravo");
  await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 2, 3000, "vision human lobby");
  await setTeam(A, B.playerId, 1);
  await addAiToTeam(A, 2);
  await addAiToTeam(A, 2);
  await readyPlayers([A, B]);
  const { starts } = await startMatch(A, [A, B]);
  const startA = starts[0];
  const allyMeta = startA.players.find((player) => player.id === B.playerId);
  ok(allyMeta?.teamId === 1, `shared vision setup has B on Team 1 (${allyMeta?.teamId})`);

  const [snapA, snapB] = await Promise.all([
    A.waitFor((msg) => msg.t === "snapshot" && msg.entities.some((e) => e.owner === B.playerId), 8000, "A sees allied entities"),
    B.waitFor((msg) => msg.t === "snapshot" && msg.entities.some((e) => e.owner === B.playerId), 8000, "B sees own entities"),
  ]);
  const allyInA = snapA.entities.filter((e) => e.owner === B.playerId && e.kind !== "steel" && e.kind !== "oil");
  const ownInB = snapB.entities.filter((e) => e.owner === B.playerId && e.kind !== "steel" && e.kind !== "oil");
  ok(allyInA.length > 0 && ownInB.length > 0,
    `team snapshot carries allied read-only entities (${allyInA.length}/${ownInB.length})`);

  const alliedWorker = allyInA.find((e) => e.kind === "worker");
  const alliedWorkerHp = alliedWorker?.hp;
  if (alliedWorker) {
    const tile = entityTile(startA, alliedWorker);
    ok(tileVisible(snapA, startA, tile.x, tile.y),
      `A's visibleTiles include an allied worker tile (${tile.x},${tile.y})`);
    A.command({ c: "stop", units: [alliedWorker.id] });
    const laterB = await B.waitFor((msg) => msg.t === "snapshot" && msg.tick > snapB.tick + 8, 3000, "post allied stop snapshots");
    const workerAfter = laterB.entities.find((e) => e.id === alliedWorker.id);
    ok(workerAfter?.hp === alliedWorkerHp,
      "malicious command against allied visible entity stays a no-op on the ally's unit");
  } else {
    ok(false, "shared vision setup exposed no allied worker for command-authority probe");
  }
  closeClients(A, B);
}

async function alliedAttackTargetCommandIsIgnored() {
  const room = uniqueRoom("team-target");
  const A = await joinNamed("target-A", room, "Alpha");
  await selectFourPlayerMap(A, "allied attack");
  const B = await joinNamed("target-B", room, "Bravo");
  await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 2, 3000, "target human lobby");
  await setTeam(A, B.playerId, 1);
  await addAiToTeam(A, 2);
  await addAiToTeam(A, 2);
  await readyPlayers([A, B]);
  await startMatch(A, [A, B]);
  const [snapA, snapB] = await Promise.all([
    A.waitFor((msg) => msg.t === "snapshot" && msg.entities.some((e) => e.owner === A.playerId && e.kind === "worker"), 8000, "A units"),
    B.waitFor((msg) => msg.t === "snapshot" && msg.entities.some((e) => e.owner === B.playerId && e.kind === "worker"), 8000, "B units"),
  ]);
  const attacker = snapA.entities.find((e) => e.owner === A.playerId && e.kind === "worker");
  const alliedTarget = snapB.entities.find((e) => e.owner === B.playerId && e.kind === "worker");
  const alliedHpBefore = alliedTarget.hp;

  A.command({ c: "attack", units: [attacker.id], target: alliedTarget.id });
  await B.waitFor((msg) => msg.t === "snapshot" && msg.tick > snapB.tick + 8, 3000, "post allied attack snapshots");
  const latest = A.msgs.filter((msg) => msg.t === "snapshot").at(-1);
  const attackerAfter = latest?.entities.find((e) => e.id === attacker.id);
  ok(attackerAfter && attackerAfter.targetId !== alliedTarget.id,
    "malicious attack command cannot assign an allied entity id as a hostile target");
  const latestB = B.msgs.filter((msg) => msg.t === "snapshot").at(-1);
  const alliedAfter = latestB?.entities.find((e) => e.id === alliedTarget.id);
  ok(alliedAfter && alliedAfter.hp === alliedHpBefore,
    `malicious allied attack command did not damage allied target (${alliedHpBefore} -> ${alliedAfter?.hp})`);
  closeClients(A, B);
}

async function twoVsTwoResolvesByTeam() {
  const room = uniqueRoom("team-victory");
  const A = await joinNamed("victory-A", room, "Alpha");
  await selectFourPlayerMap(A, "team victory");
  const B = await joinNamed("victory-B", room, "Bravo");
  const C = await joinNamed("victory-C", room, "Charlie");
  const D = await joinNamed("victory-D", room, "Delta");
  await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 4, 3000, "team victory lobby");
  await setTeam(A, B.playerId, 1);
  await setTeam(A, C.playerId, 2);
  await setTeam(A, D.playerId, 2);
  await readyPlayers([A, B, C, D]);
  await startMatch(A, [A, B, C, D]);

  B.send({ t: "giveUp" });
  await sleep(500);
  ok(!B.msgs.some((msg) => msg.t === "gameOver"),
    "2v2: defeated player on living team does not receive early gameOver");

  C.send({ t: "giveUp" });
  await sleep(300);
  ok(!C.msgs.some((msg) => msg.t === "gameOver"),
    "2v2: first defeated player on enemy team waits until the whole team is defeated");

  D.send({ t: "giveUp" });
  const overs = await waitForGameOver([A, B, C, D], { timeoutMs: 5000 });
  for (const over of overs) assertScoreProtocol(ok, over, { expectedPlayers: 4 });
  ok(overs.every((over) => over.winnerTeamId === 1),
    `2v2: final gameOver carries winning team id 1 (${overs.map((over) => over.winnerTeamId).join(",")})`);
  ok(overs[0].winnerId === A.playerId,
    `2v2: winnerId compatibility uses first living winning seat (${overs[0].winnerId}/${A.playerId})`);
  ok(overs[0].you === "won" && overs[1].you === "won" && overs[2].you === "lost" && overs[3].you === "lost",
    `2v2: teammates share final verdicts (${overs.map((over) => over.you).join(",")})`);
  closeClients(A, B, C, D);
}

async function hostOnlyAndInvalidMutationsAreIgnored() {
  const room = uniqueRoom("team-invalid");
  const A = await joinNamed("invalid-A", room, "Alpha");
  const B = await joinNamed("invalid-B", room, "Bravo");
  const C = await joinNamed("invalid-C", room, "Spectator", { spectator: true });
  const lobby = await A.waitFor((msg) => msg.t === "lobby" && msg.players.length === 3, 3000, "invalid lobby");
  assertLobbyProtocol(ok, lobby, { expectedPlayers: 3, hostId: A.playerId });

  B.send({ t: "setTeam", id: A.playerId, teamId: 2 });
  B.send({ t: "addAi", teamId: 2 });
  await sleep(400);
  let last = A.msgs.filter((msg) => msg.t === "lobby").at(-1);
  ok(last.teamPreset === "custom" && last.players.length === 3,
    "non-host team assignment and addAi(teamId) are ignored");
  ok(last.players.find((player) => player.id === A.playerId)?.teamId === 1,
    "non-host setTeam did not move the host");

  A.send({ t: "setTeam", id: B.playerId, teamId: 0 });
  A.send({ t: "setTeam", id: B.playerId, teamId: 5 });
  A.send({ t: "setTeam", id: 999999, teamId: 1 });
  A.send({ t: "setTeam", id: C.playerId, teamId: 1 });
  await sleep(400);
  last = A.msgs.filter((msg) => msg.t === "lobby").at(-1);
  ok(last.players.find((player) => player.id === B.playerId)?.teamId !== 0,
    "team id 0 assignment is ignored");
  ok(last.players.find((player) => player.id === C.playerId)?.teamId === 0,
    "spectator assignment is ignored and spectator remains team 0");

  await setTeam(A, B.playerId, 1);
  A.send({ t: "addAi", teamId: 0 });
  A.send({ t: "addAi", teamId: 5 });
  await sleep(400);
  last = A.msgs.filter((msg) => msg.t === "lobby").at(-1);
  ok(last.players.find((player) => player.id === B.playerId)?.teamId === 1,
    "host can move active players onto the same team");
  ok(last.players.length === 3 && !last.players.some((player) => player.isAi),
    "invalid addAi(teamId) requests are ignored");

  B.send({ t: "setSpectator", id: A.playerId, spectator: true });
  await sleep(300);
  last = A.msgs.filter((msg) => msg.t === "lobby").at(-1);
  ok(!last.players.find((player) => player.id === A.playerId)?.isSpectator,
    "non-host targeted spectator assignment is ignored");

  A.send({ t: "setSpectator", id: B.playerId, spectator: true });
  last = await A.waitFor((msg) =>
    msg.t === "lobby" && msg.players.find((player) => player.id === B.playerId)?.isSpectator,
    3000,
    "host spectator assignment",
  );
  ok(last.players.find((player) => player.id === B.playerId)?.teamId === 0,
    "host can move a human player into spectators");
  closeClients(A, B, C);
}

async function main() {
  await defaultLobbyAssignsEmptyTeams();
  await soloStartsWithoutForcedAi();
  await twoVsTwoStartsWithHumanAndAis();
  await alliedSnapshotUsesSharedVisionWithoutSharedControl();
  await alliedAttackTargetCommandIsIgnored();
  await twoVsTwoResolvesByTeam();
  await hostOnlyAndInvalidMutationsAreIgnored();

  await sleep(200);
  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
}

main().catch((error) => {
  console.log("TEST ERROR:", error.message);
  process.exit(2);
});

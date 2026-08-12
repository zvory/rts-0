// End-to-end server integration test — no dependencies (uses Node's built-in global
// WebSocket, Node >= 22). Drives two clients through the full lifecycle and asserts the
// authoritative pipeline: lobby/host/colors -> ready/canStart -> start (map + per-player
// payload) -> initial economy -> fog of war -> extractor income -> train -> give-up/win.
//
// Usage: start the server (`cd server && cargo run`), then `node tests/server_integration.mjs`.
// Override the endpoint with RTS_WS (default ws://127.0.0.1:8081/ws).
import { COMPACT_SNAPSHOT_VERSION, DEFAULT_FACTION_ID } from "../client/src/protocol.js";
import {
  assertCountdownProtocol,
  assertDistinctStartTiles,
  assertScoreProtocol,
  assertStartProtocol,
  closeClients,
  connectClient,
  createAssertions,
  readyPlayers,
  sleep,
  startMatch,
  uniqueRoom,
} from "./team_harness.mjs";

const ROOM = uniqueRoom("itest");
const assertions = createAssertions();
const { ok } = assertions;

(async () => {
  const A = await connectClient("A");
  ok(A.playerId != null, `A got welcome playerId=${A.playerId}`);
  A.send({ t: "join", name: "Alpha", room: ROOM });
  const initialLobby = await A.waitFor((m) => m.t === "lobby", 3000, "A lobby");
  ok(Array.isArray(initialLobby.maps) && initialLobby.maps.length >= 1,
     `lobby exposes at least one selectable map (${initialLobby.maps?.map(m=>m.name).join(",")})`);
  ok(initialLobby.maps.some(m => m.name === initialLobby.map),
     `selected map is present in selectable maps (${initialLobby.map})`);

  const B = await connectClient("B");
  B.send({ t: "join", name: "Bravo", room: ROOM });

  const C = await connectClient("C");
  C.send({ t: "join", name: "Observer", room: ROOM, spectator: true });

  const lob = await A.waitFor((m) => m.t === "lobby" && m.players.length === 3, 3000, "A lobby(3)");
  ok(lob.players.length === 3, `lobby shows 2 players and 1 spectator: ${lob.players.map((p) => p.name).join(", ")}`);
  ok(lob.hostId === A.playerId, `host is A (${lob.hostId})`);
  ok(lob.players.every((p) => /^#/.test(p.color)), `players have hex colors: ${lob.players.map((p) => p.color).join(",")}`);
  ok(lob.players.every((p) => p.factionId === DEFAULT_FACTION_ID), `lobby defaults all seats to ${DEFAULT_FACTION_ID}`);
  ok(lob.players.find((p) => p.id === C.playerId)?.isSpectator === true, "lobby marks C as spectator");

  await readyPlayers([A, B]);
  ok(true, "canStart after both ready");

  const { countdowns, starts } = await startMatch(A, [A, B, C]);
  const [countdownA, countdownB, countdownC] = countdowns;
  assertCountdownProtocol(ok, countdownA);
  ok(countdownB.durationMs === 3000 && countdownC.durationMs === 3000, "all lobby participants receive countdown");

  const [startA, startB, startC] = starts;
  assertStartProtocol(ok, startA, { playerId: A.playerId, expectedPlayers: 2, spectator: false });
  assertStartProtocol(ok, startB, { playerId: B.playerId, expectedPlayers: 2, spectator: false });
  assertStartProtocol(ok, startC, { playerId: C.playerId, expectedPlayers: 2, spectator: true });
  assertDistinctStartTiles(ok, startA);
  ok(startA.players.length === 2, `start lists 2 players`);
  ok(startA.playerId === A.playerId && startB.playerId === B.playerId, `each start carries own playerId`);
  ok(startC.playerId === C.playerId && startC.spectator === true, `spectator start carries observer id and flag`);
  ok(startC.players.length === 2 && !startC.players.some((p) => p.id === C.playerId), "spectator is not seated in start players");
  ok(startA.players.every((p) => p.factionId === DEFAULT_FACTION_ID), `start players carry ${DEFAULT_FACTION_ID} factionId`);
  const a = startA.players.find((p) => p.id === A.playerId);
  const b = startA.players.find((p) => p.id === B.playerId);
  ok(a && b && (a.startTileX !== b.startTileX || a.startTileY !== b.startTileY),
     `players start at distinct tiles A=(${a?.startTileX},${a?.startTileY}) B=(${b?.startTileX},${b?.startTileY})`);

  const snap = await A.waitFor((m) => m.t === "snapshot" && m.entities.length > 0, 3000, "A snapshot");
  ok(A.rawSnapshots.some((m) => m.t === "snapshot" && m.v === COMPACT_SNAPSHOT_VERSION && Array.isArray(m.s) && Array.isArray(m.e)),
     `server sends compact v${COMPACT_SNAPSHOT_VERSION} snapshot frames`);
  ok(snap.steel === 75, `A starts with 75 steel (${snap.steel})`);
  ok(snap.oil === 0, `A starts with 0 oil (${snap.oil})`);
  ok(snap.supplyCap === 300, `A intrinsic supply cap = 300 (${snap.supplyCap})`);
  ok(snap.supplyUsed === 5, `A supply used = 5 (${snap.supplyUsed})`);
  ok(snap.netStatus?.predictionVersion === 1 && snap.netStatus?.lastSimConsumedClientSeq === 0,
     `prediction ACK fields start at zero (v=${snap.netStatus?.predictionVersion}, seq=${snap.netStatus?.lastSimConsumedClientSeq})`);
  const mine = snap.entities.filter((e) => e.owner === A.playerId);
  ok(mine.filter((e) => e.kind === "resource_depot").length === 1, `A owns 1 Resource Depot`);
  const workers = mine.filter((e) => e.kind === "worker");
  ok(workers.length === 1, `A owns 1 Engineer (${workers.length})`);
  const steelMines = mine.filter((e) => e.kind === "steel_mine");
  const completedSteelMines = steelMines.filter((e) => e.buildProgress == null);
  ok(
    completedSteelMines.length === 6 && steelMines.length >= 6,
    `A owns 6 completed starting Steel Mines plus any automatic scaffold (${completedSteelMines.length}/${steelMines.length})`,
  );
  const steelNodes = startA.map.resources.filter((e) => e.kind === "steel");
  ok(steelNodes.length > 0 && typeof steelNodes[0].id === "number", `start lists neutral steel nodes (${steelNodes.length})`);
  ok(!snap.entities.some((e) => e.kind === "steel" || e.kind === "oil"), "snapshot omits static resource entities");
  ok(!snap.entities.some((e) => e.owner === B.playerId), `FOG: A cannot see B at start`);

  const specSnap = await C.waitFor(
    (m) => m.t === "snapshot" && m.entities.some((e) => e.owner === A.playerId) && m.entities.some((e) => e.owner === B.playerId),
    3000,
    "C spectator union-fog snapshot",
  );
  ok(specSnap.steel === 0 && specSnap.oil === 0 && specSnap.supplyUsed === 0 && specSnap.supplyCap === 0,
     `SPECTATOR: observer has no personal economy (${specSnap.steel}/${specSnap.oil}/${specSnap.supplyUsed}/${specSnap.supplyCap})`);
  ok(Array.isArray(specSnap.playerResources) && specSnap.playerResources.length === 2,
     `SPECTATOR: observer sees all player resources (${specSnap.playerResources?.length})`);
  ok(!("predictionVersion" in (specSnap.netStatus || {})),
     "SPECTATOR: observer snapshots do not carry prediction ACK metadata");
  ok(!specSnap.entities.some((e) => e.owner === C.playerId),
     "SPECTATOR: observer owns no entities");

  A.send({ t: "pauseGame" });
  const pausedStates = await Promise.all([A, B, C].map((client) =>
    client.waitFor((m) => m.t === "livePauseState" && m.paused === true && !m.resumeCountdown,
      3000, `${client.tag} live pause`)
  ));
  ok(pausedStates.every((state) => state.canUnpause === true),
     "LIVE PAUSE: players and spectator can request resume");
  await sleep(100);
  const frozenTick = A.lastSnapshot?.tick;
  B.send({ t: "unpauseGame" });
  const resumeStates = await Promise.all([A, B, C].map((client) =>
    client.waitFor((m) => m.t === "livePauseState" && m.paused === true && m.resumeCountdown,
      3000, `${client.tag} resume countdown`)
  ));
  ok(resumeStates.every((state) =>
    state.canUnpause !== true &&
    state.resumeCountdown.durationMs === 3000 &&
    state.resumeCountdown.words.join(" ") === "Drei! Zwei! Eins!"),
  "LIVE PAUSE: every recipient receives the synchronized three-second resume countdown");
  await sleep(1000);
  ok(A.lastSnapshot?.tick === frozenTick,
     `LIVE PAUSE: simulation remains frozen during countdown (tick ${frozenTick})`);
  const resumedStates = await Promise.all([A, B, C].map((client) =>
    client.waitFor((m) => m.t === "livePauseState" && m.paused === false,
      4000, `${client.tag} resumed state`)
  ));
  ok(resumedStates.every((state) => !state.resumeCountdown),
     "LIVE PAUSE: every recipient receives authoritative resumed state after countdown");
  await A.waitFor((m) => m.t === "snapshot" && m.tick > frozenTick, 3000, "post-resume snapshot");
  ok(true, "LIVE PAUSE: simulation advances after the resume countdown");

  let peak = snap.steel;
  for (let i = 0; i < 30; i++) {
    await sleep(500);
    if (A.lastSnapshot) {
      peak = Math.max(peak, A.lastSnapshot.steel);
      if (A.lastSnapshot.steel > 75) break;
    }
  }
  ok(peak > 75, `EXTRACTORS: starting Steel Mines produced steel (peak=${peak})`);

  A.command({ c: "train", building: mine.find((e) => e.kind === "resource_depot").id, unit: "worker" });
  await sleep(1200);
  // Mining income can fully offset the worker cost while the private test server advances faster
  // than wall-clock time. The accelerated 5ms test tick can also finish the Worker before this
  // snapshot, so either active production or the completed unit is a stable acceptance check.
  const resourceDepot = A.lastSnapshot.entities.find((e) => e.kind === "resource_depot" && e.owner === A.playerId);
  const workerCountAfterTrain = A.lastSnapshot.entities.filter(
    (e) => e.kind === "worker" && e.owner === A.playerId,
  ).length;
  ok(
    resourceDepot && (
      resourceDepot.prodKind === "worker" ||
      (resourceDepot.prodQueue || 0) >= 1 ||
      workerCountAfterTrain > workers.length
    ),
    `TRAIN: Resource Depot accepted production (queue=${resourceDepot?.prodQueue}, workers=${workerCountAfterTrain})`,
  );
  ok(A.lastSnapshot?.netStatus?.lastSimConsumedClientSeq >= 1,
     `TRAIN: server acknowledged consumed clientSeq ${A.lastSnapshot?.netStatus?.lastSimConsumedClientSeq}`);

  B.send({ t: "giveUp" });
  const overB = await B.waitFor((m) => m.t === "gameOver", 4000, "B gameOver after giveUp");
  ok(overB.you === "lost", `GIVE UP: B sees defeat after giving up (you=${overB.you})`);
  const over = await A.waitFor((m) => m.t === "gameOver", 4000, "A gameOver");
  const overC = await C.waitFor((m) => m.t === "gameOver", 4000, "C gameOver");
  ok(over.you === "won", `WIN: A wins after B gives up (you=${over.you})`);
  ok(overC.you === "draw" && overC.winnerId === A.playerId, `SPECTATOR: observer sees neutral result with winner (${overC.you}/${overC.winnerId})`);
  assertScoreProtocol(ok, over, { expectedPlayers: 2 });
  ok(Array.isArray(over.scores) && over.scores.length === 2, `SCORE: gameOver lists both players (${over.scores?.length})`);
  const aScore = over.scores?.find((s) => s.id === A.playerId);
  const bScore = over.scores?.find((s) => s.id === B.playerId);
  ok(aScore && aScore.unitScore >= 50 && aScore.structureScore >= 500, `SCORE: A has unit/structure value (${aScore?.unitScore}/${aScore?.structureScore})`);
  ok(bScore && bScore.unitsLost >= 1 && bScore.buildingsLost >= 7, `SCORE: surrendered B losses recorded (${bScore?.unitsLost}/${bScore?.buildingsLost})`);

  const replayStartA = await A.waitFor((m) => m.t === "start" && m.replay, 4000, "A replay start");
  const replayStartB = await B.waitFor((m) => m.t === "start" && m.replay, 4000, "B replay start");
  const replayStartC = await C.waitFor((m) => m.t === "start" && m.replay, 4000, "C replay start");
  ok(replayStartA.tick === 0 && replayStartB.tick === 0 && replayStartC.tick === 0,
     `REPLAY: post-match replay starts at tick 0 (${replayStartA.tick}/${replayStartB.tick}/${replayStartC.tick})`);
  ok(replayStartA.spectator === true && replayStartB.spectator === true && replayStartC.spectator === true,
     "REPLAY: connected humans become replay spectators");
  ok(replayStartA.replay.durationTicks > 0,
     `REPLAY: artifact metadata carries duration (${replayStartA.replay.durationTicks})`);
  const roomTimeStateA = await A.waitFor((m) => m.t === "roomTimeState" && m.currentTick === 0, 4000, "A replay state");
  const roomTimeStateB = await B.waitFor((m) => m.t === "roomTimeState" && m.currentTick === 0, 4000, "B replay state");
  ok(roomTimeStateA.speed === 2 && roomTimeStateB.speed === 2,
     `REPLAY: playback defaults to 2x (${roomTimeStateA.speed}/${roomTimeStateB.speed})`);

  await A.waitNext((m) => m.t === "snapshot" && m.tick >= 2, 4000, "advancing replay snapshot");
  A.send({ t: "seekRoomTimeTo", tick: 0 });
  const [seekStartedA, seekStartedB, seekStartedC] = await Promise.all([
    A.waitFor((m) => m.t === "roomTimeSeekStarted", 4000, "A replay seek started"),
    B.waitFor((m) => m.t === "roomTimeSeekStarted", 4000, "B replay seek started"),
    C.waitFor((m) => m.t === "roomTimeSeekStarted", 4000, "C replay seek started"),
  ]);
  ok(
    [seekStartedA, seekStartedB, seekStartedC].every((m) =>
      m.controllerId === A.playerId && m.fromTick >= 2 && m.targetTick === 0),
    `REPLAY: accepted seek immediately broadcasts controller/distance to every viewer (${seekStartedA.fromTick}->${seekStartedA.targetTick})`,
  );

  A.send({ t: "returnToLobby" });
  const lobbyAfterReplay = await A.waitFor((m) => m.t === "lobby" && m.players.length === 3, 4000, "lobby after replay");
  ok(lobbyAfterReplay.players.every((p) => p.isSpectator || !p.ready),
     "REMATCH: returning from replay clears active players' ready flags");
  ok(lobbyAfterReplay.canStart === false, "REMATCH: lobby cannot immediately restart until players ready again");

  closeClients(A, B, C);
  await sleep(200);
  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
})().catch((e) => { console.log("TEST ERROR:", e.message); process.exit(2); });

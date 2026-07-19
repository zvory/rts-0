// Focused lobby-browser HTTP + join-flow coverage. Expects a running server; use
// `tests/run-all.sh` or start one with `cd server && cargo run` and set RTS_WS if needed.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
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
const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const LAB_SETUP_FIXTURE = path.join(REPO_ROOT, "server/assets/lab-scenarios/lategame.json");
const SAFE_REPLAY_LOBBY_ROW_KEYS = [
  "createdAtUnixMs",
  "hostName",
  "joinState",
  "kind",
  "map",
  "maxSlots",
  "occupiedSlots",
  "phase",
  "room",
  "spectatorCount",
].sort().join(",");

function lobbiesUrl() {
  const url = new URL(WS_URL);
  url.protocol = url.protocol === "wss:" ? "https:" : "http:";
  url.pathname = "/api/lobbies";
  url.search = "";
  return url;
}

function devReplayLobbyUrl(replay) {
  const url = lobbiesUrl();
  url.pathname = "/dev/replay-lobby";
  url.searchParams.set("replay", replay);
  return url;
}

function installReplayLobbyFixture() {
  const name = `replaylobby-${process.pid}-${Date.now()}`;
  const dir = path.join(REPO_ROOT, "server", "target", "selfplay-artifacts", name);
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, "replay.json"),
    `${JSON.stringify(schemaThreeReplayFixture(), null, 2)}\n`,
  );
  return { name, dir };
}

function schemaThreeReplayFixture() {
  const setup = JSON.parse(fs.readFileSync(LAB_SETUP_FIXTURE, "utf8"));
  const checkpoint = JSON.parse(setup.checkpointPayload);
  const players = checkpoint.players.map((player) => ({
    id: player.id,
    team_id: player.teamId,
    faction_id: player.factionId,
    name: player.name,
    color: player.color,
    is_ai: !!player.isAi,
  }));
  return {
    artifactSchemaVersion: 3,
    serverBuildSha: "live-node-fixture",
    mapName: setup.map.name,
    mapSchemaVersion: setup.map.schemaVersion,
    mapContentHash: setup.map.contentHash,
    seed: setup.seed,
    playerLoadouts: checkpoint.startingLoadouts,
    players,
    startState: {
      mapName: setup.map.name,
      mapSchemaVersion: setup.map.schemaVersion,
      mapContentHash: setup.map.contentHash,
      seed: setup.seed,
      checkpointPayload: setup.checkpointPayload,
    },
    durationTicks: 120,
    commandLog: [],
    winnerId: null,
    winnerTeamId: null,
    finalScores: checkpoint.players.map((player) => ({
      id: player.id,
      teamId: player.teamId,
      name: player.name,
      color: player.color,
      unitScore: player.score?.unitScore ?? 0,
      structureScore: player.score?.structureScore ?? 0,
      unitsKilled: player.score?.unitsKilled ?? 0,
      unitsLost: player.score?.unitsLost ?? 0,
      buildingsKilled: player.score?.buildingsKilled ?? 0,
      buildingsLost: player.score?.buildingsLost ?? 0,
    })),
  };
}

async function createLobby(room) {
  return fetch(lobbiesUrl(), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ room }),
  });
}

async function createDevReplayLobby(replay) {
  return fetch(devReplayLobbyUrl(replay), { method: "POST" });
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

async function expectNoLobbyRow(room, label) {
  const rows = await lobbyRows();
  ok(!rows.some((entry) => entry.room === room), label);
}

async function main() {
  const abandonedRoom = uniqueRoom("browser-abandoned");
  const abandonedName = `alex's ${abandonedRoom}`;
  const abandoned = await createLobby(abandonedName);
  ok(abandoned.status === 201, `POST /api/lobbies accepts apostrophe names (${abandoned.status})`);
  const abandonedDuplicate = await createLobby(abandonedName);
  ok(abandonedDuplicate.status === 201,
    `pending create lease gets an available numbered name (${abandonedDuplicate.status})`);
  const abandonedDuplicatePayload = await abandonedDuplicate.json();
  ok(abandonedDuplicatePayload.room === `${abandonedName} 2`,
    `duplicate pending create returns its numbered room (${abandonedDuplicatePayload.room})`);
  // Empty pending reservations are intentionally hidden from GET /api/lobbies, so wait beyond
  // the server's five-second lease before checking that the original name can be reclaimed.
  await sleep(5500);
  const recreatedAbandoned = await createLobby(abandonedName);
  ok(recreatedAbandoned.status === 201,
    `abandoned pending create lease releases the name (${recreatedAbandoned.status})`);
  const recreatedAbandonedPayload = await recreatedAbandoned.json();
  ok(recreatedAbandonedPayload.room === abandonedName,
    `released pending create can reclaim its original name (${recreatedAbandonedPayload.room})`);

  const labRoom = `__lab__:${uniqueRoom("browser-lab")}:map=Chokes:seed=321`;
  const LabViewer = await connectClient("browser-lab");
  LabViewer.send({ t: "join", name: "Lab", room: labRoom, spectator: true });
  await LabViewer.waitFor((msg) => msg.t === "start" && msg.lab, 3000, "internal lab start");
  await expectNoLobbyRow(labRoom, "occupied internal lab room stays out of the public browser");
  closeClients(LabViewer);
  await sleep(200);
  await expectNoLobbyRow(labRoom, "empty internal lab room does not leak into the public browser");

  const replayFixture = installReplayLobbyFixture();
  const replayCreate = await createDevReplayLobby(replayFixture.name);
  ok(replayCreate.status === 201,
    `dev replay-lobby helper creates a replay staging room (${replayCreate.status})`);
  const replayPayload = await replayCreate.json();
  const replayRoom = replayPayload.room;
  ok(/^__match_replay__:[0-9a-f]+$/.test(replayRoom),
    `dev replay-lobby helper returns a match-history-style room (${replayRoom})`);
  await expectNoLobbyRow(replayRoom, "empty replay staging room stays out of the public browser");

  const ReplayHost = await connectClient("browser-replay-host");
  ReplayHost.send({ t: "join", name: "Archivist", room: replayRoom });
  const hostReplayLobby = await ReplayHost.waitFor(
    (msg) => msg.t === "lobby" && msg.kind === "replay" && msg.room === replayRoom,
    3000,
    "replay staging host lobby",
  );
  ok(hostReplayLobby.hostId === ReplayHost.playerId, "first replay staging viewer becomes host");
  ok(hostReplayLobby.canStart, "replay staging host can start immediately");
  ok(hostReplayLobby.players.length === 1 && hostReplayLobby.players[0].isSpectator,
    "replay staging lobby admits the first viewer as spectator only");
  ok(hostReplayLobby.maps.length === 0, "replay staging lobby does not expose map selection");

  const replayRow = await waitForLobbyRow(
    replayRoom,
    (row) => row.kind === "replay" &&
      row.hostName === "Archivist" &&
      row.joinState === "fullSpectatorOnly" &&
      row.occupiedSlots === 0 &&
      row.maxSlots === 0 &&
      row.spectatorCount === 1,
    "replay staging browser row",
  );
  ok(Object.keys(replayRow).sort().join(",") === SAFE_REPLAY_LOBBY_ROW_KEYS,
    "replay browser row exposes only safe lobby metadata");
  const replaySetup = JSON.parse(fs.readFileSync(LAB_SETUP_FIXTURE, "utf8"));
  ok(replayRow.map === replaySetup.map.name, "replay browser row shows the artifact map name");

  const ReplayGuest = await connectClient("browser-replay-guest");
  ReplayGuest.send({ t: "join", name: "Viewer", room: replayRoom, spectator: true });
  await ReplayHost.waitFor(
    (msg) => msg.t === "lobby" && msg.kind === "replay" && msg.players.length === 2,
    3000,
    "replay staging host sees second viewer",
  );
  const guestReplayLobby = await ReplayGuest.waitFor(
    (msg) => msg.t === "lobby" && msg.kind === "replay" && msg.players.length === 2,
    3000,
    "replay staging guest lobby",
  );
  ok(guestReplayLobby.players.every((player) => player.isSpectator),
    "replay staging lobby keeps every occupant as spectator");
  await waitForLobbyRow(
    replayRoom,
    (row) => row.kind === "replay" && row.spectatorCount === 2 && row.occupiedSlots === 0,
    "replay staging browser row with two viewers",
  );

  ReplayHost.send({ t: "start" });
  const [replayStartHost, replayStartGuest] = await Promise.all([
    ReplayHost.waitFor((msg) => msg.t === "start" && msg.replay, 4000, "replay host start"),
    ReplayGuest.waitFor((msg) => msg.t === "start" && msg.replay, 4000, "replay guest start"),
  ]);
  ok(replayStartHost.spectator && replayStartGuest.spectator,
    "shared replay playback starts both staging viewers as spectators");
  ok(replayStartHost.players.length === 2 && replayStartGuest.players.length === 2,
    "shared replay playback carries the original active replay players");
  ok(replayStartHost.replay.durationTicks > 0,
    `shared replay playback carries replay metadata (${replayStartHost.replay.durationTicks} ticks)`);
  const [hostRoomTime, guestRoomTime] = await Promise.all([
    ReplayHost.waitFor((msg) => msg.t === "roomTimeState" && msg.currentTick === 0,
      4000,
      "replay host room-time state"),
    ReplayGuest.waitFor((msg) => msg.t === "roomTimeState" && msg.currentTick === 0,
      4000,
      "replay guest room-time state"),
  ]);
  ok(hostRoomTime.speed === 2 && guestRoomTime.speed === 2,
    "shared replay playback keeps the replay default speed");
  await waitForLobbyRow(
    replayRoom,
    (row) => row.kind === "replay" &&
      row.joinState === "inGame" &&
      row.map === replaySetup.map.name &&
      row.spectatorCount === 2,
    "active replay playback browser row",
  );

  const ReplayLate = await connectClient("browser-replay-late");
  ReplayLate.send({
    t: "join",
    name: "Late Viewer",
    room: replayRoom,
    spectator: true,
    replayOk: true,
  });
  const lateReplayStart = await ReplayLate.waitFor(
    (msg) => msg.t === "start" && msg.replay,
    4000,
    "late active replay join start",
  );
  const lateReplayState = await ReplayLate.waitFor(
    (msg) => msg.t === "roomTimeState",
    4000,
    "late active replay join room-time state",
  );
  const lateReplaySnapshot = await ReplayLate.waitFor(
    (msg) => msg.t === "snapshot",
    4000,
    "late active replay join current snapshot",
  );
  ok(lateReplayStart.spectator && lateReplayStart.replay.durationTicks > 0,
    "late active replay join enters as a replay spectator");
  ok(lateReplaySnapshot.tick >= lateReplayState.currentTick,
    `late active replay join receives a current snapshot (${lateReplaySnapshot.tick} >= ${lateReplayState.currentTick})`);
  await waitForLobbyRow(
    replayRoom,
    (row) => row.kind === "replay" && row.joinState === "inGame" && row.spectatorCount === 3,
    "active replay late-viewer count",
  );

  const guestInitialSnapshot = await ReplayGuest.waitFor(
    (msg) => msg.t === "snapshot",
    4000,
    "replay guest initial snapshot",
  );
  ReplayHost.send({ t: "returnToLobby" });
  const guestContinuedSnapshot = await ReplayGuest.waitNext(
    (msg) => msg.t === "snapshot" && msg.tick > guestInitialSnapshot.tick,
    4000,
    "replay guest snapshot after host leaves",
  );
  ok(guestContinuedSnapshot.tick > guestInitialSnapshot.tick,
    "leaving one replay viewer keeps playback alive for remaining viewers");
  closeClients(ReplayHost, ReplayGuest, ReplayLate);
  await sleep(200);

  const savedReplayRoom = `__replay_artifact__:${replayFixture.name}`;
  const SavedReplayViewer = await connectClient("browser-saved-replay");
  SavedReplayViewer.send({ t: "join", name: "Saved", room: savedReplayRoom, spectator: true });
  const savedReplayPrompt = await SavedReplayViewer.waitFor(
    (msg) => msg.t === "joinReplayPrompt" && msg.room === savedReplayRoom,
    3000,
    "saved replay artifact prompt",
  );
  ok(!!savedReplayPrompt, "saved replay artifacts still require explicit replay confirmation");
  await expectNoLobbyRow(savedReplayRoom, "saved replay artifact rooms stay out of the public browser");
  SavedReplayViewer.send({
    t: "join",
    name: "Saved",
    room: savedReplayRoom,
    spectator: true,
    replayOk: true,
  });
  const savedReplayStart = await SavedReplayViewer.waitFor(
    (msg) => msg.t === "start" && msg.replay,
    4000,
    "saved replay artifact playback",
  );
  ok(savedReplayStart.spectator && savedReplayStart.replay.durationTicks > 0,
    "saved replay artifacts still enter immediate replay playback after confirmation");
  closeClients(SavedReplayViewer);
  await sleep(200);
  await expectNoLobbyRow(savedReplayRoom, "saved replay artifact playback remains hidden from browser");

  const room = uniqueRoom("browser-flow");

  const created = await createLobby(`  ${room}  `);
  ok(created.status === 201, `POST /api/lobbies creates a lobby (${created.status})`);
  const payload = await created.json();
  ok(payload.room === room, `create trims and returns the room name (${payload.room})`);

  const duplicate = await createLobby(room);
  ok(duplicate.status === 201, `duplicate create reserves another lobby (${duplicate.status})`);
  const duplicatePayload = await duplicate.json();
  ok(
    duplicatePayload.room === `${room} 2`,
    `duplicate create returns the numbered lobby name (${duplicatePayload.room})`,
  );

  const A = await connectClient("browser-A");
  A.send({ t: "join", name: "Alpha", room });
  const lobbyA = await A.waitFor((msg) => msg.t === "lobby", 3000, "browser active join lobby");
  ok(lobbyA.players.find((player) => player.id === A.playerId)?.isSpectator === false,
    "browser open-row join enters as an active player");

  A.send({ t: "selectMap", map: "Chokes" });
  await A.waitFor((msg) => msg.t === "lobby" && msg.map === "Chokes", 3000, "browser four-seat map selection");

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

  Host.send({ t: "selectMap", map: "Chokes" });
  await Host.waitFor((msg) => msg.t === "lobby" && msg.map === "Chokes", 3000, "browser map selection");
  await waitForLobbyRow(
    lifecycleRoom,
    (row) => row.map === "Chokes",
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
  await Host.waitFor((msg) => msg.t === "start", 6000, "browser lifecycle match start");
  ok(!Host.msgs.some((msg) => msg.t === "matchCountdown"), "browser AI-assisted start skips countdown");
  const inGameRow = await waitForLobbyRow(
    lifecycleRoom,
    (row) => row.joinState === "inGame" && row.map === "Chokes",
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

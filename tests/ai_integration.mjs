// End-to-end test for the optional AI opponents — no dependencies (Node >= 22 built-in
// WebSocket). Drives a single host through the lobby AI controls and asserts:
//   - addAi seats a computer opponent (isAi=true, hex color, always ready, not the host)
//   - the room can start with only the human ready (AIs don't gate readiness)
//   - removeAi unseats an AI
//   - addAi is host-only (a non-host's addAi is ignored)
//   - a 1-human + 1-AI match skips the countdown, starts as a real 2-player match
//     (distinct start tiles), and stays live — the AI's actual build/attack behavior is verified
//     deterministically by the Rust unit test `game::tests::ai_builds_economy_and_attacks`.
//
// Usage: start the server (`cd server && cargo run`), then `node tests/ai_integration.mjs`.
// Override the endpoint with RTS_WS (default ws://127.0.0.1:8081/ws).
import {
  addAi,
  closeClients,
  connectClient,
  createAssertions,
  removeAi,
  sleep,
  startMatchDirect,
  uniqueRoom,
} from "./team_harness.mjs";
import { DEFAULT_FACTION_ID } from "../client/src/protocol.js";
import { DEFAULT_AI_PROFILE_ID } from "../client/src/lobby.js";

const ROOM = uniqueRoom("ai-itest");
const assertions = createAssertions();
const { ok } = assertions;

(async () => {
  const A = await connectClient("A");
  A.send({ t: "join", name: "Host", room: ROOM });
  const solo = await A.waitNext((m) => m.t === "lobby", 3000, "A lobby");
  ok(solo.players.length === 1 && solo.hostId === A.playerId, `host alone in lobby (host=${solo.hostId})`);

  // Add an AI opponent.
  const [ai] = await addAi(A);
  const withAi = A.msgs.filter((m) => m.t === "lobby").at(-1);
  const human = withAi.players.find((p) => p.id === A.playerId);
  ok(!!ai, "addAi seated a computer opponent");
  ok(ai && ai.isAi === true && human && human.isAi === false, "isAi flag distinguishes AI from human");
  ok(ai && /^#/.test(ai.color) && ai.color !== human.color, `AI has a distinct hex color (${ai?.color} vs ${human?.color})`);
  ok(ai && ai.ready === true, "AI is always ready");
  ok(ai && ai.id !== A.playerId, "AI got its own player id");
  ok(ai && ai.factionId === DEFAULT_FACTION_ID, `AI defaults to ${DEFAULT_FACTION_ID}`);
  ok(ai && ai.aiProfileId === DEFAULT_AI_PROFILE_ID, `AI defaults to ${DEFAULT_AI_PROFILE_ID}`);
  ok(ai && ai.name === "AI 2.1", `AI uses the default profile label as its name (${ai?.name})`);

  const lobbyCountBeforeTurtleSelection = A.msgs.filter((m) => m.t === "lobby").length;
  A.send({ t: "setAiProfile", id: ai.id, aiProfileId: "ai_turtle" });
  await sleep(400);
  const withTurtle = A.msgs.filter((m) => m.t === "lobby").at(-1);
  const turtleAi = withTurtle.players.find((p) => p.id === ai.id);
  ok(
    turtleAi && turtleAi.aiProfileId === DEFAULT_AI_PROFILE_ID,
    "player lobby rejects the internal Turtle AI profile",
  );
  ok(
    A.msgs.filter((m) => m.t === "lobby").length === lobbyCountBeforeTurtleSelection,
    "internal Turtle selection does not mutate the player lobby",
  );
  ok(turtleAi && turtleAi.name === "AI 2.1", `AI seat keeps the supported label (${turtleAi?.name})`);

  const lobbyCountBeforeUnsupportedSelection = A.msgs.filter((m) => m.t === "lobby").length;
  A.send({ t: "setAiProfile", id: ai.id, aiProfileId: "unsupported_profile" });
  await sleep(400);
  const afterUnsupportedSelection = A.msgs.filter((m) => m.t === "lobby").at(-1);
  const afterUnsupportedAi = afterUnsupportedSelection.players.find((p) => p.id === ai.id);
  ok(
    afterUnsupportedAi && afterUnsupportedAi.aiProfileId === DEFAULT_AI_PROFILE_ID,
    "unsupported AI profile selection leaves the existing profile unchanged",
  );
  ok(
    A.msgs.filter((m) => m.t === "lobby").length === lobbyCountBeforeUnsupportedSelection,
    "unsupported AI profile selection does not mutate the lobby",
  );

  // Send a hand-built future faction field. Phase 3 keeps addAi team-only, so the server may
  // ignore or reject the unsupported request, but it must never create a non-Kriegsia AI seat.
  A.send({ t: "selectMap", map: "Chokes" });
  await A.waitNext((m) => m.t === "lobby" && m.map === "Chokes", 3000, "four-seat map selection");
  const beforeHandBuiltIds = new Set(withAi.players.map((player) => player.id));
  A.send({ t: "addAi", factionId: "ekat" });
  await sleep(400);
  let withTwo = A.msgs.filter((m) => m.t === "lobby").at(-1);
  const handBuiltAi = withTwo.players.find((p) => p.isAi && !beforeHandBuiltIds.has(p.id));
  ok(
    !handBuiltAi || handBuiltAi.factionId === DEFAULT_FACTION_ID,
    "hand-built addAi factionId cannot create unsupported AI",
  );

  // Add a second AI, then remove the first — exercises both controls and the cap accounting.
  if (!handBuiltAi) {
    await addAi(A);
    withTwo = A.msgs.filter((m) => m.t === "lobby").at(-1);
  }
  ok(withTwo.players.filter((p) => p.isAi).length === 2, "second addAi seated a third player");
  const removed = await removeAi(A, ai.id);
  ok(!removed.players.some((p) => p.id === ai.id), "removeAi unseated the targeted AI");

  // A non-host cannot add AIs: B joins, sends addAi, and the player count must not change.
  const B = await connectClient("B");
  B.send({ t: "join", name: "Guest", room: ROOM });
  await A.waitNext((m) => m.t === "lobby" && m.players.length === 3, 3000, "lobby with B");
  B.send({ t: "addAi" });
  await sleep(400);
  const last = A.msgs.filter((m) => m.t === "lobby").at(-1);
  ok(last.players.length === 3, `non-host addAi ignored (still ${last.players.length} players)`);
  // Drop B so the start is a clean 1-human + 1-AI match.
  closeClients(B);
  await A.waitNext((m) => m.t === "lobby" && m.players.length === 2, 3000, "lobby after B leaves");

  // Start: only the host needs to be ready (the AI doesn't gate canStart).
  A.ready(true);
  await A.waitNext((m) => m.t === "lobby" && m.canStart, 3000, "canStart with just host ready");
  ok(true, "match can start with one human ready + one AI");

  const starts = await startMatchDirect(A, [A]);
  ok(!A.msgs.some((m) => m.t === "matchCountdown"), "1-human + AI start skips match countdown");
  const [start] = starts;
  ok(start.players.length === 2, `start lists 2 players (human + AI) (${start.players.length})`);
  ok(start.players.every((p) => p.factionId === DEFAULT_FACTION_ID), `start players carry ${DEFAULT_FACTION_ID}`);
  const sa = start.players.find((p) => p.id === A.playerId);
  const sai = start.players.find((p) => p.id !== A.playerId);
  ok(sa && sai && (sa.startTileX !== sai.startTileX || sa.startTileY !== sai.startTileY),
     `human and AI start at distinct tiles`);

  // The match is live: confirm the human keeps receiving snapshots and its City Centre is present. (The
  // AI's economy/attack behavior is covered by the Rust unit test — fog hides the AI base from
  // the human here, so there's nothing fast to observe over the wire beyond a running match.)
  const firstSnap = await A.waitFor((m) => m.t === "snapshot" && m.entities.length > 0, 3000, "first snapshot");
  ok(firstSnap.entities.some((e) => e.owner === A.playerId && e.kind === "city_centre"), "human owns its City Centre in-match");
  const tick0 = firstSnap.tick;
  const advancedSnap = await A.waitNext((m) => m.t === "snapshot" && m.tick > tick0, 3000, "advancing snapshot");
  ok(advancedSnap.tick > tick0, `match advancing (tick ${tick0} -> ${advancedSnap.tick})`);
  ok(!A.msgs.some((m) => m.t === "gameOver"), "match still running (not prematurely resolved)");

  closeClients(A);
  await sleep(200);
  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
})().catch((e) => { console.log("TEST ERROR:", e.message); process.exit(2); });

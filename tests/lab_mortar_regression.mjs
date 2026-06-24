// Focused lab mortar event regression. Requires a live server:
//   cd server && cargo run
//   node tests/lab_mortar_regression.mjs
// Override the endpoint with RTS_WS (default ws://127.0.0.1:8081/ws).
import { ABILITY, EVENT, KIND, cmd } from "../client/src/protocol.js";
import {
  closeClients,
  connectClient,
  createAssertions,
  uniqueRoom,
} from "./team_harness.mjs";

const TILE_SIZE = 32;
const PLAYER_ONE_ID = 1;
const PLAYER_TWO_ID = 2;
const SNAPSHOT_TIMEOUT_MS = 15_000;

const assertions = createAssertions();
const { ok } = assertions;

function tileCenter(tileX, tileY) {
  return {
    x: tileX * TILE_SIZE + TILE_SIZE / 2,
    y: tileY * TILE_SIZE + TILE_SIZE / 2,
  };
}

function requireCondition(condition, message) {
  ok(condition, message);
  if (!condition) throw new Error(message);
}

async function labRequest(client, requestId, op, label) {
  client.send({ t: "lab", requestId, op });
  const result = await client.waitFor(
    (msg) => msg.t === "labResult" && msg.requestId === requestId,
    5_000,
    `${label} labResult`,
  );
  requireCondition(result.ok, `${label} accepted${result.error ? `: ${result.error}` : ""}`);
  return result;
}

async function spawnLabEntity(client, requestId, { owner, kind, x, y }) {
  const result = await labRequest(
    client,
    requestId,
    { op: "spawnEntity", owner, kind, x, y, completed: true },
    `spawn ${kind} for P${owner}`,
  );
  const entityId = result.outcome?.entityId;
  requireCondition(Number.isInteger(entityId), `spawn ${kind} returns entityId`);
  return entityId;
}

async function exportScenario(client, requestId) {
  const result = await labRequest(
    client,
    requestId,
    { op: "exportScenario", name: "lab mortar regression" },
    "export scenario",
  );
  const scenario = result.outcome?.scenario;
  requireCondition(scenario && Array.isArray(scenario.entities), "export scenario returns entities");
  return scenario;
}

async function importScenario(client, requestId, scenario, oldMortarId) {
  const result = await labRequest(
    client,
    requestId,
    { op: "importScenario", scenario },
    "import deployed mortar scenario",
  );
  const remap = result.outcome?.entityIdMap || [];
  const entry = remap.find((item) => item.oldId === oldMortarId);
  requireCondition(Number.isInteger(entry?.newId), "scenario import remaps mortar entity id");
  return entry.newId;
}

async function prepareDeployedPlayerTwoMortar(client) {
  const mortarPosition = tileCenter(30, 30);
  const targetPosition = tileCenter(38, 30);
  const mortarId = await spawnLabEntity(client, 1, {
    owner: PLAYER_TWO_ID,
    kind: KIND.MORTAR_TEAM,
    ...mortarPosition,
  });
  await spawnLabEntity(client, 2, {
    owner: PLAYER_ONE_ID,
    kind: KIND.RIFLEMAN,
    ...targetPosition,
  });

  const scenario = await exportScenario(client, 3);
  const mortar = scenario.entities.find((entity) => entity.id === mortarId);
  requireCondition(mortar, "exported scenario includes spawned P2 mortar");
  mortar.setUp = true;
  mortar.setupTarget = { ...targetPosition };

  const remappedMortarId = await importScenario(client, 4, scenario, mortarId);
  return { mortarId: remappedMortarId, targetPosition };
}

async function waitForMortarLaunchBeforeImpact(client, mortarId, startMessageIndex) {
  let launch = null;
  let impact = null;
  let cursor = startMessageIndex;

  while (!impact) {
    let snapshot = null;
    while (cursor < client.msgs.length) {
      const msg = client.msgs[cursor++];
      if (msg.t === "snapshot") {
        snapshot = msg;
        break;
      }
    }
    if (!snapshot) {
      snapshot = await client.waitNext(
        (msg) => msg.t === "snapshot",
        SNAPSHOT_TIMEOUT_MS,
        "snapshot containing mortar launch/impact events",
      );
      cursor = client.msgs.length;
    }
    for (const event of snapshot.events || []) {
      if (event?.e === EVENT.MORTAR_LAUNCH && event.from === mortarId) {
        launch = { event, tick: snapshot.tick };
      } else if (event?.e === EVENT.MORTAR_IMPACT) {
        if (launch) {
          impact = { event, tick: snapshot.tick };
        }
      }
    }
  }

  requireCondition(launch, "P2 lab mortarLaunch reaches the lab operator");
  requireCondition(
    impact.tick > launch.tick,
    `mortarImpact arrives after mortarLaunch (launch=${launch.tick}, impact=${impact.tick})`,
  );
}

(async () => {
  const room = `__lab__:${uniqueRoom("lab-mortar")}:map=Default:seed=4242`;
  const operator = await connectClient("lab-mortar");
  try {
    operator.send({ t: "join", name: "Lab", room, spectator: true });
    const start = await operator.waitFor(
      (msg) => msg.t === "start" && msg.lab,
      3_000,
      "lab start payload",
    );
    requireCondition(start.lab?.vision?.mode === "fullWorld", "lab starts with full-world vision");
    await operator.waitFor(
      (msg) => msg.t === "snapshot" && msg.entities.length > 0,
      3_000,
      "initial lab snapshot",
    );

    const { mortarId, targetPosition } = await prepareDeployedPlayerTwoMortar(operator);
    const commandStartMessageIndex = operator.msgs.length;
    await labRequest(
      operator,
      5,
      {
        op: "issueCommandAs",
        playerId: PLAYER_TWO_ID,
        cmd: cmd.useAbility(ABILITY.MORTAR_FIRE, [mortarId], targetPosition.x, targetPosition.y),
        ignoreCommandLimits: false,
      },
      "issue P2 mortarFire",
    );
    await waitForMortarLaunchBeforeImpact(operator, mortarId, commandStartMessageIndex);
  } finally {
    closeClients(operator);
  }

  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
})().catch((error) => {
  console.log("TEST ERROR:", error.message);
  process.exit(2);
});

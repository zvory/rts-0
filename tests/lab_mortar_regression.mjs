// Focused lab mortar event regression. Requires a live server:
//   cd server && cargo run
//   node tests/lab_mortar_regression.mjs
// Override the endpoint with RTS_WS (default ws://127.0.0.1:8081/ws).
import { ABILITY, EVENT, KIND, cmd } from "../client/src/protocol.js";
import {
  closeClients,
  connectClient,
  createAssertions,
  sleep,
  uniqueRoom,
} from "./team_harness.mjs";

const TILE_SIZE = 32;
const PLAYER_ONE_ID = 1;
const PLAYER_TWO_ID = 2;
const SNAPSHOT_TIMEOUT_MS = 15_000;
const ATTEMPTS = 3;

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
  const checkpoint = checkpointPayload(scenario);
  requireCondition(
    Array.isArray(checkpoint.entities?.entities),
    "export scenario returns checkpoint entities",
  );
  return scenario;
}

function checkpointPayload(scenario) {
  requireCondition(
    scenario?.kind === "labCheckpointScenario" && typeof scenario.checkpointPayload === "string",
    "scenario uses checkpoint-backed lab shape",
  );
  return JSON.parse(scenario.checkpointPayload);
}

function writeCheckpointPayload(scenario, checkpoint) {
  scenario.checkpointPayload = JSON.stringify(checkpoint);
}

async function importScenario(client, requestId, scenario, oldMortarId) {
  const importStartMessageIndex = client.msgs.length;
  const result = await labRequest(
    client,
    requestId,
    { op: "importScenario", scenario },
    "import deployed mortar scenario",
  );
  const remap = result.outcome?.entityIdMap || [];
  const entry = remap.find((item) => item.oldId === oldMortarId);
  requireCondition(Number.isInteger(entry?.newId), "scenario import remaps mortar entity id");
  return { mortarId: entry.newId, importStartMessageIndex };
}

async function prepareDeployedPlayerTwoMortar(client) {
  const mortarPosition = tileCenter(30, 30);
  const targetPosition = tileCenter(38, 30);
  const mortarId = await spawnLabEntity(client, 1, {
    owner: PLAYER_TWO_ID,
    kind: KIND.MORTAR_TEAM,
    ...mortarPosition,
  });

  const scenario = await exportScenario(client, 3);
  const checkpoint = checkpointPayload(scenario);
  const mortar = checkpoint.entities.entities.find((entity) => entity.id === mortarId);
  requireCondition(mortar, "exported scenario includes spawned P2 mortar");
  const setupFacing = Math.atan2(targetPosition.y - mortar.pos_y, targetPosition.x - mortar.pos_x);
  mortar.combat.setup = "Deployed";
  mortar.combat.weapon_facing = setupFacing;
  mortar.combat.desired_weapon_facing = setupFacing;
  mortar.combat.emplacement_facing = setupFacing;
  if (mortar.movement) mortar.movement.facing = setupFacing;
  writeCheckpointPayload(scenario, checkpoint);

  const { mortarId: remappedMortarId, importStartMessageIndex } = await importScenario(
    client,
    4,
    scenario,
    mortarId,
  );
  await waitForRestoredMortarSnapshot(client, remappedMortarId, importStartMessageIndex);
  return { mortarId: remappedMortarId, targetPosition };
}

async function waitForRestoredMortarSnapshot(client, mortarId, startMessageIndex) {
  const restored = (msg) =>
    msg.t === "snapshot" &&
    msg.entities.some(
      (entity) =>
        entity.id === mortarId &&
        entity.owner === PLAYER_TWO_ID &&
        entity.kind === KIND.MORTAR_TEAM &&
        entity.setupState === "deployed",
    );
  const existing = client.msgs.slice(startMessageIndex).find(restored);
  if (existing) return existing;
  return client.waitNext(restored, 5_000, "post-import deployed mortar snapshot");
}

async function waitForMortarLaunchBeforeImpact(client, mortarId, startMessageIndex) {
  let launch = null;
  let impact = null;
  let cursor = startMessageIndex;
  const deadline = Date.now() + SNAPSHOT_TIMEOUT_MS;

  while (!impact && Date.now() < deadline) {
    while (cursor < client.msgs.length) {
      const msg = client.msgs[cursor++];
      if (msg.t !== "snapshot") continue;
      for (const event of msg.events || []) {
        if (event?.e === EVENT.MORTAR_LAUNCH && event.from === mortarId) {
          launch = { event, tick: msg.tick };
        } else if (event?.e === EVENT.MORTAR_IMPACT) {
          if (launch) {
            impact = { event, tick: msg.tick };
          }
        }
      }
    }
    if (!impact) await sleep(20);
  }

  if (!impact) {
    throw new Error("[lab-mortar] timeout waiting for snapshot containing mortar launch/impact events");
  }

  requireCondition(launch, "P2 lab mortarLaunch reaches the lab operator");
  requireCondition(
    impact.tick > launch.tick,
    `mortarImpact arrives after mortarLaunch (launch=${launch.tick}, impact=${impact.tick})`,
  );
}

async function runLabMortarRegressionAttempt(attempt) {
  const room = `__lab__:${uniqueRoom(`lab-mortar-${attempt}`)}:map=Chokes:seed=4242`;
  const operator = await connectClient("lab-mortar");
  try {
    operator.send({ t: "join", name: "Lab", room, spectator: true });
    const start = await operator.waitFor(
      (msg) => msg.t === "start" && msg.lab,
      3_000,
      "lab start payload",
    );
    requireCondition(start.lab?.vision?.mode === "all", "lab starts with all-team vision");
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
}

function retryableMortarTimeout(error) {
  return /snapshot containing mortar launch\/impact events/.test(error?.message || "");
}

(async () => {
  let lastError = null;
  for (let attempt = 1; attempt <= ATTEMPTS; attempt++) {
    try {
      await runLabMortarRegressionAttempt(attempt);
      lastError = null;
      break;
    } catch (error) {
      lastError = error;
      if (!retryableMortarTimeout(error) || attempt === ATTEMPTS) throw error;
      console.log(`[lab-mortar] retrying after transient event timeout (${attempt}/${ATTEMPTS})`);
      await sleep(1_000);
    }
  }
  if (lastError) throw lastError;

  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
})().catch((error) => {
  console.log("TEST ERROR:", error.message);
  process.exit(2);
});

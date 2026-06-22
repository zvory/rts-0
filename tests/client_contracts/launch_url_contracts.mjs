// tests/client_contracts/launch_url_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";

async function testDevWatchScenarioConfig() {
  const priorDocument = globalThis.document;
  const priorWindow = globalThis.window;
  globalThis.document = {
    getElementById: () => null,
  };
  globalThis.window = {
    location: new URL(
      "http://localhost/?watchScenario=1&id=vehicle_small_block_baseline&unit=scout_car&count=5",
    ),
    localStorage: { getItem: () => null },
  };
  try {
    const { devWatchConfig } = await import("../../client/src/bootstrap.js");
    let config = devWatchConfig();
    assert(config, "vehicle_small_block_baseline dev scenario should be recognized");
    assert(config.kind === "scenario", "dev scenario should set scenario kind");
    assert(
      config.room === "__dev_scenario__:vehicle_small_block_baseline:unit=scout_car:count=5",
      "dev scenario should auto-join the server scenario room",
    );

    globalThis.window.location = new URL(
      "http://localhost/?watchScenario=1&id=vehicle_small_block_baseline&unit=scout_car&count=5&blocker=machine_gunner",
    );
    config = devWatchConfig();
    assert(config, "vehicle_small_block_baseline blocker variant should be recognized");
    assert(
      config.room ===
        "__dev_scenario__:vehicle_small_block_baseline:unit=scout_car:count=5:blocker=machine_gunner",
      "dev scenario should include blocker variants in the server scenario room",
    );

    globalThis.window.location = new URL(
      "http://localhost/?watchScenario=1&id=tank_trap_pathing_matrix&unit=scout_car&count=1&case=enemy_vehicle_breach",
    );
    config = devWatchConfig();
    assert(config, "tank_trap_pathing_matrix case variant should be recognized");
    assert(
      config.room ===
        "__dev_scenario__:tank_trap_pathing_matrix:unit=scout_car:count=1:case=enemy_vehicle_breach",
      "dev scenario should include matrix case variants in the server scenario room",
    );

    globalThis.window.location = new URL(
      "http://localhost/?watchScenario=1&id=bad/scenario&unit=scout_car&count=5",
    );
    config = devWatchConfig();
    assert(config === null, "dev scenario parser should reject unsafe scenario ids");
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  }
}

async function testReplayArtifactLaunchConfig() {
  const priorDocument = globalThis.document;
  const priorWindow = globalThis.window;
  globalThis.document = {
    getElementById: () => null,
  };
  globalThis.window = {
    location: new URL("http://localhost/?replayArtifact=manual_worker_rush_latest"),
    localStorage: { getItem: () => null },
  };
  try {
    const { replayLaunchConfig } = await import("../../client/src/bootstrap.js");
    let config = replayLaunchConfig();
    assert(config, "replay artifact launch config should be recognized");
    assert(
      config.room === "__replay_artifact__:manual_worker_rush_latest",
      "replay artifact launch should auto-join the neutral replay artifact room",
    );

    globalThis.window.location = new URL("http://localhost/?replayArtifact=bad/artifact");
    config = replayLaunchConfig();
    assert(config === null, "replay artifact launch rejects unsafe artifact names");
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  }
}

async function testLabLaunchConfig() {
  const priorDocument = globalThis.document;
  const priorWindow = globalThis.window;
  globalThis.document = {
    getElementById: () => null,
  };
  globalThis.window = {
    location: new URL("http://localhost/lab?room=sandbox&map=low-econ&seed=1234"),
    localStorage: { getItem: () => null },
  };
  try {
    const { labLaunchConfig } = await import("../../client/src/bootstrap.js");
    let config = labLaunchConfig();
    assert(config, "lab route launch config should be recognized");
    assert(config.publicRoom === "sandbox", "lab launch keeps public room label");
    assert(config.map === "low-econ", "lab launch keeps map label");
    assert(
      config.room === "__lab__:sandbox:map=low-econ:seed=1234",
      "lab launch should build the server lab room id",
    );

    globalThis.window.location = new URL("http://localhost/lab?room=bad/room&map=bad map");
    config = labLaunchConfig();
    assert(
      config.room === "__lab__:default:map=Default",
      "lab launch falls back for unsafe room and map tokens without adding the default scenario to custom map URLs",
    );

    globalThis.window.location = new URL("http://localhost/lab");
    config = labLaunchConfig();
    assert(
      config.room === "__lab__:default:map=Default:scenario=lategame",
      "plain lab launch should request the default lategame scenario",
    );

    globalThis.window.location = new URL("http://localhost/lab?scenario=blank");
    config = labLaunchConfig();
    assert(
      config.room === "__lab__:default:map=Default:scenario=blank",
      "explicit lab scenario override should be preserved",
    );

    globalThis.window.location = new URL("http://localhost/?room=sandbox");
    assert(labLaunchConfig() === null, "non-lab route does not auto-join a lab");
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  }
}

await testDevWatchScenarioConfig();
await testReplayArtifactLaunchConfig();
await testLabLaunchConfig();

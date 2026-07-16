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
    const { devWatchConfig, snapshotStreamLaunchConfig } = await import("../../client/src/bootstrap.js");
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
      "http://localhost/?watchScenario=1&id=tank_trap_pathing_matrix&unit=scout_car&count=1&case=enemy_vehicle_reroute",
    );
    config = devWatchConfig();
    assert(config, "tank_trap_pathing_matrix case variant should be recognized");
    assert(
      config.room ===
        "__dev_scenario__:tank_trap_pathing_matrix:unit=scout_car:count=1:case=enemy_vehicle_reroute",
      "dev scenario should include matrix case variants in the server scenario room",
    );

    globalThis.window.location = new URL(
      "http://localhost/?watchScenario=1&id=entrenchment_inspection&unit=rifleman&count=1",
    );
    config = devWatchConfig();
    assert(config, "entrenchment_inspection dev scenario should be recognized");
    assert(
      config.room === "__dev_scenario__:entrenchment_inspection:unit=rifleman:count=1",
      "infantry dev scenarios should auto-join the server scenario room",
    );

    globalThis.window.location = new URL(
      "http://localhost/?watchScenario=1&id=bad/scenario&unit=scout_car&count=5",
    );
    config = devWatchConfig();
    assert(config === null, "dev scenario parser should reject unsafe scenario ids");

    globalThis.window.location = new URL(
      "http://localhost/?snapshotStream=supply-300-hellhole",
    );
    const snapshotStream = snapshotStreamLaunchConfig();
    assert(
      snapshotStream?.id === "supply-300-hellhole",
      "snapshot stream launch recognizes a safe static artifact id",
    );
    globalThis.window.location = new URL("http://localhost/?snapshotStream=bad/path");
    assert(snapshotStreamLaunchConfig() === null, "snapshot stream launch rejects unsafe paths");
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

    globalThis.window.location = new URL("http://localhost/?replayRoom=__match_replay__:abc123");
    config = replayLaunchConfig();
    assert(config?.room === "__match_replay__:abc123" && config.staging === true,
      "match-history replay room links join the replay staging lobby instead of auto-confirming playback");
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
    location: new URL("http://localhost/lab?room=sandbox&map=Chokes&seed=1234"),
    localStorage: { getItem: () => null },
  };
  try {
    const { labCatalogRouteConfig, labLaunchConfig } = await import("../../client/src/bootstrap.js");
    let config = labLaunchConfig();
    assert(config, "lab route launch config should be recognized");
    assert(config.publicRoom === "sandbox", "lab launch keeps public room label");
    assert(config.map === "Chokes", "lab launch keeps map label");
    assert(
      config.room === "__lab__:sandbox:map=Chokes:seed=1234",
      "lab launch should build the server lab room id",
    );

    globalThis.window.location = new URL("http://localhost/lab?room=bad/room&map=bad map");
    config = labLaunchConfig();
    assert(
      config.room === "__lab__:default:map=1v1",
      "lab launch falls back for unsafe room and map tokens without adding the default scenario to custom map URLs",
    );

    globalThis.window.location = new URL("http://localhost/lab");
    config = labLaunchConfig();
    assert(config === null, "plain lab route should show the scenario catalog before joining");
    assert(
      labCatalogRouteConfig().room === "default",
      "plain lab route should default the catalog room label",
    );

    globalThis.window.location = new URL("http://localhost/lab?room=sandbox");
    config = labLaunchConfig();
    assert(config === null, "room-only lab route should still show the scenario catalog");
    assert(
      labCatalogRouteConfig().room === "sandbox",
      "room-only lab route should prefill the catalog room label",
    );

    globalThis.window.location = new URL("http://localhost/lab?room=sandbox&visualProfile=trench-variants-1");
    config = labLaunchConfig();
    assert(config === null, "catalog lab route should not auto-join only because visualProfile is present");
    assert(
      labCatalogRouteConfig().visualProfileId === "trench-variants-1",
      "catalog lab route should retain a safe visual profile id for the selected scenario launch",
    );

    globalThis.window.location = new URL("http://localhost/lab?scenario=lategame");
    config = labLaunchConfig();
    assert(
      config.room === "__lab__:default:map=1v1:scenario=lategame",
      "explicit lategame lab scenario launch should request the bundled catalog id",
    );

    globalThis.window.location = new URL("http://localhost/lab?scenario=blank");
    config = labLaunchConfig();
    assert(
      config.room === "__lab__:default:map=1v1:scenario=blank",
      "explicit lab scenario override should be preserved",
    );

    globalThis.window.location = new URL(
      "http://localhost/lab?scenario=entrenchment_inspection&visualProfile=trench-variants-1",
    );
    config = labLaunchConfig();
    assert(
      config.visualProfileId === "trench-variants-1",
      "lab launch should keep a safe visual profile id for local registry resolution",
    );
    assert(
      config.room === "__lab__:default:map=1v1:scenario=entrenchment_inspection",
      "visual profile ids must not enter the server lab room id",
    );

    globalThis.window.location = new URL(
      "http://localhost/lab?scenario=entrenchment_inspection&visualProfile=../bad.svg",
    );
    config = labLaunchConfig();
    assert(config.visualProfileId === "", "unsafe visual profile ids are not preserved");
    assert(
      config.visualProfileError?.code === "invalid",
      "unsafe visual profile ids should fail closed before registry lookup",
    );
    assert(
      !config.room.includes("visualProfile") && !config.room.includes("bad.svg"),
      "unsafe visual profile input must not leak into the server room id",
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

async function testMatchLaunchConfig() {
  const {
    matchLaunchConfig,
  } = await import("../../client/src/launch_url.js");

  let config = matchLaunchConfig(new URL(
    "http://localhost/?rtsLaunch=match&rtsRoom=agent-ai-selfplay&rtsRole=spectator&rtsAi=1:ai_2_1&rtsAi=2:ai_turtle&rtsStart=1&rtsMap=Chokes",
  ));
  assert(config, "match launch URL should be recognized");
  assert(config.errors.length === 0, `match launch URL should be valid (${config.errors.join(" ")})`);
  assert(config.room === "agent-ai-selfplay", "match launch keeps the requested room");
  assert(config.name === "Spectator", "spectator match launch uses the spectator display name");
  assert(config.spectator === true, "match launch can join as spectator");
  assert(config.start === true, "match launch defaults to starting when requested");
  assert(config.map === "Chokes", "match launch keeps a safe map display name");
  assert(
    JSON.stringify(config.ai) === JSON.stringify([
      { teamId: 1, aiProfileId: "ai_2_1" },
      { teamId: 2, aiProfileId: "ai_turtle" },
    ]),
    "match launch preserves canonical AI profile choices",
  );

  config = matchLaunchConfig(
    new URL("http://localhost/?rtsLaunch=match"),
    { now: 0 },
  );
  assert(config.room === "ai-selfplay-0", "match launch generates a safe room when omitted");
  assert(
    JSON.stringify(config.ai) === JSON.stringify([
      { teamId: 1, aiProfileId: "ai_2_1" },
      { teamId: 2, aiProfileId: "ai_2_1" },
    ]),
    "match launch defaults to a two-team AI 2.1 self-play",
  );

  config = matchLaunchConfig(new URL(
    "http://localhost/?rtsLaunch=match&rtsRoom=agent-ai-selfplay&rtsAi=ai_turtle",
  ));
  assert(
    JSON.stringify(config.ai) === JSON.stringify([{ teamId: 1, aiProfileId: "ai_turtle" }]),
    "profile-only AI entries preserve AI Turtle",
  );

  config = matchLaunchConfig(new URL("http://localhost/?rtsLaunch=match&rtsRoom=bad%0Aroom"));
  assert(config.errors.some((error) => error.includes("unsupported characters")),
    "match launch rejects control characters in room names");

  config = matchLaunchConfig(new URL("http://localhost/?rtsLaunch=match&rtsRoom=__lab__:bad"));
  assert(config.errors.some((error) => error.includes("reserved")),
    "match launch rejects reserved server room prefixes");

  config = matchLaunchConfig(new URL(
    "http://localhost/?rtsLaunch=match&rtsRole=player&rtsAi=1&rtsAi=2&rtsAi=3&rtsAi=4",
  ));
  assert(config.errors.some((error) => error.includes("4-player match limit")),
    "player launches cannot overfill the four-seat match limit");

  assert(
    matchLaunchConfig(new URL("http://localhost/?rtsLaunch=replay")) === null,
    "unknown launch modes do not claim the page",
  );
}

async function testMatchLaunchActions() {
  const {
    matchLaunchConfig,
    nextMatchLaunchAction,
  } = await import("../../client/src/launch_url.js");
  const config = matchLaunchConfig(new URL(
    "http://localhost/?rtsLaunch=match&rtsRoom=agent-ai-selfplay&rtsRole=spectator&rtsAi=1:ai_2_1&rtsAi=2:ai_turtle&rtsStart=1",
  ));
  const spectator = { id: 7, isAi: false, isSpectator: true, ready: false };

  let action = nextMatchLaunchAction(config, {
    room: "agent-ai-selfplay",
    hostId: 7,
    map: "Chokes",
    maps: [{ name: "Chokes" }],
    players: [spectator],
    canStart: false,
  }, 7);
  assert(
    JSON.stringify(action) === JSON.stringify({ type: "addAi", teamId: 1, aiProfileId: "ai_2_1" }),
    "empty spectator-hosted launch lobby adds the first requested AI",
  );

  action = nextMatchLaunchAction(config, {
    room: "agent-ai-selfplay",
    hostId: 7,
    map: "Chokes",
    maps: [{ name: "Chokes" }],
    players: [
      spectator,
      { id: 20, isAi: true, isSpectator: false, teamId: 1, aiProfileId: "ai_2_1" },
    ],
    canStart: false,
  }, 7);
  assert(
    JSON.stringify(action) === JSON.stringify({ type: "addAi", teamId: 2, aiProfileId: "ai_turtle" }),
    "launch action adds the second requested AI after the first one appears",
  );

  action = nextMatchLaunchAction(config, {
    room: "agent-ai-selfplay",
    hostId: 7,
    map: "Chokes",
    maps: [{ name: "Chokes" }],
    players: [
      spectator,
      { id: 20, isAi: true, isSpectator: false, teamId: 1, aiProfileId: "ai_2_1" },
      { id: 21, isAi: true, isSpectator: false, teamId: 2, aiProfileId: "unsupported_profile" },
    ],
    canStart: true,
  }, 7);
  assert(
    JSON.stringify(action) === JSON.stringify({ type: "setAiProfile", id: 21, aiProfileId: "ai_turtle" }),
    "launch action corrects existing AI profile mismatches before start",
  );

  action = nextMatchLaunchAction(config, {
    room: "agent-ai-selfplay",
    hostId: 7,
    map: "Chokes",
    maps: [{ name: "Chokes" }],
    players: [
      spectator,
      { id: 20, isAi: true, isSpectator: false, teamId: 1, aiProfileId: "ai_2_1" },
      { id: 21, isAi: true, isSpectator: false, teamId: 2, aiProfileId: "ai_turtle" },
    ],
    canStart: true,
  }, 7);
  assert(action.type === "start", "launch action starts once the requested AI lobby is startable");

  const mapConfig = matchLaunchConfig(new URL(
    "http://localhost/?rtsLaunch=match&rtsRoom=agent-ai-selfplay&rtsMap=4%20Player%20Map",
  ));
  action = nextMatchLaunchAction(mapConfig, {
    room: "agent-ai-selfplay",
    hostId: 7,
    map: "Chokes",
    maps: [{ name: "Chokes" }, { name: "4 Player Map" }],
    players: [spectator],
    canStart: false,
  }, 7);
  assert(
    JSON.stringify(action) === JSON.stringify({ type: "selectMap", map: "4 Player Map" }),
    "launch action selects the requested available map before seating AIs",
  );

  action = nextMatchLaunchAction(config, {
    room: "agent-ai-selfplay",
    hostId: 7,
    map: "Chokes",
    maps: [{ name: "Chokes" }],
    players: [
      spectator,
      { id: 8, isAi: false, isSpectator: false, ready: false },
    ],
    canStart: false,
  }, 7);
  assert(action.type === "fail" && action.message.includes("active human seats"),
    "spectator self-play launch refuses to start over existing active human seats");
}

async function testVisualProfileRegistry() {
  const priorFetch = globalThis.fetch;
  globalThis.fetch = () => {
    throw new Error("visual profile registry must not fetch");
  };
  try {
    const {
      getVisualProfile,
      resolveVisualProfileLaunch,
      visualProfileIds,
    } = await import("../../client/src/visual_profiles.js");

    assert(
      visualProfileIds().includes("trench-variants-1"),
      "visual profile registry should include the first trench workflow profile",
    );
    const resolved = resolveVisualProfileLaunch({ visualProfileId: "trench-variants-1" });
    assert(
      resolved.profile?.id === "trench-variants-1",
      "registry should resolve checked-in visual profiles by id",
    );
    assert(
      Array.isArray(resolved.profile.staticSamples),
      "trench profile exposes staticSamples for the renderer-only Phase 2 read model",
    );
    assert(
      resolved.profile.staticSamples.length >= 5,
      "trench profile includes several checked-in static sample candidates",
    );
    assert(
      resolved.profile.initialCamera?.version === 1 &&
        Number.isFinite(resolved.profile.initialCamera?.focus?.x) &&
        Number.isFinite(resolved.profile.initialCamera?.focus?.y) &&
        Number.isFinite(resolved.profile.initialCamera?.framingScale),
      "trench profile provides a versioned semantic initial camera view",
    );
    assert(
      getVisualProfile("trench-variants-1") === resolved.profile,
      "direct registry lookup returns the checked-in profile object",
    );
    assert(
      getVisualProfile("constructor") === null,
      "registry lookup must not resolve inherited object prototype names",
    );

    let lookupCount = 0;
    const invalid = resolveVisualProfileLaunch(
      { visualProfileId: "", visualProfileError: { code: "invalid" } },
      () => {
        lookupCount += 1;
        return null;
      },
    );
    assert(invalid.error?.code === "invalid", "invalid visual profile ids remain local errors");
    assert(lookupCount === 0, "invalid visual profile ids must not reach registry lookup");

    const unknown = resolveVisualProfileLaunch({ visualProfileId: "future-profile" }, () => {
      lookupCount += 1;
      return null;
    });
    assert(unknown.profile === null, "unknown safe visual profile ids should not resolve");
    assert(unknown.error?.code === "unknown", "unknown safe visual profile ids should surface a local error");
    assert(lookupCount === 1, "unknown safe visual profile ids perform only registry lookup");

    const inheritedName = resolveVisualProfileLaunch({ visualProfileId: "toString" });
    assert(inheritedName.profile === null, "object prototype property names are not profiles");
    assert(
      inheritedName.error?.code === "unknown",
      "prototype property names should report the same local unknown-profile error",
    );
  } finally {
    if (priorFetch === undefined) delete globalThis.fetch;
    else globalThis.fetch = priorFetch;
  }
}

await testDevWatchScenarioConfig();
await testReplayArtifactLaunchConfig();
await testLabLaunchConfig();
await testMatchLaunchConfig();
await testMatchLaunchActions();
await testVisualProfileRegistry();

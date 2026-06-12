import {
  assertClientPrediction,
  assertLocalDisabledReason,
  assertLocalRenderOwnedOnly,
  assertLocalUnsupportedField,
  capture,
  issue,
  scenario,
  selectOwn,
  waitForAck,
  waitForClientPredictionReady,
} from "../dsl.mjs";

export default scenario("combat_command_authoritative_only", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
    quickstart: true,
  },
  network: {
    mode: "profile",
    name: "phase-6-combat-authoritative-only",
    tickMs: 10,
    snapshotLatencyTicks: 8,
    seed: 6006,
  },
  steps: [
    waitForClientPredictionReady(),
    capture("before-attack"),
    assertLocalUnsupportedField("combat"),
    selectOwn("worker", 0),
    issue("attack", { target: 999999999 }),
    capture("after-local-attack-command"),
    assertLocalDisabledReason("commandUnsupported"),
    assertLocalRenderOwnedOnly(),
    waitForAck(1, { timeoutMs: 10000 }),
    assertClientPrediction({ minAcknowledgedCount: 1, pendingCommandCount: 0 }),
  ],
});

import {
  assertClientPrediction,
  capture,
  injectClientSnapshot,
  issueBurst,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
} from "../dsl.mjs";

export default scenario("duplicate_and_skipped_snapshots_are_diagnostic", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    setClientSnapshotDelivery(false),
    issueBurst([
      { command: "move", args: { dx: 32, dy: 0 } },
      { command: "move", args: { dx: 64, dy: 0, queued: true } },
    ]),
    injectClientSnapshot("duplicate", { ackSeq: 0 }),
    injectClientSnapshot("duplicate", { ackSeq: 0 }),
    injectClientSnapshot("skipped", { ackSeq: 1, tickDelta: 4 }),
    capture("diagnostic-snapshots"),
    assertClientPrediction({
      pendingClientSeqs: [2],
      latestAckSeq: 1,
      minDuplicateSnapshotCount: 1,
      minSkippedSnapshotCount: 1,
    }),
  ],
});

import { assert, assertThrows } from "./assertions.mjs";
import { messagePackSnapshotFrame } from "./snapshot_frame_helpers.mjs";
import { COMPACT_SNAPSHOT_VERSION, S } from "../../client/src/protocol.js";
import {
  SnapshotStreamNet,
  parseSnapshotStream,
  snapshotStreamAssetUrl,
} from "../../client/src/snapshot_stream_net.js";

function streamBytes({ id = "fixture", frames, loop = true }) {
  const header = new TextEncoder().encode(JSON.stringify({
    schemaVersion: 1,
    id,
    tickRateHz: 30,
    loop,
    loopGapMs: 0,
    frameCount: frames.length,
    start: {
      playerId: 1,
      spectator: true,
      tick: 0,
      map: { width: 4, height: 4, tileSize: 32, terrain: [], resources: [] },
      players: [],
      snapshotStream: { id, serverSimulation: false },
    },
  }));
  const total = 8 + 4 + header.length + frames.reduce((sum, frame) => sum + 4 + frame.byteLength, 0);
  const out = new Uint8Array(total);
  out.set(new TextEncoder().encode("RTSSTRM1"));
  const view = new DataView(out.buffer);
  view.setUint32(8, header.length, true);
  out.set(header, 12);
  let offset = 12 + header.length;
  for (const frame of frames) {
    view.setUint32(offset, frame.byteLength, true);
    offset += 4;
    out.set(frame, offset);
    offset += frame.byteLength;
  }
  return out;
}

function snapshotFrame(tick) {
  return messagePackSnapshotFrame({
    t: "snapshot",
    v: COMPACT_SNAPSHOT_VERSION,
    s: [tick, 0, 0, 0, 0],
    e: [],
    n: [0, 0, 0, 0, 0, 0, 0, null],
  });
}

{
  assert(
    snapshotStreamAssetUrl("supply-300-hellhole") ===
      "/assets/snapshot-streams/supply-300-hellhole.rtsstream",
    "snapshot stream ids map to static same-origin artifacts",
  );
  assertThrows(() => snapshotStreamAssetUrl("../secret"), "snapshot stream asset ids reject paths");

  const fixture = streamBytes({ frames: [snapshotFrame(1), snapshotFrame(2)] });
  const parsed = parseSnapshotStream(fixture);
  assert(parsed.header.frameCount === 2, "snapshot stream parser reads its bounded header");
  assert(parsed.frames.length === 2, "snapshot stream parser reads each framed payload");
  assertThrows(
    () => parseSnapshotStream(fixture.subarray(0, fixture.length - 1)),
    "snapshot stream parser rejects truncated frames",
  );
}

{
  const fixture = streamBytes({ id: "fixture", frames: [snapshotFrame(11), snapshotFrame(12)] });
  const scheduled = [];
  let now = 100;
  const net = new SnapshotStreamNet({
    id: "fixture",
    fetchFn: async () => ({ ok: true, arrayBuffer: async () => fixture.buffer }),
    now: () => now,
    setTimeoutFn: (callback, delay) => {
      scheduled.push({ callback, delay });
      return scheduled.length;
    },
    clearTimeoutFn: () => {},
  });
  const events = [];
  net.on("open", () => events.push("open"));
  net.on(S.START, () => events.push("start"));
  net.on(S.SNAPSHOT, (snapshot) => events.push(`snapshot:${snapshot.tick}`));

  await net.connect();
  assert(net.offline && net.ws === null, "snapshot stream transport never creates a WebSocket");
  assert(events.join(",") === "open,start", "offline transport starts the normal match shell");
  assert(scheduled.length === 1 && scheduled[0].delay === 0, "first snapshot is scheduled after start");

  scheduled.shift().callback();
  now += 34;
  scheduled.shift().callback();
  scheduled.shift().callback();
  assert(
    events.join(",") === "open,start,snapshot:11,snapshot:12,start",
    "offline transport decodes normal snapshot frames and restarts cleanly when looping",
  );
  assert(net.publicState.serverSimulation === false && net.publicState.websocket === false,
    "offline transport exposes its isolation state for benchmark verification");
  net.close();
}

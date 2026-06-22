import { encodeMessagePack } from "../../scripts/snapshot-codec-bakeoff.mjs";
import { SNAPSHOT_CODEC_VERSION } from "../../client/src/protocol.js";

export function messagePackSnapshotFrame(raw) {
  const payload = encodeMessagePack(raw);
  const frame = new Uint8Array(5 + payload.byteLength);
  frame.set([0x52, 0x54, 0x53, 0x4d, SNAPSHOT_CODEC_VERSION], 0);
  frame.set(payload, 5);
  return frame;
}

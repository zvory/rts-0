import { S } from "./protocol.js";

const MAX_COMMAND_RECEIPTS = 64;
const LATEST_TYPES = Object.freeze([
  S.SNAPSHOT,
  S.ROOM_TIME_STATE,
  S.LIVE_PAUSE_STATE,
  S.OBSERVER_ANALYSIS,
]);

/** Buffer messages that race asynchronous renderer startup according to each lane's semantics. */
export function createMatchStartupInbox(net, diagnostics = null) {
  const latest = new Map();
  const receipts = [];
  let reportedReceiptDrop = false;
  const handlers = new Map();

  for (const type of [...LATEST_TYPES, S.COMMAND_RECEIPT]) {
    const handler = (message) => {
      if (type !== S.COMMAND_RECEIPT) {
        latest.set(type, message);
        return;
      }
      if (receipts.length >= MAX_COMMAND_RECEIPTS) {
        receipts.shift();
        if (!reportedReceiptDrop) {
          diagnostics?.mark?.("match.create.pendingCommandReceiptsDropped");
          reportedReceiptDrop = true;
        }
      }
      receipts.push(message);
    };
    handlers.set(type, handler);
    net.on(type, handler);
  }

  return {
    stop() {
      for (const [type, handler] of handlers) net.off(type, handler);
    },
    flush(match) {
      applyLatest(latest, S.ROOM_TIME_STATE, match.onRoomTimeState);
      applyLatest(latest, S.LIVE_PAUSE_STATE, match.onLivePauseState);
      for (const message of receipts) match.onCommandReceipt(message);
      applyLatest(latest, S.SNAPSHOT, match.onSnapshot);
      applyLatest(latest, S.OBSERVER_ANALYSIS, match.onObserverAnalysis);
    },
  };
}

function applyLatest(latest, type, handler) {
  if (latest.has(type)) handler(latest.get(type));
}

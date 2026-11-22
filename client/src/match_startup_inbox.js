import { S } from "./protocol.js";

const MAX_COMMAND_RECEIPTS = 64;
const LATEST_TYPES = Object.freeze([
  S.ROOM_TIME_STATE,
  S.LIVE_PAUSE_STATE,
  S.OBSERVER_ANALYSIS,
]);

/** Buffer messages that race asynchronous renderer startup according to each lane's semantics. */
export function createMatchStartupInbox(net, diagnostics = null) {
  const latest = new Map();
  const receipts = [];
  let snapshot = null;
  let resourceReset = false;
  const resourceDeltas = new Map();
  const replayResourceHistory = [];
  let reportedReceiptDrop = false;
  let pauseStateReceivedAt = 0;
  const handlers = new Map();

  for (const type of [...LATEST_TYPES, S.SNAPSHOT, S.COMMAND_RECEIPT, S.REPLAY_RESOURCE_HISTORY]) {
    const handler = (message) => {
      if (type === S.SNAPSHOT) {
        // Entity/fog lanes are complete views; resources are a replacement plus patches.
        // Coalesce by id rather than retaining traffic proportional to loading time.
        if (message.resourceReset === true) {
          resourceDeltas.clear();
          resourceReset = true;
        }
        for (const row of message.resourceDeltas || []) resourceDeltas.set(row.id, row);
        snapshot = message;
        return;
      }
      if (type === S.REPLAY_RESOURCE_HISTORY) {
        if (message.replace) replayResourceHistory.length = 0;
        replayResourceHistory.push(message);
        return;
      }
      if (type !== S.COMMAND_RECEIPT) {
        latest.set(type, message);
        if (type === S.LIVE_PAUSE_STATE) pauseStateReceivedAt = performance.now();
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
      const pauseState = latest.get(S.LIVE_PAUSE_STATE);
      if (pauseState?.resumeCountdown) {
        // Renderer startup must not restart the server-owned setup/countdown clock.
        latest.set(S.LIVE_PAUSE_STATE, {
          ...pauseState,
          resumeCountdown: {
            ...pauseState.resumeCountdown,
            remainingMs: Math.max(1, pauseState.resumeCountdown.remainingMs -
              Math.max(0, performance.now() - pauseStateReceivedAt)),
          },
        });
      }
      applyLatest(latest, S.LIVE_PAUSE_STATE, match.onLivePauseState);
      for (const message of receipts) match.onCommandReceipt(message);
      if (snapshot) {
        match.onSnapshot({
          ...snapshot,
          resourceReset,
          resourceDeltas: [...resourceDeltas.values()],
        });
      }
      applyLatest(latest, S.OBSERVER_ANALYSIS, match.onObserverAnalysis);
      for (const message of replayResourceHistory) match.onReplayResourceHistory(message);
      // A handoff is consumable once: replaying a reset later could erase newer knowledge.
      latest.clear();
      receipts.length = 0;
      replayResourceHistory.length = 0;
      snapshot = null;
      resourceReset = false;
      resourceDeltas.clear();
    },
  };
}

function applyLatest(latest, type, handler) {
  if (latest.has(type)) handler(latest.get(type));
}

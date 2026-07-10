// tests/client_contracts/match_history_contracts.mjs
// Match-history lobby table contracts imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { withFakeHudDocument } from "./fakes.mjs";
import { MatchHistory, matchHistoryWinnerLabel } from "../../client/src/match_history.js";

const FALLBACK_LABEL = "\u2014";

assert(
  matchHistoryWinnerLabel({ winnerName: "Alpha", outcome: "win" }) === "Alpha",
  "match history winner label displays a normal winner",
);
assert(
  matchHistoryWinnerLabel({ winnerName: null, outcome: "draw" }) === "Draw",
  "match history winner label displays draws",
);
assert(
  matchHistoryWinnerLabel({ winnerName: null, outcome: "aborted" }) === "Aborted",
  "match history winner label displays aborted matches",
);
assert(
  matchHistoryWinnerLabel({ winnerName: "Alpha", outcome: "aborted" }) === "Aborted",
  "match history winner label does not display a winner for aborted rows",
);
assert(
  matchHistoryWinnerLabel({ winnerName: null, outcome: "unexpected" }) === FALLBACK_LABEL,
  "match history winner label falls back for unknown outcomes",
);
assert(
  matchHistoryWinnerLabel({}) === FALLBACK_LABEL,
  "match history winner label falls back for empty rows",
);

withFakeHudDocument(() => {
  let requestUrl = null;
  const history = new MatchHistory(document.createElement("div"), {
    fetchImpl(url) {
      requestUrl = url;
      return new Promise(() => {});
    },
  });

  assert(requestUrl === "/api/matches?limit=300", "match history requests the 300-row display cap");
  history.destroy();
});

{
  let joinedRoom = "";
  let renderedRows = 0;
  const history = Object.assign(Object.create(MatchHistory.prototype), {
    fetchImpl: async (url, init) => {
      assert(
        url === "/api/matches/42/replay" && init?.method === "POST",
        "watch replay posts to the replay launch endpoint",
      );
      return {
        ok: true,
        json: async () => ({ room: "__match_replay__:abc123" }),
      };
    },
    onReplayRoom(room) {
      joinedRoom = room;
      return true;
    },
    _launchingId: null,
    _launchErrors: new Map(),
    _renderRows() {
      renderedRows += 1;
    },
  });

  await history._launchReplay(42);
  assert(joinedRoom === "__match_replay__:abc123",
    "watch replay hands the created replay room back to the lobby flow");
  assert(history._launchingId === null && renderedRows >= 2,
    "watch replay clears launching state after lobby handoff");
}

withFakeHudDocument(() => {
  let launchedId = null;
  const row = {
    id: 7,
    startedAt: new Date().toISOString(),
    mapName: "Default",
    participants: ["Alice", "Bravo"],
    winnerName: null,
    outcome: "aborted",
    durationMs: 12_000,
    replayAvailable: true,
    replayUnavailableReason: null,
    scoreScreen: [
      { name: "Alice", unitScore: 10, structureScore: 0 },
      { name: "Bravo", unitScore: 9, structureScore: 0 },
    ],
  };
  const history = Object.assign(Object.create(MatchHistory.prototype), {
    _tableHost: document.createElement("div"),
    _rows: [row],
    _expandedId: row.id,
    _launchingId: null,
    _launchErrors: new Map(),
    _toggleRow() {},
    _launchReplay(id) {
      launchedId = id;
    },
  });

  history._renderRows();

  const renderedText = collectText(history._tableHost).join(" ");
  assert(renderedText.includes("Aborted"), "expanded aborted match row renders Aborted as the result");
  assert(renderedText.includes("Watch replay"), "expanded aborted match row exposes replay launch");
  assert(renderedText.includes("Alice") && renderedText.includes("Bravo"),
    "expanded aborted match row keeps score-screen player detail");

  const buttons = history._tableHost.querySelectorAll("button");
  assert(buttons.length === 1, "expanded aborted match row renders one replay button");
  buttons[0].listeners.click({ stopPropagation() {} });
  assert(launchedId === row.id, "aborted replay button launches the aborted match replay");
});

function collectText(node, out = []) {
  if (typeof node?.textContent === "string" && node.textContent.length > 0) {
    out.push(node.textContent);
  }
  for (const child of node?.children || []) collectText(child, out);
  return out;
}

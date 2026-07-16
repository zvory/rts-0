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
  let refreshes = 0;
  const host = document.createElement("div");
  const history = Object.assign(Object.create(MatchHistory.prototype), {
    host,
    refresh() {
      refreshes += 1;
    },
  });
  history._render();
  const button = host.querySelectorAll("button")[0];
  assert(button?.textContent === "Refresh", "match history exposes a manual refresh button");
  button.listeners.click();
  assert(refreshes === 1, "match history refresh button performs one request");
});

{
  const requests = [];
  const history = Object.assign(Object.create(MatchHistory.prototype), {
    limit: 20,
    _ac: null,
    _loading: false,
    _rows: [],
    fetchImpl(_url, { signal }) {
      return new Promise((resolve, reject) => requests.push({ resolve, reject, signal }));
    },
    _setLoading() {},
    _renderRows() {},
    _setError() {},
    _reflectRefreshButton() {},
  });
  const staleRefresh = history.refresh();
  const staleRequest = requests[0];
  const currentRefresh = history.refresh();
  const currentRequest = requests[1];
  assert(staleRequest.signal.aborted, "a newer match-history refresh aborts the older request");

  currentRequest.resolve({
    ok: true,
    async json() { return [{ id: 2 }]; },
  });
  await currentRefresh;
  staleRequest.resolve({
    ok: true,
    async json() { return [{ id: 1 }]; },
  });
  await staleRefresh;

  assert(history._rows[0]?.id === 2,
    "a stale match-history response cannot overwrite the latest refresh");
  assert(!history._loading, "the latest match-history refresh owns the loading state");
}

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
    mapName: "Chokes",
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

withFakeHudDocument(() => {
  const rows = [
    {
      id: 42,
      replayNumber: 105,
      startedAt: new Date().toISOString(),
      mapName: "Chokes",
      participants: ["Alice", "Bravo"],
      durationMs: 12_000,
    },
    {
      id: 99,
      replayNumber: 104,
      startedAt: new Date().toISOString(),
      mapName: "River Crossing",
      participants: ["Casey", "Dana"],
      durationMs: 20_000,
    },
  ];
  const history = Object.assign(Object.create(MatchHistory.prototype), {
    _tableHost: document.createElement("div"),
    _rows: rows,
    _expandedId: null,
    _launchingId: null,
    _launchErrors: new Map(),
    _toggleRow() {},
  });

  history._renderRows();

  const table = history._tableHost.children[0];
  const [thead, tbody] = table.children;
  const replayRows = tbody.children.filter((row) => row.className === "match-history-row");
  assert(thead.innerHTML.includes("Replay #"), "match history labels its replay-number column");
  assert(
    replayRows.map((row) => row.children[0].textContent).join(",") === "105,104",
    "match history renders the server's global visible-history numbers instead of page-local indexes",
  );
});

function collectText(node, out = []) {
  if (typeof node?.textContent === "string" && node.textContent.length > 0) {
    out.push(node.textContent);
  }
  for (const child of node?.children || []) collectText(child, out);
  return out;
}

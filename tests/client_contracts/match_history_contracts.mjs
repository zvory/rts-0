// tests/client_contracts/match_history_contracts.mjs
// Match-history lobby table contracts imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
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

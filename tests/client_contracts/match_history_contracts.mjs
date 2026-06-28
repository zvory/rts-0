// tests/client_contracts/match_history_contracts.mjs
// Match-history lobby table contracts imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { matchHistoryWinnerLabel } from "../../client/src/match_history.js";

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

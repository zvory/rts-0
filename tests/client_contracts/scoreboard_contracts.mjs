import { assert } from "./assertions.mjs";
import { formatTeamLabel, scoreRowIsWinner } from "../../client/src/scoreboard.js";

export function runScoreboardContracts() {
  assert(formatTeamLabel(2) === "Team 2", "scoreboard formats numeric team labels");
  assert(formatTeamLabel(null) === "-", "scoreboard formats missing team labels");
  assert(scoreRowIsWinner({ id: 7, teamId: 2 }, 7, null), "scoreboard keeps winnerId fallback");
  assert(scoreRowIsWinner({ id: 8, teamId: 2 }, 7, 2), "scoreboard highlights all winning-team rows");
  assert(!scoreRowIsWinner({ id: 7, teamId: 1 }, 7, 2),
    "winnerTeamId takes precedence over singleton winnerId highlighting");
}

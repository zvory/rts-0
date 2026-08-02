import { assert } from "./assertions.mjs";
import {
  formatTeamLabel,
  replayResultHeadline,
  scoreRowIsWinner,
} from "../../client/src/scoreboard.js";

export function runScoreboardContracts() {
  assert(formatTeamLabel(2) === "Team 2", "scoreboard formats numeric team labels");
  assert(formatTeamLabel(null) === "-", "scoreboard formats missing team labels");
  assert(scoreRowIsWinner({ id: 7, teamId: 2 }, 7, null), "scoreboard keeps winnerId fallback");
  assert(scoreRowIsWinner({ id: 8, teamId: 2 }, 7, 2), "scoreboard highlights all winning-team rows");
  assert(!scoreRowIsWinner({ id: 7, teamId: 1 }, 7, 2),
    "winnerTeamId takes precedence over singleton winnerId highlighting");
  assert(
    replayResultHeadline([
      { id: 7, teamId: 1, name: "Alex" },
      { id: 8, teamId: 2, name: "DV" },
    ], 7, 1) === "Alex has won",
    "replay result names the winning player instead of using the spectator verdict",
  );
  assert(
    replayResultHeadline([
      { id: 7, teamId: 1, name: "Alex" },
      { id: 8, teamId: 1, name: "DV" },
      { id: 9, teamId: 2, name: "Rival" },
    ], 7, 1) === "Alex and DV have won",
    "team replay results name every winning player",
  );
  assert(replayResultHeadline([], null, null) === "Draw",
    "replay result keeps Draw when no winner exists");
}

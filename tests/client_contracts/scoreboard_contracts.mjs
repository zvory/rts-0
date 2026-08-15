import { assert } from "./assertions.mjs";
import {
  formatTeamLabel,
  matchConclusionDetail,
  renderMatchConclusionDetail,
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
  assert(
    matchConclusionDetail([
      { id: 7, name: "Alex" },
      { id: 8, name: "DV" },
    ], { defeatedPlayerIds: [8], reason: "gaveUp" }) === "DV has given up.",
    "scoreboard explains a surrender by name",
  );
  assert(
    matchConclusionDetail([
      { id: 7, name: "Alex" },
      { id: 8, name: "DV" },
    ], { defeatedPlayerIds: [8], reason: "lostAllBuildings" }) ===
      "DV lost all their buildings.",
    "scoreboard explains a base-destruction defeat by name",
  );
  const detailElement = { textContent: "stale", hidden: false };
  renderMatchConclusionDetail(detailElement, [], null);
  assert(detailElement.textContent === "" && detailElement.hidden,
    "scoreboard clears stale conclusion details between matches");
}

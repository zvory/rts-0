export function formatTeamLabel(teamId) {
  const id = Number(teamId);
  return Number.isInteger(id) && id > 0 ? `Team ${id}` : "-";
}

export function scoreRowIsWinner(score, winnerId = null, winnerTeamId = null) {
  const teamId = Number(score?.teamId);
  if (winnerTeamId != null && Number.isFinite(teamId)) {
    return teamId === Number(winnerTeamId);
  }
  const id = Number(score?.id);
  return winnerId != null && Number.isFinite(id) && id === Number(winnerId);
}

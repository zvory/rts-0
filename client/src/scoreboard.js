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

export function replayResultHeadline(scores, winnerId = null, winnerTeamId = null) {
  const winnerNames = (Array.isArray(scores) ? scores : [])
    .filter((score) => scoreRowIsWinner(score, winnerId, winnerTeamId))
    .map((score) => {
      const name = typeof score?.name === "string" ? score.name.trim() : "";
      const id = Number(score?.id);
      return name || (Number.isFinite(id) ? `Player ${id}` : "Player");
    });
  if (!winnerNames.length) return "Draw";
  if (winnerNames.length === 1) return `${winnerNames[0]} has won`;
  const lastName = winnerNames.pop();
  const joinedNames = winnerNames.length === 1
    ? `${winnerNames[0]} and ${lastName}`
    : `${winnerNames.join(", ")}, and ${lastName}`;
  return `${joinedNames} have won`;
}

export function matchConclusionDetail(scores, conclusion) {
  const ids = Array.isArray(conclusion?.defeatedPlayerIds)
    ? new Set(conclusion.defeatedPlayerIds.map(Number).filter(Number.isFinite))
    : new Set();
  const names = (Array.isArray(scores) ? scores : [])
    .filter((score) => ids.has(Number(score?.id)))
    .map((score) => {
      const name = typeof score?.name === "string" ? score.name.trim() : "";
      return name || `Player ${Number(score.id)}`;
    });
  if (!names.length) return "";
  const lastName = names.pop();
  const subject = names.length === 0
    ? lastName
    : names.length === 1
      ? `${names[0]} and ${lastName}`
      : `${names.join(", ")}, and ${lastName}`;
  if (conclusion.reason === "gaveUp") {
    return `${subject} ${names.length ? "have" : "has"} given up.`;
  }
  if (conclusion.reason === "lostAllBuildings") {
    return `${subject} lost all their buildings.`;
  }
  return "";
}

export function renderMatchConclusionDetail(element, scores, conclusion) {
  if (!element) return;
  const detail = matchConclusionDetail(scores, conclusion);
  element.textContent = detail;
  element.hidden = !detail;
}

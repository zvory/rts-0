import { COMMAND_BUDGET_OVERFLOW_NOTICE, commandWithinBudget } from "./command_budget.js";

export function createMatchCommandIssuer(match) {
  return {
    issueCommand: (command, options = {}) => {
      if (match.livePauseState.paused) return { sent: false, blocked: "livePause" };
      if (match.labControlPolicy?.kind === "lab") {
        return match.labControlPolicy.issueCommand(command, {
          state: match.state,
          toast: match.toast,
        });
      }
      const budget = commandWithinBudget(match.state, command);
      if (!budget.ok) {
        match.toast?.(COMMAND_BUDGET_OVERFLOW_NOTICE);
        return { clientSeq: null, sent: false, predicted: false, blocked: "commandBudget", budget };
      }
      const predictionOptions = match.predictionVisualsPaused()
        ? { ...options, predictMovement: false }
        : options;
      const issued = match.prediction.issueCommand(command, predictionOptions);
      if (issued?.sent) {
        match.apmTracker.recordAction(match.state?.tick);
        const issuedAt = match.frameProfiler?.now?.() ?? performance.now();
        match.health.noteCommandIssued(issuedAt);
        match.frameProfiler?.recordDiagnosticCounter?.("commands.issued");
      }
      match.applyPredictionDisplayOverlay(match.prediction.predictionDisplayOverlay());
      return issued;
    },
  };
}

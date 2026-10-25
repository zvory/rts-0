import { execFileSync } from "node:child_process";

export function enforceClassifierBudgets(commits, prompts, options) {
  if (commits.length > options.maxCommits) {
    throw new Error(
      `classify budget exceeded: ${commits.length} considered commits exceeds --max-commits ${options.maxCommits}`,
    );
  }
  const uncachedPrompts = prompts.filter((entry) => !entry.cacheHit);
  const tooLarge = uncachedPrompts.find((entry) => entry.promptTokens > options.maxPromptTokens);
  if (tooLarge) {
    throw new Error(
      `classify budget exceeded: ${tooLarge.commit.shortSha} prompt estimate ${tooLarge.promptTokens} tokens exceeds --max-prompt-tokens ${options.maxPromptTokens}`,
    );
  }
  const totalTokens = uncachedPrompts.reduce((sum, entry) => sum + entry.promptTokens, 0);
  if (totalTokens > options.maxTotalPromptTokens) {
    throw new Error(
      `classify budget exceeded: prompt estimate ${totalTokens} tokens exceeds --max-total-prompt-tokens ${options.maxTotalPromptTokens}`,
    );
  }
}


// Batch only at first-parent boundaries. Every ancestor introduced by a merge
// stays in the same batch, so advancing the checkpoint cannot hide side commits.
export function selectClassifierBatch(report, entries, options) {
  const fits = (items) => items.length <= options.maxCommits &&
    items.filter((entry) => !entry.cacheHit).every((entry) => entry.promptTokens <= options.maxPromptTokens) &&
    items.filter((entry) => !entry.cacheHit).reduce((sum, entry) => sum + entry.promptTokens, 0) <= options.maxTotalPromptTokens;
  if (fits(entries)) return { report, entries };
  const git = (args) => execFileSync("git", args, { cwd: options.repoRoot, encoding: "utf8" }).trim();
  const candidates = git(["rev-list", "--first-parent", "--reverse", `${report.base.sha}..${report.head.sha}`]).split("\n").filter(Boolean);
  const at = (index) => {
    const checkpointSha = candidates[index];
    const included = new Set(git(["rev-list", `${report.base.sha}..${checkpointSha}`]).split("\n"));
    return { checkpointSha, included, entries: entries.filter((entry) => included.has(entry.commit.sha)) };
  };
  let low = 0;
  let high = candidates.length - 1;
  let selected = null;
  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    const candidate = at(mid);
    if (fits(candidate.entries)) {
      selected = candidate;
      low = mid + 1;
    } else high = mid - 1;
  }
  if (!selected) {
    throw new Error("classifier batch cannot fit the first ancestry-complete commit group within configured budgets; operator review required");
  }
  const commits = report.commits.filter((commit) => selected.included.has(commit.sha));
  return {
    entries: selected.entries,
    report: {
      ...report,
      batch: { checkpointSha: selected.checkpointSha, deferredCommits: report.commits.length - commits.length },
      commits,
      summary: {
        ...report.summary,
        totalCommits: commits.length,
        consideredCommits: selected.entries.length,
        skippedMergeCommits: commits.filter((commit) => commit.skipReason === "merge_commit").length,
        skippedEmptyCommits: commits.filter((commit) => commit.skipReason === "empty_commit").length,
        skippedDocsOnlyCommits: commits.filter((commit) => commit.skipReason === "docs_only_churn").length,
        noCommits: commits.length === 0,
      },
    },
  };
}

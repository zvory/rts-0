import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

function markdownList(items, emptyText) {
  if (items.length === 0) {
    return `- ${emptyText}`;
  }
  return items.map((item) => `- ${item}`).join("\n");
}

function formatUsage(usage) {
  if (!usage) {
    return "unavailable";
  }
  const parts = [];
  if (usage.inputTokens !== null) {
    parts.push(`input=${usage.inputTokens}`);
  }
  if (usage.cachedInputTokens !== null) {
    parts.push(`cached_input=${usage.cachedInputTokens}`);
  }
  if (usage.outputTokens !== null) {
    parts.push(`output=${usage.outputTokens}`);
  }
  if (usage.reasoningTokens !== null) {
    parts.push(`reasoning=${usage.reasoningTokens}`);
  }
  if (usage.totalTokens !== null) {
    parts.push(`total=${usage.totalTokens}`);
  }
  return parts.length > 0 ? parts.join(", ") : "unavailable";
}

export function renderMarkdown(report) {
  if (report.mode === "full") {
    return renderFullMarkdown(report);
  }
  if (report.mode === "generate-docs") {
    return renderGenerateDocsMarkdown(report);
  }
  if (report.mode === "classify") {
    return renderClassifierMarkdown(report);
  }

  const lines = [
    "# Documentation Drift Sweep Dry Run",
    "",
    `Base: ${report.base.ref} (${report.base.sha.slice(0, 12)})`,
    `Head: ${report.head.ref} (${report.head.sha.slice(0, 12)})`,
    `Trace map: ${report.traceMap.path} (version ${report.traceMap.version ?? "unknown"}, ${report.traceMap.routeCount} routes)`,
    "",
    "## Summary",
    "",
    `- Total commits: ${report.summary.totalCommits}`,
    `- Considered commits: ${report.summary.consideredCommits}`,
    `- Skipped merge commits: ${report.summary.skippedMergeCommits}`,
    `- Skipped empty commits: ${report.summary.skippedEmptyCommits}`,
    `- Skipped docs-only churn commits: ${report.summary.skippedDocsOnlyCommits}`,
    "",
  ];

  if (report.summary.noCommits) {
    lines.push("No commits to sweep between the checkpoint and head.", "");
    return `${lines.join("\n")}\n`;
  }

  const considered = report.commits.filter((commit) => commit.status === "considered");
  lines.push("## Considered Commits", "");
  if (considered.length === 0) {
    lines.push("No non-merge, non-docs-only commits need doc drift classification.", "");
  }
  for (const commit of considered) {
    lines.push(
      `### ${commit.shortSha} - ${commit.subject}`,
      "",
      `- Author date: ${commit.authorDate}`,
      `- Diff stat: ${commit.diffStat}`,
      `- Design docs touched: ${commit.docsTouched.anyDesign ? commit.docsTouched.design.join(", ") : "none"}`,
      `- Context docs touched: ${commit.docsTouched.anyContext ? commit.docsTouched.context.join(", ") : "none"}`,
      `- Trace-map candidate docs: ${commit.traceDocs.length > 0 ? commit.traceDocs.join(", ") : "none"}`,
      "",
      "Changed paths:",
      markdownList(commit.changedPaths, "none"),
    );
    if (commit.body) {
      lines.push("", "Commit body:", "", commit.body.split("\n").map((line) => `> ${line}`).join("\n"));
    }
    lines.push("");
  }

  const skipped = report.commits.filter((commit) => commit.status === "skipped");
  lines.push("## Skipped Commits", "");
  if (skipped.length === 0) {
    lines.push("No commits were skipped.", "");
  }
  for (const commit of skipped) {
    lines.push(`- ${commit.shortSha} ${commit.subject} (${commit.skipReason})`);
  }
  lines.push("");

  return `${lines.join("\n")}\n`;
}

function renderClassifierMarkdown(report) {
  const lines = [
    "# Documentation Drift Classifier Report",
    "",
    `Base: ${report.base.ref} (${report.base.sha.slice(0, 12)})`,
    `Head: ${report.head.ref} (${report.head.sha.slice(0, 12)})`,
    `Trace map: ${report.traceMap.path} (version ${report.traceMap.version ?? "unknown"}, ${report.traceMap.routeCount} routes)`,
    `Classifier prompt: ${report.classifier.promptVersion}`,
    `Classifier cache: ${report.classifier.cacheDir}`,
    "",
    "## Summary",
    "",
    `- Total commits: ${report.summary.totalCommits}`,
    `- Considered commits: ${report.summary.consideredCommits}`,
    `- Decisions: ${report.classifier.summary.totalDecisions}`,
    `- Move on: ${report.classifier.summary.moveOn}`,
    `- Update docs: ${report.classifier.summary.updateDocs}`,
    `- Cache hits: ${report.classifier.summary.cacheHits}`,
    `- Estimated prompt tokens: ${report.classifier.budget.estimatedPromptTokens}`,
    "",
  ];

  if (report.classifier.summary.totalDecisions === 0) {
    lines.push("No non-merge, non-docs-only commits need doc drift classification.", "");
    return `${lines.join("\n")}\n`;
  }

  lines.push("## Decisions", "");
  for (const decision of report.classifier.decisions) {
    lines.push(
      `### ${decision.shortSha} - ${decision.subject}`,
      "",
      `- Decision: ${decision.decision}`,
      `- Likely docs: ${decision.likelyDocs.length > 0 ? decision.likelyDocs.join(", ") : "none"}`,
      `- Evidence: ${decision.evidenceNote}`,
      `- Cache: ${decision.cache.hit ? "hit" : "miss"}${decision.cache.reason ? ` (${decision.cache.reason})` : ""} (${decision.cache.path})`,
      `- Invocation mode: ${decision.codex.mode}`,
      `- Codex usage: ${formatUsage(decision.codex.usage)}`,
      "",
    );
  }

  const skipped = report.commits.filter((commit) => commit.status === "skipped");
  lines.push("## Skipped Commits", "");
  if (skipped.length === 0) {
    lines.push("No commits were skipped.", "");
  }
  for (const commit of skipped) {
    lines.push(`- ${commit.shortSha} ${commit.subject} (${commit.skipReason})`);
  }
  lines.push("");
  return `${lines.join("\n")}\n`;
}

function renderGenerateDocsMarkdown(report) {
  const lines = [
    "# Documentation Drift Generated Docs Report",
    "",
    `Base: ${report.base.ref} (${report.base.sha.slice(0, 12)})`,
    `Head: ${report.head.ref} (${report.head.sha.slice(0, 12)})`,
    `Trace map: ${report.traceMap.path} (version ${report.traceMap.version ?? "unknown"}, ${report.traceMap.routeCount} routes)`,
    `Classifier prompt: ${report.classifier.promptVersion}`,
    `Doc patch prompt: ${report.docPatch.promptVersion}`,
    `Classifier cache: ${report.classifier.cacheDir}`,
    `Doc patch cache: ${report.docPatch.cacheDir}`,
    "",
    "## Summary",
    "",
    `- Total commits: ${report.summary.totalCommits}`,
    `- Considered commits: ${report.summary.consideredCommits}`,
    `- Update-docs decisions: ${report.docPatch.summary.updateDocsDecisions}`,
    `- Patch records: ${report.docPatch.summary.patchRecords}`,
    `- Patches: ${report.docPatch.summary.patches}`,
    `- Applied patches: ${report.docPatch.summary.applied}`,
    `- Already applied patches: ${report.docPatch.summary.alreadyApplied}`,
    `- Doc patch cache hits: ${report.docPatch.summary.cacheHits}`,
    `- Estimated doc patch prompt tokens: ${report.docPatch.budget.estimatedPromptTokens}`,
    `- Partial failure: ${report.docPatch.partial ? "yes" : "no"}`,
    "",
  ];

  if (report.docPatch.failure) {
    lines.push(
      "## Partial Failure",
      "",
      `- Failed decision: ${report.docPatch.failure.index}/${report.docPatch.failure.total}`,
      `- Failed commit: ${report.docPatch.failure.shortSha} ${report.docPatch.failure.subject}`,
      `- Successful records before failure: ${report.docPatch.failure.appliedRecords}`,
      `- Error: ${report.docPatch.failure.message}`,
      "",
    );
  }

  if (report.docPatch.summary.patchRecords === 0) {
    lines.push("No update_docs decisions produced documentation patches.", "");
    return `${lines.join("\n")}\n`;
  }

  lines.push("## Generated Patches", "");
  for (const record of report.docPatch.records) {
    lines.push(
      `### ${record.shortSha} - ${record.subject}`,
      "",
      `- Summary: ${record.summary}`,
      `- Evidence: ${record.decision.evidenceNote}`,
      `- Target docs: ${record.docTargets.length > 0 ? record.docTargets.join(", ") : "none"}`,
      `- Target source: ${record.docTargetSource}`,
      `- Cache: ${record.cache.hit ? "hit" : "miss"}${record.cache.reason ? ` (${record.cache.reason})` : ""} (${record.cache.path})`,
      `- Invocation mode: ${record.codex.mode}`,
      `- Codex usage: ${formatUsage(record.codex.usage)}`,
      "",
    );
    if (record.applications.length === 0) {
      lines.push("Applications:", "- none", "");
    } else {
      lines.push("Applications:");
      for (const application of record.applications) {
        lines.push(`- ${application.path}: ${application.status} - ${application.rationale}`);
      }
      lines.push("");
    }
  }
  return `${lines.join("\n")}\n`;
}

function renderFullMarkdown(report) {
  const lines = [
    "# Documentation Drift Full Sweep",
    "",
    `Run: ${report.run.id}`,
    `Output: ${report.run.outDir}`,
    `Dry run: ${report.dryRun ? "yes" : "no"}`,
    `Action: ${report.sweep.action}`,
    `Base: ${report.base.ref} (${report.base.sha.slice(0, 12)})`,
    `Head: ${report.head.ref} (${report.head.sha.slice(0, 12)})`,
    `Checkpoint: ${report.checkpoint.file}${report.checkpoint.advanced ? ` -> ${report.checkpoint.after.sha.slice(0, 12)}` : " unchanged"}`,
    "",
    "## Summary",
    "",
    `- Total commits: ${report.summary.totalCommits}`,
    `- Considered commits: ${report.summary.consideredCommits}`,
    `- Skipped merge commits: ${report.summary.skippedMergeCommits}`,
    `- Skipped empty commits: ${report.summary.skippedEmptyCommits}`,
    `- Skipped docs-only churn commits: ${report.summary.skippedDocsOnlyCommits}`,
    `- Update-docs decisions: ${report.docPatch?.summary?.updateDocsDecisions ?? "not run"}`,
    `- Applied patches: ${report.docPatch?.summary?.applied ?? "not run"}`,
    `- PR: ${report.sweep.prUrl ?? "none"}`,
    "",
    "## Lifecycle",
    "",
  ];
  for (const step of report.lifecycle) {
    lines.push(`- ${step.status}: ${step.name}${step.command ? ` (${step.command})` : ""}${step.note ? ` - ${step.note}` : ""}`);
  }
  lines.push("");
  return `${lines.join("\n")}\n`;
}

export function writeOutputs(report, outDir) {
  const absOutDir = path.resolve(outDir);
  mkdirSync(absOutDir, { recursive: true });
  const stem =
    report.mode === "full"
      ? "docdrift-full"
      : report.mode === "generate-docs"
        ? "docdrift-generate"
        : report.mode === "classify"
          ? "docdrift-classify"
          : "docdrift-sweep";
  writeFileSync(path.join(absOutDir, `${stem}.json`), `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(path.join(absOutDir, `${stem}.md`), renderMarkdown(report));
}

# Repeatable Hotspot Analysis Runbook

This runbook turns the Phase 1 through Phase 3 hotspot method into a repeatable repo-local workflow.
Use it to decide what to inspect before a cleanup plan, and to compare whether a cleanup PR reduced
review load after it lands. It is triage evidence only; it does not authorize runtime refactors by
itself.

## Quick Run

From a clean worktree, refresh the target ref and generate a full JSON report:

```bash
git fetch origin main
node scripts/hotspot-analysis.mjs --base-ref origin/main --recent-days 14 --limit 0 --output /tmp/rts-hotspots-current.json
```

Show the top current files, group summaries, and stale raw paths without extra dependencies:

```bash
node -e 'const r=require("/tmp/rts-hotspots-current.json"); console.table(r.current_file_hotspots.slice(0,20).map(({rank,file,group,hotspot_score,non_empty_loc,touches,recent_churn,recent_cochange_degree})=>({rank,file,group,hotspot_score,non_empty_loc,touches,recent_churn,recent_cochange_degree}))); console.table(r.architectural_group_hotspots.slice(0,12).map(({group,files,non_empty_loc,touches,recent_churn,recent_external_cochange_degree})=>({group,files,non_empty_loc,touches,recent_churn,recent_external_cochange_degree}))); console.table(r.stale_raw_path_hotspots.slice(0,10).map(({path,raw_churn,current_replacement_hint})=>({path,raw_churn,current_replacement_hint})));'
```

The default window is 14 days because the Phase 1 baseline used a two-week view. Use 7 days for an
active feature burst and 30 days when churn is lower or a cleanup follows a quieter area.

## What The Script Measures

`scripts/hotspot-analysis.mjs` emits `hotspots-analysis-v1` JSON with:

- current source files under `server/`, `client/`, `tests/`, and `scripts/` with `.rs`, `.js`,
  `.mjs`, `.sh`, `.css`, and `.html` extensions;
- generated, build, fixture, replay, and artifact path fragments excluded from the default ranking;
- current LOC and non-empty LOC from the selected ref;
- per-current-file `git log --follow --find-renames=40% --numstat` touches and churn;
- recent touches, recent churn, and recent co-change pairs from the selected recent window;
- fix/regression-looking commit-subject counts as a weak defect-pressure signal;
- architectural group summaries using [group-map.md](group-map.md);
- raw no-rename path churn and stale raw paths so old moved files do not dominate the cleanup list;
- rename events affecting source files.

The score is a damped triage blend, not a verdict. Read the top files and groups before opening a
cleanup plan, especially when a high score belongs to a mirrored contract, room runtime, or broad
test aggregate.

## Compare Before And After A Cleanup

Before a cleanup branch starts:

```bash
node scripts/hotspot-analysis.mjs --base-ref origin/main --recent-days 14 --limit 0 --output /tmp/rts-hotspots-before.json
```

After the cleanup branch is ready, run the same report against the branch head:

```bash
node scripts/hotspot-analysis.mjs --base-ref HEAD --recent-days 14 --limit 0 --output /tmp/rts-hotspots-after.json
```

Compare the same logical groups rather than only the old file path:

```bash
node - <<'NODE'
const before = require("/tmp/rts-hotspots-before.json");
const after = require("/tmp/rts-hotspots-after.json");
const groups = new Set([
  ...before.architectural_group_hotspots.map((row) => row.group),
  ...after.architectural_group_hotspots.map((row) => row.group),
]);
const byGroup = (report) => new Map(report.architectural_group_hotspots.map((row) => [row.group, row]));
const beforeGroups = byGroup(before);
const afterGroups = byGroup(after);
const rows = [...groups].sort().map((group) => {
  const oldRow = beforeGroups.get(group) ?? {};
  const newRow = afterGroups.get(group) ?? {};
  return {
    group,
    files: `${oldRow.files ?? 0} -> ${newRow.files ?? 0}`,
    loc: `${oldRow.non_empty_loc ?? 0} -> ${newRow.non_empty_loc ?? 0}`,
    recent_churn: `${oldRow.recent_churn ?? 0} -> ${newRow.recent_churn ?? 0}`,
    external_degree: `${oldRow.recent_external_cochange_degree ?? 0} -> ${newRow.recent_external_cochange_degree ?? 0}`,
  };
});
console.table(rows);
NODE
```

A useful cleanup usually lowers the amount of code a reviewer must load at once while keeping the
same logical group easy to find. A file split that only makes one old path disappear is not enough;
check the group summary, top files, and recent coupling to see where the responsibility moved.

## Interpreting Results

- Treat the top 10 to 20 current-file rows as the first reading queue.
- Treat groups with high recent churn and high external co-change degree as contract or
  orchestration surfaces, even if no single file is now huge.
- Ignore stale raw paths as direct cleanup targets unless they point to a current replacement. They
  are mainly evidence that a previous move disrupted path-level history.
- Use `git blame -w -M -C -C -- <file>` on the top rows only when line freshness or moved-line
  origin matters. The repeatable script intentionally omits blame to stay cheap for planning.
- For mirrored protocol or balance groups, require parity and design-doc review before any later
  extraction plan. For room runtime, coordinate with the active room owner before moving code.

## Rerun Cadence

Rerun the workflow:

- before creating any broad cleanup plan;
- after each cleanup PR lands, using `origin/main` at the new head;
- after large file moves, crate splits, or test-suite splits;
- monthly during active cleanup work if no major split has landed.

The most useful thresholds from the Phase 1 baseline are: top-20 current-file rank, hotspot score
above roughly 50, recent churn above roughly 2,000 lines, and recent co-change degree above roughly
100. These numbers are triage thresholds, not acceptance criteria.

## Current Follow-Up Recommendation

The Phase 3 ranking still stands. Create the next cleanup plan around splitting
`tests/client_contracts.mjs` by contract area while preserving `node tests/client_contracts.mjs` as
the stable command and tracking future `tests/client_contracts/**` files under the
`protocol-and-contracts` group.

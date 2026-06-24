# Hotspot Analysis

Use hotspot analysis to decide what to inspect before a cleanup plan and to compare whether a
cleanup PR reduced review load after it lands. It is triage evidence only: a high score does not
authorize runtime refactors, protocol movement, balance movement, or test rewrites by itself.

Completed historical evidence from the first hotspot triage lives under
`plans/archive/hotspots/`. Active implementation plans belong under `plans/<one-word-name>/`;
create one only after the current evidence supports a concrete cleanup sequence.

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

The default window is 14 days because the first baseline used a two-week view. Use 7 days for an
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
- architectural group summaries using the groups in this document and the script-local
  `GROUP_RULES` table;
- raw no-rename path churn and stale raw paths so old moved files do not dominate the cleanup list;
- rename events affecting source files.

The score is a damped blend, not a verdict:

```text
100 * (
  0.22 * sqrt(non_empty_loc / max)
  + 0.23 * sqrt(rename_aware_touches / max)
  + 0.22 * sqrt(total_churn / max)
  + 0.13 * sqrt(recent_churn / max)
  + 0.10 * sqrt(fix_like_touches / max)
  + 0.10 * sqrt(recent_cochange_degree / max)
)
```

The square-root damping keeps very large files from completely hiding smaller files that are
frequently co-changed. Use the score to choose what to read first, not to make a cleanup decision by
itself.

## Interpreting Results

- Treat the top 10 to 20 current-file rows as the first reading queue.
- Treat groups with high recent churn and high external co-change degree as contract or
  orchestration surfaces, even if no single file is now huge.
- Ignore stale raw paths as direct cleanup targets unless they point to a current replacement. They
  are mainly evidence that a previous move disrupted path-level history.
- Use `git blame -w -M -C -C -- <file>` on the top rows only when line freshness or moved-line
  origin matters. The repeatable script intentionally omits blame to stay cheap for planning.
- For mirrored protocol or balance groups, require parity and design-doc review before any later
  extraction plan.
- For room runtime, coordinate with the active room owner before moving code.
- Treat aggregate test files as possible test-suite organization problems before assuming runtime
  coupling.

Useful triage thresholds from the first baseline were top-20 current-file rank, hotspot score above
roughly 50, recent churn above roughly 2,000 lines, and recent co-change degree above roughly 100.
These numbers are triage thresholds, not acceptance criteria.

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

## Rerun Cadence

Rerun the workflow:

- before creating any broad cleanup plan;
- after each cleanup PR lands, using `origin/main` at the new head;
- after large file moves, crate splits, or test-suite splits;
- monthly during active cleanup work if no major split has landed.

## Architectural Groups

The script-local `GROUP_RULES` table is the executable source for matching. This section is the
reviewable version that explains why each group exists and how to extend it after future splits.

| Group | Current paths and future split paths | Why it is grouped |
| --- | --- | --- |
| `protocol-and-contracts` | `server/crates/protocol/**` including `server/crates/protocol/src/contract_metadata.rs`, `server/crates/protocol/src/messagepack_frame.rs`, and `server/crates/protocol/src/compact_snapshot.rs`, `server/crates/contract/**`, `server/src/protocol.rs`, `client/src/protocol.js`, `client/src/protocol_constants.js`, `client/src/protocol_frame.js`, `client/src/protocol_snapshot.js`, future `client/src/protocol_*.js`, future `client/src/protocol/**`, `tests/protocol_parity.mjs`, `tests/client_contracts.mjs`, future `tests/client_contracts/**` | Rust protocol, JS protocol, compact codecs, parity tests, and the broad client contract runner co-change as one wire-contract surface. |
| `balance-and-config` | `server/crates/rules/src/balance.rs`, `server/crates/rules/src/balance/**`, `server/crates/rules/src/defs.rs`, `server/crates/rules/src/faction.rs`, `server/src/config.rs`, `server/crates/sim/src/config.rs`, `client/src/config.js`, `client/src/config/**`, future `client/src/config_*.js`, `scripts/check-faction-catalog-parity.mjs`, `scripts/check-wiki.mjs` | Rust rules are authoritative and the client config mirror is player-visible; split files must still be reviewed as one balance mirror. |
| `server-lobby-runtime` | `server/src/lobby/**` | Room lifecycle, session policy, projection, participants, lab/replay/live flow, and room-task helper splits stay one runtime ownership area. |
| `sim-command-service` | `server/crates/sim/src/game/services/commands.rs`, future `server/crates/sim/src/game/services/commands/**`, `server/crates/sim/src/game/command.rs`, `server/crates/sim/src/game/commands.rs` | Command input validation, command DTOs, planner adapters, and command-service tests should be compared together after extraction. |
| `sim-tests` | `server/crates/sim/src/game/tests.rs`, future `server/crates/sim/src/game/tests/**` | Broad `Game` API tests can be split by behavior family without changing their logical ownership. |
| `sim-movement-service` | `server/crates/sim/src/game/services/movement/**` | Movement tests and helpers already have their own service boundary. |
| `sim-combat-service` | `server/crates/sim/src/game/services/combat/**` | Combat tests and helpers already have their own service boundary. |
| `sim-services` | other `server/crates/sim/src/game/services/**` files | Shared sim service helpers that are not command, movement, or combat specific. |
| `sim-core` | remaining `server/crates/sim/**` files | `Game`, setup, systems, projection, stores, and other sim crate surfaces that are not narrower service groups. |
| `ai` | `server/crates/ai/**` | AI decision, self-play, fixture, and profile code often moves together and has environment-gated coverage. |
| `client-match-shell` | `client/src/match.js`, `client/src/match_*` app-shell collaborators, app/frame/health/replay/pause/observer shell helpers | Match composition, frame order, teardown, and live/replay/lab shell behavior should stay one review group. |
| `client-hud` | `client/src/hud.js`, `client/src/hud_*.js`, `client/src/resource_icons.js` | HUD rendering and command-card helpers are player-facing DOM/control surfaces with shared contracts. |
| `client-state-model` | `client/src/state.js`, state query/effect helpers, `client/src/client_intent.js`, `client/src/command_budget.js`, command composer, prediction, progress, and sim-wasm adapter helpers | Client model, intent, command-budget, and prediction state must stay separate from renderer/HUD while being analyzed as one model surface. |
| `client-input` | `client/src/input/**`, `client/src/replay_camera_input.js` | Input routing has its own dependency and browser-event constraints. |
| `client-renderer` | `client/src/renderer/**`, `client/src/camera.js`, `client/src/fog.js`, `client/src/minimap.js` | Rendering, fog display, camera, and minimap co-change around frame presentation. |
| `client-ui` | `client/styles.css`, lobby, lab, settings, match-history, and other UI modules not claimed above | Remaining DOM/UI surfaces share static-client and CSS selector constraints. |
| `server-backend` | remaining `server/src/**` files | Axum, startup, backend routing, and server support code outside room-runtime helpers. |
| `scripts-tooling` | `scripts/**` | Repo-local checks, runners, and analysis tools are tooling rather than runtime gameplay code. |
| `rules` | remaining `server/crates/rules/**` files | Rules data outside the balance mirror may still co-change with balance and sim code. |
| `tests` | remaining `tests/**` files | Integration and smoke tests outside the broad client contract runner should be reviewed as test infrastructure. |

## Updating Groups

- Add new split files to the same group as the responsibility they came from before comparing
  before/after cleanup metrics.
- Keep protocol and balance mirrors grouped across Rust, JS, tests, docs checks, and parity scripts;
  do not judge one side by path-level churn alone.
- If a follow-up plan creates a new stable area, update both this document and the `GROUP_RULES`
  table in `scripts/hotspot-analysis.mjs` in the same commit.
- Treat raw stale paths as history clues, not cleanup targets. Prefer a current file or group with
  current LOC, recent churn, and recent co-change evidence.

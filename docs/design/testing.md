## 9. API-driven self-play test harness

The automated self-play harness is a **test-only** layer in `rts_ai::selfplay`. It is intentionally
separate from the simulation core: gameplay AI is a player feature, while self-play is a
regression harness for exercising the public simulation API.

**Contract.** Self-play scripts may only drive the game through the `Game` seam in §3.1:
`start_payload()`, `snapshot_for(player)`, `enqueue(player, SimCommand)`, `tick()`,
`alive_players()`, and `tick_count()`. Scripts observe the same fog-filtered snapshots a client
would receive and issue ordinary domain commands. They must not mutate entities, players, map state, or
private system internals. This keeps the simulation architected for future API clients, replay
tools, and external test drivers without adding a second privileged control path.

**Command log replay.** `Game` records every command at the authoritative apply tick, after callers
have enqueued human, scripted, or AI commands and before systems apply the pending queue.
`game/replay.rs` translates that wire-compatible log into `SimCommand`s, restores a fresh `Game`
from the replay artifact's checkpoint-backed start composition with AI thinking disabled, and
compares the resulting event stream and final per-player snapshots. Replay and live play use the
same typed command application path, so a replay proves both the recorded command artifact and the
deterministic simulation ordering. Preserved schema 2 incident replay files are historical evidence,
not supported replay fixtures; current replay workloads use schema 3 captures. Entity iteration and
A* tie-breaking must remain stable; avoid hash-order-dependent simulation behavior.

**Derived-state rebuild coverage.** The test-only `Game` rebuild seam clears the persistent
`PathingService` cache and rebuilds the final spatial index from authoritative entities at a tick
boundary. The paired-game derived-state test warms pathing, rebuilds derived state in one copy, and
then compares semantic authoritative state plus player, full, and spectator snapshots while both
games continue ticking and repathing.

**Fast/full AI split.** Plain `cargo nextest run --config-file .config/nextest.toml --manifest-path
server/Cargo.toml --profile default` keeps the self-play harness in the default gate, but only runs
the fast scripted coverage. Long profile-backed and real-AI self-play tests return early unless
`RTS_FULL_AI_TESTS=1` is set; `tests/run-all.sh --full-ai` enables that mode for the full
orchestrator.
`RTS_SELFPLAY_FULL=1` remains accepted as an alias for manual self-play runs. Use full AI coverage
when touching AI strategy, profile-backed self-play, replay determinism, or balance behavior that
depends on long matches.

AI arena runs that end with no winner because of elimination remain unresolved elimination draws;
they are not scored using the tick-cap army-value tiebreak. An arena run is rejected when distinct
candidate and baseline requests resolve to the same concrete profile for a seed.

**Profile-backed coverage.** The long profile-backed tests spawn AI-profile players through the
self-play adapter and run matches headlessly under `RTS_FULL_AI_TESTS=1 cargo nextest run
--config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default`. The
profiles gather steel and oil, construct supply and tech structures, train Riflemen, Scout Cars, and
Tanks, and launch attack-move waves at public enemy start tiles. The self-play adapter owns
harness-only state such as pending build intents, failed build spots, and staging/attack guards
needed to interpret fog-filtered snapshots without duplicating profile strategy logic. The harness
checks per-tick invariants for invalid resources, supply overflow, malformed entity snapshots,
out-of-bounds positions, and non-finite progress values. It also enforces progress deadlines so a
stuck economy/tech/combat loop fails as a deadlock instead of timing out silently.

Special harness scripts remain where they cover behavior that is not a normal AI strategy profile:
`WorkerRushScript` is an all-in worker-pull scenario, and `MineOnlyScript` is passive mining/fairness
coverage. These scripts are kept isolated from the canonical profile list.

**Artifacts.** On failure, the test writes `target/selfplay-failures/<test>-<pid>-<time>/`
with:
- `replay.json`: a normal `ReplayArtifactV1` command-log replay artifact, loadable through the
  same replay runtime as post-match and match-history replays. New artifacts use schema 3 with a
  launch-time `startState.checkpointPayload` (`GameCheckpointV1`) plus the recorded command stream;
  schema 2 and older artifacts are intentionally rejected by the current replay loader.
  Team-capable artifacts preserve `players[].teamId`, `winnerTeamId`, and team-aware final score
  rows; old singleton-FFA artifacts without team fields still load through compatibility defaults.
- `diagnostic.json`: self-play-only start payload, script decision log, event log, milestone state,
  and sampled snapshot summaries.
- `summary.log`: short human-readable failure summary and missing milestones.

The replay artifact is meant to be enough to reproduce or inspect a failing run without manually
playtesting first. Load an artifact with
`/?replayArtifact=<artifact_name>` on a local server using the same Cargo target directory. The
older `/dev/replay-artifact?replay=<artifact_name>` route redirects to that canonical launch URL.
For DB-free group replay lobby coverage, `POST /dev/replay-lobby?replay=<artifact_name>` loads the
same safe artifact directories and returns a `__match_replay__:*` staging room without exposing the
artifact JSON; the normal saved-artifact URL still uses immediate replay confirmation/playback.
By default successful runs do not write artifacts. For manual inspection,
setting `RTS_SELFPLAY_SAVE_REPLAY=1` writes a successful run to
`target/selfplay-artifacts/<test>-<pid>-<time>/`; setting `RTS_SELFPLAY_SAVE_REPLAY=<name>` uses
that explicit safe artifact name instead.

**Profile matchup CLI.** The `ai-matchup` binary is the manual fixed-horizon matchup facility for
profile-vs-profile runs. It composes the same self-play adapter and `Game` seam as the tests, runs
one directed match until a starting City Centre objective win or the tick cap, optionally verifies
deterministic replay, and can write a replay artifact. No winner by the default 25,000-tick horizon
is a draw; army value and other material metrics are diagnostics, not tiebreakers:

```bash
cd server
cargo run --bin ai-matchup -- ai ai
cargo run --bin ai-matchup -- ai_2_1 ai_turtle --seed 7 --ticks 3000 --json
cargo run --bin ai-matchup -- default ai_turtle --seed 7 --ticks 25000 --json
cargo run --bin ai-matchup -- --list-profiles
```

The `ai-arena` binary is the agent-facing profile comparison layer. It runs side-swapped seed pairs
around the same profile matchup result, defaults to AI 2.1 against AI Turtle, and writes a
top-level `arena-summary.json` plus per-run sidecars:

```bash
cd server
cargo run --bin ai-arena -- --candidate ai_2_1 --baseline ai_turtle --seeds 3 --ticks 9000
cargo run --bin ai-arena -- --candidate ai_turtle --baseline ai_2_1 --out-dir target/ai-arena
```

Each run directory contains a deterministic `replay.json` plus `manifest.json`, `summary.json`,
`decision-trace.jsonl`, and `brief.md`. The manifest records canonical profile identities and
fingerprints; the brief is the first artifact agents should read before opening the replay or
searching trace labels.

Keep fast invariant-style milestone coverage in `cargo nextest run`; use
`RTS_FULL_AI_TESTS=1 cargo nextest run --config-file .config/nextest.toml --manifest-path
server/Cargo.toml --profile default`
for the long regression gate and the CLI for balance exploration, seed sweeps, and strategy result
sampling.

## 10. Dev scenario inspection

Game-backed dev scenarios are live, no-fog watcher rooms for inspecting authored simulation
situations through the normal Pixi client. Start a local server, then open the index:

```bash
open "http://localhost:<port>/dev/scenarios"
```

The index lists every supported launch and links to the current URL shape:

```text
/dev/scenarios?id=<scenario_id>&unit=<unit>&count=<count>[&blocker=<unit|none>][&case=<case>]
```

The handler redirects into the normal client with `watchScenario=1`; the client auto-joins a
reserved spectator room named:

```text
__dev_scenario__:<scenario_id>:unit=<unit>:count=<count>[:blocker=<unit|none>][:case=<case>]
```

Current scenario ids:

- `dynamic_construction_path_block` — two workers receive simultaneous orders: one moves 20 tiles
  while the other starts a Barracks across its already-planned route; selectable `head_on`,
  `slight_angle`, and `major_angle` cases cover static-obstruction recovery across approach angles.
- `scout_car_snaking_corridor` — movement/pathing through the snaking stone corridor.
- `direct_reverse_order` — one vehicle ordered directly behind its current facing.
- `scout_car_wall_chokepoint` — vehicle groups moving through a narrow wall gap.
- `vehicle_corner_wall` — vehicle groups cornering around a wall spur.
- `vehicle_small_block_baseline` — vehicles moving through optional small-unit blockers.
- `factory_zero_gap_perpendicular` — one vehicle starting flush against a factory and moving east.
- `command_car_building_corner` — one Command Car entering the reduced three-building corner from
  the Soupman match reproduction.
- `command_car_building_corner_west_southwest` — the identical Command Car corner reproduction
  with its target ten tiles west and four tiles south of its starting position.
- `factory_wall_rally_spawn` — one completed Factory vehicle spawning below a two-tile terrain wall
  and rallying almost due west, matching replay 104 tick 7923 geometry.
- `tank_trap_line_horizontal` — Training Centre, engineers, one rifleman, and one vehicle for
  manually building a horizontal Tank Trap line before the test units try to cross.
- `tank_trap_line_vertical` — Training Centre, engineers, one rifleman, and one vehicle for
  manually building a vertical Tank Trap line before the test units try to cross.
- `tank_trap_line_diagonal` — Training Centre, engineers, one rifleman, and one vehicle for
  manually building a diagonal Tank Trap line before the test units try to cross.
- `tank_trap_pathing_matrix` — one dropdown-backed matrix scenario with selectable cases:
  `friendly_vehicle_reroute`, `enemy_vehicle_reroute`, `infantry_pass_through`, and
  `explicit_infantry_attack`.
- `entrenchment_inspection` — seeded neutral trenches, researched friendly infantry, friendly and
  enemy eligible trench reusers, and a Machine Gunner for crowded slotting/rendering checks.
- Panzerfaust inspection is composed in Lab: enable `panzerfausts` research for the owner,
  spawn Panzerfausts plus Scout Car/Tank/Command Car targets, and compare loaded versus spent art,
  cancellable windup, detached impact, exact target filtering, and explicit-Attack pursuit behavior.
  The bundled `render-preview` and `supply-300-hellhole` scenarios include loaded Panzerfausts for
  renderer and high-density coverage.
- `tank_coax_inspection` — one held Tank with its cannon cooldown delayed faces infantry-priority
  targets, support weapons, Ekat/Golem units, armored fallback targets, blockers, resources, smoke,
  and buildings around the coax arc for secondary-machine-gun inspection.
- `attack_move_reload_acquisition` — one Tank begins with its cannon and coax reloading, then after
  a ten-second inspection pause receives a real attack-move through an invulnerable enemy Tank
  already inside its current moving-range boundary. On the known failure, it keeps advancing until
  reload permits fresh acquisition; after a fix, the same scene should stop at the initial boundary
  and wait there to fire.
- `tank_under_fire_retreat` — one reinforced inspection Tank takes frontal fire from a deployed
  Anti-Tank Gun, then after 20 seconds receives a long move order directly behind it. The baseline
  captures the current 180-degree pivot before retreat; future under-fire reverse behavior should
  back away immediately.
- `tank_reverse_traffic` — three outward-facing Tanks take frontal fire from three deployed
  Anti-Tank Guns, then after 20 seconds receive individual movement orders across the shared
  center. Their future reverse trajectories intersect, providing a rearward traffic-control
  inspection scene. The scenario-only Tanks have extra health so sustained fire does not end the
  observation early.

The watcher shows movement debug path overlays by default. Replay speed controls are reused for
dev scenarios: `Pause` sets the simulation speed to zero, and `Step` advances exactly one
authoritative tick while paused. Normal seek/reset controls are replay-only.

Scenario setup is server-side only under `server/crates/sim/src/game/setup/dev_scenarios.rs`; do
not expose arbitrary spawning or map editing through client commands. The Interact `dev-scenario`
namespace may observe, frame, screenshot, record, and time-lapse these watcher rooms; its artifacts
remain confined under `target/interact/scenario/<session-id>/`.

The Tank Trap pathing matrix scenarios are harnesses for owner-independent vehicle pathing, infantry
pass-through, explicit infantry attacks, and attack-move acquisition filtering. Vehicle path
planning, physical movement, and standability treat every live Tank Trap footprint and closed
one-tile gap as a vehicle-body blocker regardless of ownership or visibility.
Combat acquisition should prioritize a Tank Trap only when that trap lies on the vehicle's bounded
route window or closes a gap across it; irrelevant nearby traps should lose to combat targets.

## 11. Package-aware test selection policy

The authoritative full gate is the PR `./tests/run-all.sh` check from the `Main test gate` workflow. Local runs should
usually be narrower and selected by the changed files or contracts. Use
`node tests/select-suites.mjs --from=<base-ref>` or pass changed paths directly to see the expected
suites.

Lab coverage derives the expected lategame research set from the Kriegsia catalog and requires every
bundled preview scenario to grant that full set. Client fixtures treat completed research arrays as
unordered state. The lab client contract suite requires pasted JSON with the retired `labScenario`
envelope to fail locally with an explicit lab result instead of falling through to generic server
message parsing. The agent lab driver exposes its page bridge only on Lab routes, rebuilds the
server for the selected worktree, cancels interrupted startup deterministically, and transfers
daemon startup to the child through a random nonce lease before socket bind. Detached daemon startup
errors surface immediately, partial startup ownership and runtime resources are cleaned up, and idle
expiry uses monotonic time. Dead-parent locks are preserved for a bounded grace period, only verified
stale records are reclaimed, and lock release requires nonce ownership. Malformed bridge inspection
filters fail closed so invalid bounds cannot broaden a query. The driver evaluates capture fallback and render errors against the
current frame so transient startup work does not block a later clean capture, and clamps seeks to
the supported range.

Lab preview contracts require a 24-hour-or-longer artifact TTL and prove that a published URL remains
fetchable after the Lab publisher closes and its originating worktree is removed. CLI coverage also
proves daemon shutdown leaves issued screenshot links valid, while concurrent recording wait/stop
callers receive the same deduplicated durable publication.

Interact source, page-bridge, Rust artifact-bridge, focused-test, CLI-documentation, and local
skill changes select `interact-contracts` plus the browser `client-smoke` shard. The fast
Node/static gate installs the root lock and runs the strict no-emit Lab TypeScript check before the
contracts. Node-side Lab implementation sources execute directly as `.ts` on Node 22.18+, while the
browser bridge and tests remain buildless JavaScript. The fast contracts use the fake driver and
isolate UUID-named session and portable artifacts, while the one
live canary exercises open, spawn, update, order/step, screenshot/PNG preview, setup round trip,
short H.264 recording, reset, close, stale-session rejection, and daemon shutdown. Standalone live
canary runs own a private server; the browser shard passes its existing loopback server through
`RTS_INTERACT_LAB_BASE_URL` so CI does not build or start a second one.

- Phase runner plan/path handling: run `node tests/phase_runner_agents.mjs` when changing
  `scripts/phase-runner*.mjs` or phased plan path handling, including slash-separated nested plan
  names, sanitized worktree/log slugs, executor model inheritance, and generated `codex exec`
  arguments.
- Agent PR passes / adversarial quality workflow: run `node tests/agent_pr_passes.mjs` and
  `node tests/adversarial_quality_pass.mjs` when changing `scripts/agent-pr-passes.mjs`, its
  configured passes, `scripts/adversarial-quality-pass.mjs`, their schemas, or agent PR wiring.
  `scripts/agent-pr.sh` runs manifest-ordered specialist passes before the final adversarial review.
  Each pass may select its own model through the manifest's `modelEnv`; the patch-note pass uses
  `RTS_PATCH_NOTES_MODEL` when set and otherwise lets Codex choose its default. It cheaply skips
  branches without runtime paths that may affect players, and qualifying branches receive one
  fragment at `patch-notes/YYYY-MM-DD/<branch-slug>.md` before final review.
  Dry-run coverage should keep preview generation non-mutating before clean/fetch checks, and nested
  Codex quality-pass coverage should verify access to linked worktree git common directories while
  marking the environment so `scripts/agent-pr.sh` refuses recursive PR lifecycle calls.
  Branch-handling changes should also include a dry-run `--head-branch` mismatch check that fails
  before Codex, push, or status work runs. PR-helper changes should preserve the Markdown
  quality-pass report in the owned PR body so the status has a durable audit trail. Pure Markdown
  branches, defined as every changed file ending in `.md` regardless of directory, must skip Codex
  adversarial review while still pushing the branch, posting the `adversarial-quality-pass` success
  status, and recording a docs-only skip report in the PR body. Include
  `bash -n scripts/agent-pr.sh tests/run-all.sh && node --check scripts/adversarial-quality-pass.mjs`
  for shell and JS syntax coverage.
- Net-report incident packaging: run `node tests/net_report_log_parser.mjs` when changing
  `scripts/parse-net-report-logs.mjs` or `scripts/net-report-incident-package.mjs`. Run
  `node tests/net_report_incident_capture.mjs` when changing `scripts/capture-net-incident.mjs`.
  Capture coverage must verify that `--force` replaces generated capture-package directories and
  refuses to recursively remove directories that do not look like generated capture packages.
  Parser coverage must preserve numeric per-match fixture labels, prefer `match_run_id` labels for
  nonnumeric combined log artifacts, and limit per-match coverage rows to the matching run id when
  one source log contains multiple matches.
- `rts-contract` or `rts-protocol`: run Rust contract/protocol tests, compact snapshot tests, JS
  protocol mirror/decode tests, and Node integration when a top-level message or compact shape
  changed.
- `rts-rules`: run rules tests plus sim tests that consume stats/formulas. If visible balance
  values changed, run client config/protocol mirror checks and include factual player-facing patch
  notes.
- Client match-shell combat audio: synthetic self-target `Attack` events used for fog-safe artillery
  firing reveals update visual reveal and recoil state without playing attack audio. Normal
  point-fire artillery attacks still play combat audio.
- Faction guardrails: run `node scripts/check-faction-assumptions.mjs` for faction docs, lifecycle
  policy, lobby admission, protocol/config vocabulary, or checker changes. Run
  `node scripts/check-faction-catalog-parity.mjs` when faction catalog facts, the Rust catalog dump,
  or client mirrors can change, including `server/crates/rules/src/faction.rs`,
  `server/crates/rules/src/bin/dump-faction-catalog.rs`, `client/src/config.js`,
  `client/src/lobby_view.js`, protocol/config mirror files, or the catalog parity checker itself.
  Docs-only faction policy edits should select these guardrails without requiring live-server
  suites.
- `rts-sim`: run sim package tests, deterministic replay coverage, and live-server integration for
  changed behavior that crosses the room/network boundary. Run
  `node scripts/check-lobby-architecture.mjs` when changing lobby room-task ownership or mutation
  boundaries. Tank Trap blocker/pathing changes should include the focused gap/pathability
  regression and constructible horizontal, vertical, and diagonal dev scenario coverage.
- SVG legacy unit renderer oracle: run `node tests/legacy_unit_visual_oracle.mjs` when legacy unit
  rendering behavior or `tests/fixtures/svg/legacy-unit-oracle.baseline.json` changes. The oracle
  uses a deterministic Node fixture, semantic measurements, and bounded pixel-diff thresholds across
  current unit kinds and representative animation states.
- Lab panel controls: run `node tests/client_contracts.mjs` and
  `node scripts/check-client-architecture.mjs` when changing `client/src/lab_panel.js` player setup,
  spawn, resources, research, or result re-render behavior.
- Client performance harness: run `node --check scripts/client-perf-harness.mjs`,
  `node scripts/client-perf-harness.mjs --list`, and
  `node tests/client_contracts/client_flamegraph_contracts.mjs` when changing the fixed browser
  performance workloads, stress-matrix dimensions, CPU/flame-graph capture, harness script, or
  documented performance workflow. Workload execution uses local headless Chrome by default and
  writes bounded JSON summaries under `target/client-perf`; it is measurement-only and does not add
  FPS gates. The offline Hellhole stream remains in the default workload set as active Player 1's
  fog-filtered 2v2 projection, while the live
  server/client Hellhole is opt-in and requires an explicit workload id. The integrated launcher
  exposes its controlled Chrome window for visual inspection without changing workload identity.
  Stress-matrix runs vary CPU throttle, viewport, DPR, and repeat count, then write JSON and
  Markdown rollups.
  Render-lag summaries report advisory 60/120/240/480 FPS frame-work budget targets, p95 margin,
  next missed headroom budget, grouped render diagnostics, and long-frame context from local
  evidence instead of portable RAF FPS claims. `ClientNetReport` uploads are unchanged by these local
  artifacts. The actual client target is 240 FPS/4.17 ms on the reference machine because it is a
  planning proxy for approximately 60 FPS/16.67 ms on hardware with one quarter of the performance;
  it is not a literal portable FPS certification. Comparisons must preserve the workload's complete
  per-frame presentation semantics and cadence. Reduced entity/fog/animation/overlay update rates,
  intentional staleness, cross-frame staggering, or unmeasured relocation of main-thread work do not
  satisfy the target. Exact redundant-work elimination remains valid when the same state is reflected
  without added latency and deterministic pixel parity is preserved.
- Hellhole server performance: run `scripts/hellhole-perf-harness.sh --ticks 900` in release mode.
  This direct API-in/API-out lane includes simulation, full-world projection, compaction, and
  MessagePack encoding plus pre-tick scripted commands and respawn placement, but no room task,
  network transport, browser, or wall-clock pacing. Its JSON reports shuttle commands/selected
  units, deaths, respawn batches/units, and the minimum outgoing snapshot entity count. The entity
  invariant is checked after pre-tick actions; a death tick's outgoing snapshot may intentionally
  be lower until the following pre-tick restores the roster. Use
  `scripts/hellhole-perf-harness.sh --integrated` only when the live Lab server and visible Pixi
  client need to be inspected together; do not use its timings as isolated server evidence. The
  old static fixture's `realtime_factor >= 8.0` target is not a gate for the new command/death churn
  workload. Compare the full counter shape and timings like-for-like on the reference machine until
  a repeated churn baseline establishes a new headroom target. The canonical scenario is 2v2
  (`1+3` versus `2+4`); the server lane keeps its full-world serialization pressure while the
  checked-in client stream uses Player 1's normal team-fog projection.
- Transparent SVG rig pixel gates: run `node tests/transparent_unit_pixels.mjs --parts --no-artifacts`
  when SVG rig runtime/schema behavior, rig importer fixtures, or transparent unit pixel comparisons
  change. The harness compares Worker and Tank part and composition samples.
- Team-aware authored start assignment is covered by `cargo nextest run map` for deterministic
  FFA compatibility, fixed-start proximity, 1v2/1v3 team assignment, unconditional base-site
  materialization, start payload team ids, and replay reconstruction. Run `node tests/team_integration.mjs` for the
  live lobby/start contract.
- `tests/team_integration.mjs` is the canonical live multi-client team suite. It requires a running
  server and covers default singleton FFA, solo sandbox starts, scripted `1v2`/`1v3`/`2v2` setup,
  host-only/invalid team mutation rejection, shared team snapshot vision, allied command-authority
  no-ops, allied attack rejection, and team victory/game-over semantics. `tests/run-all.sh --no-rust`
  includes this suite in the live Node API pass, so a final local gate already exercises it.
- `rts-ai`: run AI package tests and `node tests/ai_integration.mjs`. Run
  `RTS_FULL_AI_TESTS=1 cargo nextest run --config-file .config/nextest.toml --manifest-path
  server/Cargo.toml --profile default` or
  `tests/run-all.sh --full-ai` when strategy profiles,
  profile-backed self-play, replay determinism, or long-match balance behavior changed.
  Default AI package coverage includes team-safety assertions for `teamId` observation,
  visible-ally exclusion from `visible_enemies`, allied-start exclusion from public enemy base /
  expansion safety, live alive-player target filtering, and real-AI self-play remaining
  per-player rather than shared-team controlled.
- `rts-server`: run server/lobby tests, Node live-server integration/regression suites, and client
  smoke when connection, snapshot delivery, room lifecycle, or served client behavior changes.
- `client/`: run JS protocol/client contract checks, minimap/input contracts where relevant, and
  client smoke. Include Node integration when protocol decode or network behavior changed.

`scripts/check-source-file-sizes.mjs` runs as a cheap policy gate and enforces a 1500-line cap for
Rust, JS, and MJS source/test files under `server/`, `client/src/`, `tests/`, and `scripts/`, plus
the checked-in production stylesheet at `client/styles.css`.
Files that were already above the cap are frozen in `scripts/source-file-size-baseline.json`; new
over-cap files fail, and frozen exceptions fail on growth. Shrinkage prints a ratchet note so the
baseline can be lowered or removed.

`scripts/check-crate-boundaries.mjs` is part of the gate and fails on forbidden Cargo package
edges or server-only imports in lower crates. The sim architecture ratchet is also part of the gate:
`cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` fails when
`rts-sim::game` grows new service edges, broad mutable APIs, direct state writes/usages, public API
surface, or file-size budget over the committed baseline. Prefer reducing coupling first. If the
growth is intentional, update `server/crates/archcheck/baselines/sim-architecture.json` with:

```bash
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture --bless --reason "short reason"
```

Avoid broad allowlist additions unless the same change or a tracked follow-up explains the cleanup
path. `tests/select-suites.mjs --verify` keeps the changed file mapping itself covered by small
examples. CI comments document any intentionally skipped suite; that skip becomes invalid when the
changed-file mapping selects the skipped behavior.

## 12. PR CI contract

The canonical required PR check context is `./tests/run-all.sh` in the `Main test gate` workflow.
It is an aggregate check over split coverage jobs for server binary build, Rust policy/lint, two
complementary Rust nextest partitions, live Node, and two complementary browser/tri-state shards on
pull requests targeting `main` and on pushes to `main`. The two nextest jobs use `slice:1/2` and
`slice:2/2`, so their union runs the same Rust test set as the local gate. The browser shards divide
the current PR coverage into client smoke plus phase 0.5, and phases 2.5 plus 5; each shard gets an
isolated prebuilt server. The split jobs run `tests/run-all.sh` sub-modes under CI so the required
aggregate gate preserves client smoke plus tri-state browser coverage without serializing every suite
in one runner. The server-build job uploads generated sim-WASM browser assets, and both browser
shards download them into `client/vendor/sim-wasm` before client smoke runs from its clean checkout. Local
`tests/run-all.sh` runs keep client smoke in the default browser gate but skip the latency-sensitive
tri-state browser scenarios unless `--with-tri-state-browser` or `RTS_RUN_TRI_STATE_BROWSER=1` is
set. WASM-backed tri-state groups also stay opt-in unless `RTS_RUN_WASM_TRI_STATE=1` is set. When
the generated prediction WASM glue is absent, the server serves a JavaScript fallback module that
disables prediction without a 404; generated WASM files take precedence, and other missing assets
still return 404. Client smoke reports failing response URLs with browser console resource errors.
Changed-file detection classifies PRs and `main` pushes as `docs_only`, `client_only`, or `full`
from the PR base/head range or the push before/after range. `docs_only` keeps the same check
contexts green but exits before expensive suites. `client_only` is limited to conservative
`client/` paths and skips Rust nextest, lint, and Rust architecture work while still building the
server and running live Node plus browser coverage. Contract-adjacent client paths such as the
`client/src/config.js` facade and the explicitly classified rules, faction, timing, and
player-palette mirror modules, along with `client/src/protocol.js`, `client/src/net.js`,
`client/src/lobby_view.js`, and generated sim-WASM assets, fall back to `full`. Client-owned
`client/src/config/presentation.js` remains client-only, and selector verification requires each
production config module to declare one classification. Branch protection
should require this single aggregate full-gate check unless a plan phase explicitly changes the
contract.

`node scripts/check-docs-health.mjs` runs in the early changed-files CI lane before expensive split
jobs. It validates `docs/doc-map.json`, enforces the 5 KiB `docs/context/*.md` capsule cap, and
checks local Markdown links in `docs/` and `plans/`.

The `PR ownership` workflow validates owned agent PR metadata for `zvorygin/*` branches with
`scripts/check-pr-ownership.sh`.

`scripts/agent-pr.sh` reuses the changed-file policy before opening or updating an owned PR. A
supplied `--head` value must match the current branch before the docs-only skip can push or post
status. Before that final classification, it runs the ordered entries in
`scripts/agent-pr-passes.json`; mutating passes must commit their work and leave the same branch
clean so the adversarial review covers their final output. Pass reports are preserved in the PR
body. When the resulting branch diff against `origin/main` contains only `.md` files, including Markdown
files outside `docs/`, it skips the Codex adversarial quality pass but still pushes the branch, posts
a successful `adversarial-quality-pass` status, and writes a docs-only skip report into the PR body.
Any non-Markdown changed file keeps the normal adversarial quality pass requirement.

Rust formatting is intentionally not a CI test gate. The repository pins Rust and rustfmt in
`rust-toolchain.toml`; the final quality pass invoked by `scripts/agent-pr.sh` runs
`scripts/format-touched-rust.sh` after review and before committing/pushing. It formats only Rust
files changed by the branch or quality pass, so an owned PR carries its own formatting without a
workspace-wide formatter pass or an unrelated formatter failure.

The old standalone `Rust` and `Integration` workflows are retired. Their package, architecture,
live Node, and browser coverage is owned by the split `Main test gate` jobs under the required
aggregate `./tests/run-all.sh` check, so separate auxiliary workflows would duplicate coverage and
consume extra runner capacity without increasing merge safety.

GitHub Actions uses standard `ubuntu-latest` runners for this contract. Public-repository standard
runners are acceptable for the current cost posture, while larger paid runner classes are out of
scope. The gate remains portable through `tests/run-all.sh` so it can run locally or on another
runner if the hosting or billing posture changes.

PR workflows use concurrency groups scoped by workflow plus PR number, with cancellation enabled
only for pull request events. A newer push to the same PR branch may cancel superseded runs, while
pushes to `main` and unrelated branches keep independent results.

Beta deployment is downstream of the full gate but must only deploy tested `main` push commits. The
deploy workflow checks that the completed `Main test gate` run came from a push event on `main`
before checking out and deploying the tested head SHA.

## 13. Documentation drift sweeper

`scripts/docdrift-sweep.mjs --dry-run` is the deterministic operator surface for reviewing commits
between `docs/docdrift-checkpoint.txt` or `--base` and `--head`. It reads commit metadata,
changed paths, compact diff stats, docs touched, and `docs/doc-map.json` trace-map candidates, but
does not edit docs, create PRs, or advance the checkpoint. Merge commits, empty commits, and
docs-only churn are skipped before classifier prompts are built.

`scripts/docdrift-sweep.mjs --classify` adds the cheap Codex CLI classifier. Live classifier runs
must use Codex CLI authentication through the local `codex exec` path; they must not use the
OpenAI Agents SDK, direct API clients, API-key environment variables, or API-billed fallback
routes. Fixture runs use `--no-codex --fixture <name>` and are the required focused verification
path before any live Codex smoke. Classifier decisions are cached under the ignored
`.docdrift/classifier-cache/` runtime directory by prompt version and commit SHA, and reports can be
written with `--out-dir`. Live Codex calls run read-only with approval policy forced to `never` via
Codex config override, emit per-commit progress on stderr, and record token usage when the Codex
JSON event stream includes it. Each Codex CLI invocation is wall-time bounded by
`--codex-timeout-seconds` so a wedged classifier or doc-patch generation call fails the run and
leaves the daily failure marker instead of blocking launchd indefinitely.

`scripts/docdrift-sweep.mjs --generate-docs` reruns or reuses the classifier records, selects only
`update_docs` decisions, loads targeted authoritative design-doc sections, and asks Codex CLI for
exact minimal find/replace doc patches. The generator prefers classifier-selected design docs; docs
touched in the commit and broad trace-map design docs are fallbacks, not an automatic union. It
builds and applies doc-patch prompts sequentially so later `update_docs` decisions see docs already
changed by earlier decisions in the same sweep; if the supplied sections already cover the behavior,
the generator should return an empty patch set instead of restating it. The script applies generated
patches to the working tree and writes `docdrift-generate.{md,json}` with `--out-dir`; operators
inspect the resulting docs diff before any PR lifecycle step. Fixture runs use the same
`--no-codex --fixture <name>` path and must remain idempotent. If a retry sees that a cached patch's
replacement text is already present, it reports the patch as already applied without spending
another Codex generation call. Patch application is atomic per commit: all patches for one
`update_docs` decision are validated against the current working tree before any file is written.
If one `update_docs` decision cannot generate or apply a safe patch, the report records that
decision in `docPatch.skipped[]` with the commit, error kind, and message, then continues with
later decisions. Skipped doc-patch decisions do not make the command exit non-zero; they are
visible in `docdrift-generate.{md,json}`, the full-sweep report, and any sweep PR body so humans
can decide whether to follow up manually.

`scripts/docdrift-sweep.mjs --full` is the PR-first operator lifecycle. It fetches `origin/main`,
uses the local checkpoint from `.docdrift/checkpoint.txt` when present, falls back to the committed
seed in `docs/docdrift-checkpoint.txt`, and gives each new run a unique
`zvorygin/docdrift-sweep-<run-id>` branch plus matching isolated worktree. Before creating that
branch it atomically writes `.docdrift/runs/<run-id>/run-state.json`; the schema-versioned record is
updated after every lifecycle step and is the authority for the run's base/head, generated head,
branch/worktree, PR identity and state, checkpoint target, and recovery action. With no explicit
`--run-id`, recovery resumes only the single recorded nonterminal run; multiple candidates fail
closed.

An open PR resumes its exact recorded branch, head, PR, and first incomplete step. A missing
worktree can be recreated from an exact local or remote recorded head, and the only automatic ref
reconciliation is a clean local fast-forward to that exact remote head. Merged runs may finish
their recorded idempotent checkpoint step; merged or closed-unmerged terminal runs otherwise keep
their old refs and reports while a fresh unique run starts at fetched `origin/main`. The one legacy
fixed branch, `zvorygin/docdrift-sweep`, can be adopted only when its clean local, remote, and
worktree heads agree with exactly one terminal owned PR head. Automatic adoption is restricted to
the known `68f6e958...` incident; another verified terminal head requires explicit
`--adopt-legacy`, and the legacy ref is never resumed or rewritten. A closed PR's stale
mergeability metadata is terminal evidence, not a conflict decision.

Dirty or conflicted worktrees, in-progress Git operations, non-fast-forward or mismatched heads,
open conflicted PRs, ambiguous PR/run matches, and ref or worktree collisions stop before refs,
checkpoints, or GitHub state are changed. Recovery never stashes, resets, rebases, force-pushes,
deletes, or resolves conflicts. Classification and doc generation then run on the selected safe
worktree, `scripts/agent-pr.sh` owns PR creation, and `scripts/wait-pr.sh` proves merge reachability.
The checkpoint advances atomically only after a no-PR range is fully processed or a recorded merged
run is verified.
Generated sweep PRs carry the `docdrift-sweep` label so effectiveness audits can list sweep output
separately from PRs that merely change the sweeper tooling.
Per-decision doc-patch skips do not block checkpoint advancement: once any generated docs PR merges,
or once a run has no docs changes to PR, the checkpoint advances to the processed head. This keeps
the nightly gardener from retrying the same stale patch indefinitely. Lifecycle failures such as
failed checks, closed PRs, stale branches, dirty sweep worktrees, or unrecoverable git/GitHub errors
still exit non-zero and leave the checkpoint unchanged.

Full sweeps write ignored local reports under `.docdrift/runs/<run-id>/`, including the recovery
authority `run-state.json`, `docdrift-full.{md,json}`, and any classify/generate reports. The full
report's `sweep.recoveryAction` states whether the run was created, resumed, completed after merge,
started after a terminal PR, adopted from the legacy branch, or stopped for operator review. Use
`scripts/docdrift-daily.sh` as the launchd-friendly daily command; pass normal
`docdrift-sweep.mjs` options after it, for example `--dry-run` for a lifecycle preview or
`--run-id <id>` for predictable report paths. The wrapper first fetches `origin/main`, creates or
refreshes a clean detached `.docdrift/worktrees/docdrift-runner` checkout at that ref, then runs
that latest `scripts/docdrift-sweep.mjs` with `--repo` pointed back at the primary checkout. The
primary checkout still owns `.docdrift` checkpoints, caches, reports, and sweep worktrees, while the
runner worktree prevents stale local `main` from running obsolete sweeper code. Set
`DOC_DRIFT_RUNNER_WORKTREE` to override the runner location. The wrapper defaults scheduled runs to
`--max-commits 300` so ordinary daily backlog does not trip the interactive classifier guard; set
`DOC_DRIFT_MAX_COMMITS` to override that limit. It also passes `--codex-timeout-seconds` from
`DOC_DRIFT_CODEX_TIMEOUT_SECONDS`, defaulting to 300 seconds per Codex call. When the daily command
exits non-zero, it writes an ignored `.docdrift/last-failure.md` marker with the command, UTC
timestamps, exit code, and stdout and stderr tails, and it clears that marker after the next
successful run. The wrapper only runs the command; it does not install or require a launchd job for
other developers.

The recurring daily schedule is local macOS `launchd`, not a GitHub Actions scheduled workflow.
Do not start a "did it run?" investigation in `.github/workflows`; start with the loaded user
LaunchAgent. On the primary workstation the job is installed as
`$HOME/Library/LaunchAgents/com.zvory.rts-docdrift.plist` with label `com.zvory.rts-docdrift`,
`WorkingDirectory` set to `/Users/az/Code/rts-0`, and `ProgramArguments` beginning with
`/Users/az/Code/rts-0/scripts/docdrift-daily.sh`. Its `StartCalendarInterval` is local wall-clock
time, currently `Hour = 4`, `Minute = 0`. Run artifacts use UTC ids, so a 4 a.m. run may appear as
`.docdrift/runs/YYYY-MM-DDT08-00-...Z` during daylight-saving time and
`.docdrift/runs/YYYY-MM-DDT09-00-...Z` during standard time.

For future operational checks, use this order:

```bash
launchctl list | rg -i "docdrift|rts-docdrift"
plutil -p "$HOME/Library/LaunchAgents/com.zvory.rts-docdrift.plist"
ls -lt .docdrift/runs
sed -n '1,180p' .docdrift/last-failure.md
tail -n 120 "$HOME/Library/Logs/rts-docdrift.out.log"
tail -n 120 "$HOME/Library/Logs/rts-docdrift.err.log"
git -C .docdrift/worktrees/docdrift-runner rev-parse --short HEAD
git -C .docdrift/worktrees/docdrift-sweep status --short
```

Treat `.docdrift/last-failure.md` as the fastest answer for "did the daily fail?". A partial
full sweep can still create, merge, and checkpoint a successful prefix PR before exiting non-zero;
in that case the failure marker is expected and should name the later commit that stopped doc
generation. Use the linked PR or the `prUrl`/`sweep.action` fields in
`.docdrift/runs/<run-id>/docdrift-full.json` to distinguish a clean no-op, a merged full sweep, a
merged partial prefix, and a hard failure that produced no PR.

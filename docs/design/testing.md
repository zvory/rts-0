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
`game/replay.rs` translates that wire-compatible log into `SimCommand`s, feeds them into a fresh
`Game` with AI thinking disabled, and compares the resulting event stream and final per-player
snapshots. Replay and live play use the same typed command application path, so a replay proves both
the recorded command artifact and the deterministic simulation ordering. Entity iteration and A*
tie-breaking must remain stable; avoid hash-order-dependent simulation behavior.

**Fast/full AI split.** Plain `cargo test` keeps the self-play harness in the default gate, but only
runs the fast scripted coverage. Long profile-backed and real-AI self-play tests return early unless
`RTS_FULL_AI_TESTS=1` is set; `tests/run-all.sh --full-ai` enables that mode for the local gate.
`RTS_SELFPLAY_FULL=1` remains accepted as an alias for manual self-play runs. Use full AI coverage
when touching AI strategy, profile-backed self-play, replay determinism, or balance behavior that
depends on long matches.

**Profile-backed coverage.** The long profile-backed tests spawn AI-profile players through the
self-play adapter and run matches headlessly under `RTS_FULL_AI_TESTS=1 cargo test`. The profiles
gather steel and oil, construct supply and tech structures, train Riflemen and Tanks, and launch
attack-move waves at public enemy start tiles. The self-play adapter owns harness-only state such as
pending build intents, failed build spots, and staging/attack guards needed to interpret
fog-filtered snapshots without duplicating profile strategy logic. The harness checks per-tick
invariants for invalid resources, supply overflow, malformed entity snapshots, out-of-bounds
positions, and non-finite progress values. It also enforces progress deadlines so a stuck
economy/tech/combat loop fails as a deadlock instead of timing out silently.

Special harness scripts remain where they cover behavior that is not a normal AI strategy profile:
`WorkerRushScript` is an all-in worker-pull scenario, and `MineOnlyScript` is passive mining/fairness
coverage. These scripts are kept isolated from the canonical profile list.

**Artifacts.** On failure, the test writes `target/selfplay-failures/<test>-<pid>-<time>/`
with:
- `replay.json`: start payload, player specs, per-player starting steel/oil (so debug mode
  matches replay with the same economy they were recorded with),
  script decision log, authoritative tick-stamped command log, event log, milestone state,
  and sampled snapshot summaries.
- `summary.log`: short human-readable failure summary and missing milestones.

The artifact is meant to be enough to reproduce or inspect a failing run without manually
playtesting first. By default successful runs do not write artifacts. For manual inspection,
setting `RTS_SELFPLAY_SAVE_REPLAY=1` writes a successful run to
`target/selfplay-artifacts/<test>-<pid>-<time>/`; setting `RTS_SELFPLAY_SAVE_REPLAY=<name>` uses
that explicit safe artifact name instead.

**Profile matchup CLI.** The `ai-matchup` binary is the manual fixed-horizon matchup facility for
profile-vs-profile runs. It composes the same self-play adapter and `Game` seam as the tests, runs
one directed match to elimination or a tick cap, optionally verifies deterministic replay, and can
write a replay artifact:

```bash
cd server
cargo run --bin ai-matchup -- rush tech
cargo run --bin ai-matchup -- saturation tech --seed 7 --ticks 20000 --json
cargo run --bin ai-matchup -- --list-profiles
```

Keep fast invariant-style milestone coverage in `cargo test`; use `RTS_FULL_AI_TESTS=1 cargo test`
for the long regression gate and the CLI for balance exploration, seed sweeps, and strategy result
sampling.

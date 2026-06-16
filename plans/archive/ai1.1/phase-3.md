# Phase 3: 30k release replay comparison

## Status

Done.

## Goal

Run AI 1.1 against AI 1.0 in a release-build self-play matchup capped at 30,000 ticks, save a verified replay artifact, and determine whether AI 1.1 outperforms AI 1.0 before any live-default promotion is considered.

## Scope

- Build/run the release `ai-matchup` binary for `ai_1_1_tank_mg` vs `ai_1_0_tech`.
- Use `--ticks 30000`, a fixed seed, JSON output, and `--save-replay`.
- Keep deterministic replay verification enabled unless it fails for a confirmed unrelated harness issue.
- Save or tee the JSON scorecard to a stable artifact path if the tooling does not already do so.
- Open the local replay viewer with macOS `open` only if a local server is running or the phase starts one; do not use browser automation for this flow.
- Update this phase document with the replay command, artifact path, high-level result, and whether the result is strong enough to justify Phase 4 promotion.

## Expected command

From `server/`:

```bash
cargo run --release --bin ai-matchup -- ai_1_1_tank_mg ai_1_0_tech --ticks 30000 --seed 7 --json --save-replay ai_1_1_vs_ai_1_0_30k
```

If Phase 1 chose a different AI 1.1 id, use that exact id and record the deviation here.

The executed command used the same profile ids with an explicit package because two workspace
packages expose an `ai-matchup` binary:

```bash
cargo run --release -p rts-ai --bin ai-matchup -- ai_1_1_tank_mg ai_1_0_tech --ticks 30000 --seed 7 --json --save-replay ai_1_1_vs_ai_1_0_30k
```

## Result

- Scorecard artifact: `plans/ai1.1/artifacts/phase-3-scorecard.json`.
- Replay artifact: `/private/tmp/rts-worktrees/ai1.1-phase-3/server/crates/ai/target/selfplay-artifacts/ai_1_1_vs_ai_1_0_30k/replay.json`.
- Replay verification: passed (`replayVerified: true`).
- Outcome: AI 1.0 won by elimination at tick 29,350.
- AI 1.1 Scout Cars: zero train commands, `firstScoutCarTick: null`, and
  `firstScoutCarHarassCommandTick: null`.
- AI 1.1 MG/Tank mix: five Machine Gunner train commands and 20 Tank train commands in the replay
  command log; the first Machine Gunner train command was tick 9,180 and the first Tank train
  command was tick 9,570.
- MG perimeter evidence: individual AI 1.1 `attackMove` commands to nearby defensive/perimeter
  coordinates appear from tick 9,234 through tick 9,936 before the first full Tank-era wave at tick
  10,320.
- Promotion decision: this evidence does not support promoting AI 1.1. Phase 4 should leave AI 1.0
  as the live default and document that AI 1.1 underperformed in the required release comparison.

## Verification

- The command exits successfully.
- The JSON result reports `replayVerification` success.
- The replay artifact exists under `server/target/selfplay-artifacts/ai_1_1_vs_ai_1_0_30k/replay.json` or the configured replay directory.
- The scorecard confirms AI 1.1 built zero Scout Cars.
- The scorecard and replay show whether AI 1.1 reached the intended MG/Tank mix.
- The result clearly states whether AI 1.1 defeated AI 1.0, survived to tick cap with a material advantage, or underperformed.

## Manual testing focus

- Inspect the replay around the first Machine Gunner completion and confirm MGs scatter toward the enemy-facing perimeter.
- Inspect the tech transition and confirm Tanks remain the priority once Tank production is available.
- Inspect late-game behavior near 30,000 ticks or match end and confirm whether AI 1.1's defensive posture helped against AI 1.0.

## Handoff

After implementation, mark this phase done and summarize the release command, seed, tick cap, winner or tick-cap outcome, replay artifact path, replay verification status, and key player-facing observations. Explicitly state whether the evidence supports promoting AI 1.1; if not, instruct Phase 4 to leave AI 1.0 as the live default and document the failed comparison.

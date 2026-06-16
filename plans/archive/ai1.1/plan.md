# AI 1.1 plan

## Summary

Phase 1 adds an `ai_1_1_tank_mg` profile as a close fork of AI 1.0 and registers it in developer tooling without changing the live lobby default. It caps ordinary Barracks growth at two, removes Scout Car harassment and Scout Car production, and keeps the AI 1.0 economy, expansion, Methamphetamines-before-Tanks rule, and Tank-required frontal attack posture intact. It also updates `docs/design/ai.md` and focused profile/self-play tests so AI 1.1 can be compared against AI 1.0 from stable tooling.

Phase 2 teaches the shared AI decision layer to reserve a bounded Machine Gunner defensive group and stage those MGs on deterministic enemy-facing perimeter points near the main resource line. It must explicitly preserve the no-Scout-Car AI 1.1 contract while adding MG production, MG reservation, and MG perimeter staging through the existing `AiActionContext` command path. The phase should reuse and generalize the existing support-line helpers so Machine Gunners move with attack-move orders and deploy naturally when they arrive.

Phase 3 runs the requested release-build comparison of AI 1.1 against AI 1.0 to 30,000 ticks and saves a replay artifact for inspection. It should use the release `ai-matchup` binary with replay verification enabled, capture a JSON scorecard, and report the replay artifact path so the result can be inspected in the local replay viewer. The handoff must state whether AI 1.1 outperformed AI 1.0, reached Tank production, produced zero Scout Cars, and formed the MG perimeter before Tank attacks dominated.

Phase 4 is conditional: only promote AI 1.1 as the live lobby default if Phase 3 shows it outperforming AI 1.0 in the replay evidence the user accepts. If the replay is inconclusive or AI 1.1 underperforms, this phase should instead document the result and leave AI 1.0 as the live default. If promotion is approved by the Phase 3 evidence, update live defaults and user-facing AI labels while keeping AI 1.0 available for developer comparison.

## Constraints and considerations

- Keep final `SimCommand` emission centralized in `AiActionContext` and `ai_core::actions`; AI 1.1 should be profile data plus small shared decision-layer policy, not a separate `think()` function.
- AI 1.1 is "basically AI 1.0": preserve the AI 1.0 worker targets, expansion timing, Tank tech path, Methamphetamines-before-Tanks gate, panic-defense behavior, and Tank-required frontal attack unless a phase explicitly documents a narrow change.
- AI 1.1 must never train Scout Cars and must not reserve Scout Cars for harassment. Every implementation phase that touches profile production, tech transitions, harassment, matchup scorecards, or replay validation must preserve and test this.
- The ordinary Barracks curve for AI 1.1 must never ask for more than two Barracks. Defensive panic may keep its existing emergency behavior only if tests document that distinction; otherwise cap that path too.
- Machine Gunner production should be bounded and defensive. The profile should make "some amount of MGs" explicit as a small target count, then prioritize Tank production once Tank production is available.
- Staged Machine Gunners should face the enemy by receiving enemy-facing attack-move stage positions. Do not add private sim access or hidden enemy-position knowledge; use public starts, visible enemies, and fog-respecting observations already available to AI decisions.
- Local defense and panic defense are higher priority than passive perimeter staging. If enemies are visible near the base, available defenders should respond to the threat before returning to their perimeter.
- Developer tooling should keep AI 1.0 available under `ai_1_0_tech`, `ai1`, and `ai_1_0` for comparison.
- Do not change the live lobby default or lobby label until after the 30,000 tick release replay comparison has shown AI 1.1 outperforming AI 1.0 and the phase handoff records that evidence.
- Update `docs/design/ai.md` in the same phase as behavior changes because AI profile availability and live-default semantics are design-visible.
- Use targeted tests during phase development. Rely on the normal commit hook for the broad gate when each implementation phase is ready to merge.
- After each phase, commit the phase work, merge that phase branch to `main`, push `main`, and only then start the next phase.
- When a phase is complete, mark that phase document as done in that phase's implementation commit.
- After each phase, the implementing agent must provide a handoff message for the next agent. The handoff must name the core features that should be manually tested; this is not a comprehensive test matrix.

## Expected comparison command

From `server/`, Phase 3 should run:

```bash
cargo run --release --bin ai-matchup -- ai_1_1_tank_mg ai_1_0_tech --ticks 30000 --seed 7 --json --save-replay ai_1_1_vs_ai_1_0_30k
```

Use the exact profile id from Phase 1 if it changes during implementation, but keep the replay name descriptive and stable.

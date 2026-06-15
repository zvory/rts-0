# Phase 2: MG perimeter staging

## Status

Not started.

## Goal

Make AI 1.1 train and reserve a small Machine Gunner defensive group, scatter those MGs on deterministic perimeter positions facing the enemy, and explicitly preserve the AI 1.1 no-Scout-Car contract while Tank production remains the main win condition.

## Scope

- Add a profile-level policy for defensive support staging if Phase 1 did not already add one.
- Reuse and generalize `server/crates/ai/src/ai_core/decision/defense.rs` support-line helpers instead of adding a separate command path.
- Reserve AI 1.1's defensive MGs before frontal-wave unit selection, similar to how AI 1.0 reserves Scout Cars for harassment.
- Issue individual `AttackMove` stage commands through `AiActionContext` so Machine Gunners naturally face toward likely enemy approach and deploy after arriving.
- Keep AI 1.1 Scout Car production and Scout Car harassment disabled while adding MG production/staging.
- Add manager trace labels/blockers for the defensive perimeter if useful for debugging, keeping trace output compact.
- Update `docs/design/ai.md` with the new AI 1.1 defensive perimeter behavior and the explicit no-Scout-Car rule.

## Behavioral requirements

- The staged group should be bounded, deterministic, and based on visible/public information. A target of three or four Machine Gunners is reasonable unless Phase 1 selected a different documented count.
- MG staging should not require hidden enemy positions. Use the nearest living public enemy base and the AI's main steel/resource-line center as the primary geometry.
- Perimeter assignments should be stable by unit id and spread laterally so the units are not clumped.
- If an assigned MG is already close to its stage point, do not reissue orders every think.
- Local defense has priority: visible base threats should pull eligible defenders into direct defense before passive perimeter staging.
- AI 1.1 Tank frontal-wave readiness must exclude the reserved MG perimeter group and must still require at least one Tank.
- AI 1.1 must still produce no Scout Cars. Add or update a targeted test that fails if AI 1.1's production priorities, harassment policy, or matchup scorecard permits Scout Cars.
- Phase 2 must not change the live lobby default.

## Expected touch points

- `server/crates/ai/src/ai_core/profiles.rs`
- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/decision/defense.rs`
- `server/crates/ai/src/ai_core/decision/frontal.rs` if the reservation needs a cleaner hook
- `server/crates/ai/src/ai_core/decision/trace.rs` if trace output is added
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `docs/design/ai.md`

## Verification

- `cargo test -p rts-ai ai_1_1`
- `cargo test -p rts-ai defensive`
- `cargo run --bin ai-matchup -- ai_1_1_tank_mg ai_1_0_tech --ticks 8000 --seed 7 --json`

## Manual testing focus

- Confirm AI 1.1 trains Machine Gunners after the Training Centre path opens.
- Confirm Machine Gunners receive spread, enemy-facing perimeter stage orders near the main resource line.
- Confirm Tank production still starts once Tank tech and Methamphetamines are ready.
- Confirm the scorecard still shows no Scout Cars for AI 1.1.

## Handoff

After implementation, mark this phase done and summarize the perimeter policy, the bounded MG count, the tests run, and the first tick where the short matchup shows Machine Gunners/Tanks. Explicitly confirm AI 1.1 still builds zero Scout Cars and that the live lobby default is still AI 1.0.

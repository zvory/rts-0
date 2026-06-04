# Phase 0 - Baseline and Extraction Guardrails

Goal: create objective baselines and rules for safe extraction before moving code.

## Scope

- Record current line counts for the hotspot files in the implementation change that starts this
  cleanup.
- Identify which tests currently cover each hotspot:
  - `cargo test` in `server/`
  - relevant Node integration scripts when lobby/client behavior is touched
  - self-play tests when AI or simulation behavior is touched
- Add or update small module comments only where they clarify intended ownership after extraction.
- Decide per phase whether the first implementation should be:
  - mechanical move-only extraction
  - move plus visibility cleanup
  - behavioral cleanup after extraction

## Guardrails

- No gameplay behavior changes in this phase.
- No wire protocol changes.
- No config or balance changes.
- No import churn outside files being prepared for extraction.
- Avoid new abstractions unless the extraction needs a stable internal API.

## Baseline Metrics

Track these before and after each phase:

- Raw line count for each touched file.
- Number of public items introduced by the split.
- Tests moved, added, or deleted.
- Whether `DESIGN.md` changed and why.

## Done

- A short cleanup baseline exists in the phase implementation notes or commit body.
- The first extraction target and ownership boundary are explicit.
- No runtime behavior changed.


# Phase 2 - Baseline and Ratchet Budgets

## Objective

Make the architecture checker useful immediately without requiring a big-bang cleanup. The checker
should capture today's coupling and fail only when a change makes the selected metrics worse.

## Work

- Add a committed baseline file, for example
  `server/crates/archcheck/baselines/sim-architecture.json`.
- Record current values for:
  - lines per tracked file
  - imports from one service module into another
  - functions accepting broad mutable world state
  - direct `PlayerState` usage sites
  - direct `Entity` field write sites outside `entity/`
  - public and `pub(crate)` export counts by module
- Make the checker fail when a metric exceeds its baseline unless the baseline is explicitly
  updated.
- Require every baseline update to include a short reason string in the baseline file.
- Add a mode such as `--bless` only if it is non-default and noisy about what changed.

## Line Count Policy

Line count is not a design law. In this repo it is useful because large files cost more review time,
more agent context, and more merge risk.

The ratchet should behave like this:

- Existing large files pass at their current size.
- New growth beyond a small buffer fails.
- Shrinking a file lowers the future budget.
- A temporary increase is allowed only through an explicit baseline update with a reason.

## Verification

- Run the checker on the current tree and confirm no failures.
- Modify a fixture or test file to simulate a file growing past budget and confirm the checker fails.
- Confirm a shrink updates the suggested ratchet output without requiring a manual policy change.

## Outcome

Agents can keep working in risky files, but they cannot casually make those files broader or more
expensive without leaving an explicit audit trail.

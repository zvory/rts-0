# Phase 4 - Freeze Reviewed Architecture Seams

Status: Incomplete.

## Objective

Prevent quiet expansion of the reviewed server and client composition seams. Capture current
intentional names and edges rather than chasing line counts, and make deliberate expansion require a
small reviewable checker update.

## Work

- Add a no-growth boundary for the public `Game` methods or exports actually consumed outside
  `rts-sim`. Prefer tracking the external seam by stable names/usages rather than relying only on a
  total count that can hide replacement churn.
- Add proportionate no-growth boundaries for the reviewed Input and Renderer prototype-grafted
  methods and important client fan-out hotspots.
- Record explicit reasons for intentional current exceptions or buffers. Keep buffers modest and
  require a checker update with a reason for deliberate expansion.
- Add focused negative fixtures that prove a new external `Game` seam member, prototype graft, or
  guarded fan-out edge fails the appropriate checker.
- Do not baseline reads or publication of command policy through `GameState`; Phase 8 removes that
  path and installs a zero-tolerance regression guard after the migration.
- Update architecture documentation only where these enforced boundaries become a maintained
  contract.

## Non-goals

- Do not narrow the `Game` API or refactor Input, Renderer, Match, or App.
- Do not add a generalized dependency-analysis platform.
- Do not duplicate the source-size policy owned by Phase 3.
- Do not guard every reported metric; only the reviewed seams above are in scope.

## Expected Touch Points

- `server/crates/archcheck/` and its checked-in baseline
- `scripts/check-client-architecture.mjs`
- focused checker fixtures or tests
- `docs/design/architecture.md` or `docs/design/client-ui.md` only as needed

## Verification

- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node scripts/check-client-architecture.mjs`
- Focused negative fixtures for every new guarded seam.
- `git diff --check`

## Manual Test Focus

No gameplay test is expected. Inspect one failure from each checker and confirm it names the new
method or edge and the narrow remedy rather than emitting only a metric delta.

## Handoff

Mark this phase done in its implementation commit. Report the exact server names/usages and client
edges guarded, retained buffers, and negative evidence. Tell the Phase 5 agent which ability and
upgrade exports are guarded so thin compatibility re-exports can be updated deliberately.

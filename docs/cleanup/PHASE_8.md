# Phase 8 - Final Hardening and Documentation Audit

Goal: finish the cleanup by removing temporary seams, checking documentation, and validating that
the new component layout is easier to maintain.

## Scope

- Remove temporary compatibility wrappers that were only needed during extraction.
- Tighten visibility from `pub(crate)` to `pub(super)` or private where possible.
- Check that module names match actual ownership rather than vague helper buckets.
- Update `DESIGN.md` if any final module boundaries or public contracts changed.
- Add brief module-level docs for new module roots that own important behavior.
- Re-run line-count and public-item baselines from Phase 0.

## Quality Checks

- No extracted module should become a generic dumping ground.
- No new cyclic dependency or hidden service coupling should exist.
- Tests should live near the behavior they protect.
- `systems.rs` should still read as the simulation tick orchestrator.
- `Game` should still be the API seam for lobby/main callers.
- Client modules should still be usable as plain ES modules without a build step.

## Tests

- Run `cargo test` in `server/`.
- Run Node integration/regression scripts for any touched lobby, protocol, or client behavior.
- Run client smoke tests after client decomposition.

## Done

- The largest files are reduced by cohesive extraction, not by scattering behavior.
- Documentation matches the resulting architecture.
- The cleanup does not change gameplay except where a separate, explicit follow-up change says so.


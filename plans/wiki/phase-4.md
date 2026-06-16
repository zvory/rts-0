# Phase 4 - Hardening, Docs, And Regression Gates

## Phase Status

- [ ] Not implemented.

## Objective

Finish the wiki by documenting its contract and adding regression gates that catch broken docs,
unsafe rendering, and stale generated tables without relying on manual browsing.

## Work

- Add or update context/design docs for the wiki route, allowed docs roots, generated stats source,
  and verification command.
- Add a focused script or test target if plain `cargo test ... wiki` is not enough for link
  integrity and generated-table completeness.
- Ensure Markdown HTML is escaped or sanitized consistently with the chosen renderer's contract.
- Confirm missing docs, unsupported paths, traversal attempts, and generated stats failures produce
  deterministic test-covered outcomes.
- Keep styling and navigation simple; only fix readability problems that block practical use.

## Expected Touch Points

- Server wiki module
- `docs/context/deployment.md` or a more relevant context capsule
- `docs/design/architecture.md` or `docs/design/balance.md` if the server/docs contract changes
- `plans/wiki/*`
- Optional `scripts/check-wiki.mjs` or Rust-only equivalent if useful

## Implementation Checklist

- [ ] Document the wiki route and generated-stats authority.
- [ ] Add a single regression command for future wiki verification.
- [ ] Cover unsafe path, missing page, escaped content, and broken internal-link cases.
- [ ] Cover completeness of generated unit/building/faction/ability tables.
- [ ] Remove any temporary fixtures or debug routes from earlier phases.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server wiki`
- `node scripts/check-wiki.mjs` if this phase adds that script
- `node scripts/check-faction-catalog-parity.mjs`
- `git diff --check`
- Commit hook for the final merge-ready commit

## Manual Test Focus

Manual testing should be limited to one fallback readability pass if automated route, link, and
table checks all pass: `/wiki`, one design doc, and `/wiki/stats`.

## Handoff Expectations

Provide the final wiki URL map, the regression command future agents should run after docs or
balance changes, and any intentionally deferred usability work.

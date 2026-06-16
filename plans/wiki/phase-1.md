# Phase 1 - Wiki Route Foundation

Status: Done.

## Objective

Create the minimal server wiki serving seam with safe Markdown rendering and automated route
coverage.

## Work

- Add a server-local wiki module or helper that maps `/wiki` requests to allowlisted Markdown files
  under `docs/`.
- Render Markdown to HTML with a normal Markdown crate instead of ad hoc string parsing.
- Serve `/wiki` and at least one doc-backed page, likely `docs/context/README.md`.
- Add basic page chrome only where needed for readability and navigation.
- Reject missing files and path traversal attempts with deterministic statuses.

## Expected Touch Points

- `server/src/main.rs`
- New `server/src/wiki.rs` or equivalent server-local module
- `server/Cargo.toml` if a Markdown rendering crate is added
- `docs/context/README.md` only if a route pointer needs clarification
- Focused Rust tests for wiki path and rendering behavior

## Implementation Checklist

- [ ] Add route handlers for `/wiki` and doc-backed wiki paths.
- [ ] Add safe path normalization that cannot escape the allowlisted docs roots.
- [ ] Render Markdown to HTML with deterministic wrapping.
- [ ] Set appropriate `Content-Type` and cache behavior for wiki pages.
- [ ] Add tests for index routing, valid doc rendering, missing docs, and traversal rejection.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server wiki`
- `git diff --check`

## Manual Test Focus

Manual testing should be unnecessary if route tests cover the implemented paths. If a visual check
is still needed, open `/wiki` once and confirm the rendered page is readable enough to navigate.

## Handoff Expectations

List the exact wiki routes added, the safety cases covered by tests, and any route behavior that
still depends on manual browser inspection.

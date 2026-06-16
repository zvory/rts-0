# Phase 2 - Docs Navigation And Link Rewriting

Status: Done.

## Objective

Make the existing docs browsable through `/wiki` with preserved relative links and automated link
integrity checks.

## Work

- Build a simple wiki index from the existing `docs/context` capsule list and `docs/design`
  Markdown files.
- Rewrite relative Markdown links to wiki URLs while leaving external links unchanged.
- Support same-page anchors and cross-doc anchors well enough for current docs.
- Keep route names stable and predictable, such as `/wiki/docs/context/balance.md`.
- Add automated checks that crawl allowlisted docs and verify rewritten internal wiki links resolve.

## Expected Touch Points

- Server wiki module from Phase 1
- `docs/context/README.md` only if index semantics need a small clarification
- New or expanded focused Rust tests for link rewriting and link integrity

## Implementation Checklist

- [x] Render a navigation index for context capsules and design docs.
- [x] Rewrite relative `.md` links into `/wiki/...` URLs.
- [x] Preserve hash anchors on rewritten links.
- [x] Keep external URLs and non-doc assets from being incorrectly rewritten.
- [x] Add a link-integrity test over the allowlisted docs set.
- [x] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server wiki`
- `git diff --check`

## Manual Test Focus

Manual testing should be a fallback only. If needed, open `/wiki`, click the balance capsule, then
click through to the balance design doc and one same-page anchor.

## Handoff Expectations

Name the docs roots exposed through the wiki, summarize the internal-link coverage, and list any
known links intentionally left unresolved or external.

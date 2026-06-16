# Server Wiki Plan

## Purpose

Add a lightweight server-hosted wiki for existing project docs and generated gameplay-reference
tables. The wiki should favor direct usefulness over polish: readable Markdown, preserved links,
simple navigation, and machine-generated tables from authoritative Rust rules data. The plan should
avoid manual browser-only validation by making route behavior, link rewriting, generated data, and
HTML safety mechanically testable.

## Overall Constraints

- Keep the game client no-build-step flow unchanged; the wiki is a server route, not a client SPA.
- Treat `docs/design/*.md` as the source of truth for prose and `server/crates/rules` as the source
  of truth for stats, catalogs, abilities, and balance tables.
- Do not scrape `client/src/config.js` or rendered Markdown for authoritative stats.
- Preserve relative Markdown links where practical by rewriting them into `/wiki/...` URLs.
- Block path traversal and only serve allowlisted repository docs.
- Keep formatting plain and robust. Ugly tables are acceptable; broken links, stale generated
  numbers, and untested route behavior are not.
- Prefer focused automated coverage for each phase over manual smoke testing.
- After each phase, provide a handoff naming exact verification results, remaining generated-data
  gaps, and the small manual fallback flow if automated coverage could not prove a behavior.
- Implement, commit, merge to `main`, and push each phase before starting the next phase.

## Phase Summaries

### [Phase 1 - Wiki Route Foundation](phase-1.md)

Add a server-local wiki module with safe path resolution, Markdown-to-HTML rendering, and basic
`/wiki` routes. The first page can be plain and sparse, but route status codes, content types,
path traversal rejection, and deterministic HTML output should have focused Rust tests. This phase
establishes the verified serving seam without trying to generate gameplay tables yet.

### [Phase 2 - Docs Navigation And Link Rewriting](phase-2.md)

Render the existing `docs/context` and `docs/design` structure through the wiki with a simple
navigation index. Rewrite relative Markdown links, headings, and same-doc anchors so the existing
docs remain browsable under `/wiki`. Verification should enumerate linked docs and assert that
known capsule/design links resolve without needing a manual click-through pass.

### [Phase 3 - Rust-Authoritative Stats Tables](phase-3.md)

Generate wiki tables for units, buildings, resource nodes, faction catalogs, upgrades, and
abilities from `rts_rules` definitions. Add tests or snapshots that compare rendered table rows to
the same Rust data used by simulation and catalog parity checks. This phase should make the wiki
useful as a player-facing reference while keeping generated values impossible to drift from Rust
constants.

### [Phase 4 - Hardening, Docs, And Regression Gates](phase-4.md)

Tighten the wiki surface after the core pages exist: coverage for missing docs, unsafe paths,
escaped Markdown/HTML, generated table completeness, and link integrity. Document the server wiki
contract in the relevant context/design docs and add a small regression command so future balance
or docs changes can verify the wiki without manual browsing. This phase is cleanup and guardrails,
not a visual redesign.

## Non-Goals

- Do not build a CMS, editor, search engine, authentication layer, or static-site pipeline.
- Do not redesign the main game client or add wiki-specific client framework code.
- Do not move docs out of `docs/` or make generated tables the prose source of truth.
- Do not require manual browser testing as the primary acceptance criterion.

## Handoff Rules

Each phase file has an implementation checklist. Handoffs must include exact verification commands,
whether the automated checks fully covered route/link/table behavior, and the smallest manual
fallback flow if any behavior still needs eyes-on confirmation.

# Phase 2 - Docs Health Script, CI, and Hooks

Status: done.

## Goal

Add a deterministic docs-health check and make it part of both CI and local commit hooks. The check
should keep context capsules small and routing metadata valid without introducing a semantic
sweeper, waiver process, or heavyweight local gate.

## Scope

- Add `scripts/check-docs-health.mjs`.
- Validate `docs/doc-map.json`:
  - file parses;
  - `version` is present;
  - every route has at least one `source` entry and one `docs` entry;
  - every referenced doc path exists;
  - duplicate route entries are diagnosed clearly.
- Enforce a hard 5 KiB size cap for `docs/context/*.md` capsules.
- Before enabling the cap, trim current oversized capsules. At minimum, inspect
  `docs/context/testing.md` and `docs/context/client-ui.md`; move long scenario inventories, CI
  implementation history, or other details into existing design/reference docs rather than deleting
  useful information.
- Check local Markdown links in `docs/**/*.md` and `plans/**/*.md` cheaply. Local links to missing
  `.md` files should fail; external URLs should be ignored.
- Add the script to CI in a cheap early lane, preferably near the changed-file classification where
  Node is already used.
- Add the script to local hooks by updating `hooks/_gate-main.sh` after the staged whitespace check.
- Update `scripts/install-hooks.sh`, `docs/context/testing.md`, or `tests/README.md` only if user
  guidance needs to mention the new cheap hook coverage.

## Out of Scope

- Do not add an LLM or semantic sweeper.
- Do not require doc changes for mapped source changes.
- Do not add a waiver format.
- Do not add new package dependencies unless the implementation proves the dependency is simpler
  than a small standard-library parser/checker.
- Do not run Rust, browser, live server, or broad test bundles from commit hooks.

## Implementation Notes

- Prefer whole-repo deterministic checks over staged-only complexity unless the hook becomes too
  slow in practice.
- Keep error messages actionable, for example:
  - `docs/context/testing.md is 12140 bytes; capsules must be <= 5120 bytes`;
  - `docs/doc-map.json route 4 references missing doc docs/design/foo.md`;
  - `plans/docs/plan.md links to missing phase-3.md`.
- If a context capsule genuinely needs to exceed 5 KiB, do not add an allowlist in this phase.
  Split or move the long material instead.

## Expected Touch Points

- `scripts/check-docs-health.mjs`
- `.github/workflows/main-tests.yml`
- `hooks/_gate-main.sh`
- `docs/doc-map.json` if Phase 2 finds schema gaps while implementing validation
- `docs/context/testing.md`
- `docs/context/client-ui.md`
- possibly `docs/design/testing.md`, `docs/design/client-ui.md`, or a small reference doc if content
  is moved out of capsules
- `tests/README.md` and `docs/context/testing.md` if hook/CI docs need a short update

## Verification

Run focused checks:

```bash
node scripts/check-docs-health.mjs
node tests/select-suites.mjs --verify
git diff --check
```

If Markdown links, wiki-served docs, or generated stats references change, also run:

```bash
node scripts/check-wiki.mjs
```

Do not run broad local bundles just because the hook changed. The PR `./tests/run-all.sh` check
remains the full merge gate.

## Manual Testing Focus

Install hooks in a temporary or current worktree and make a throwaway staged whitespace-safe change.
Confirm the pre-commit hook runs `check-docs-health.mjs` and fails with a clear message if a context
capsule is temporarily made larger than 5 KiB or if `docs/doc-map.json` is temporarily invalid.
Revert the throwaway changes before committing the real phase work.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff should summarize the final health
rules, which oversized capsules were trimmed, the exact CI/hook entry points, focused verification
results, and any content moved from capsules into design or reference docs.

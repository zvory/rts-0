# Phase 1 - Make Safeguards Truthful

Status: Incomplete.

## Objective

Restore confidence that the existing architecture, CI-selection, and documentation checks cover
the code they claim to cover. Keep this a focused guardrail refresh: capture today's intentional
shape, prevent quiet growth or bypasses, and avoid runtime refactors or cleanup done only to satisfy
metrics.

## Work

- Treat changes anywhere in the client rules/config mirror, not only the `config.js` facade, as
  cross-language contract changes. Ensure suite selection runs the Rust/client faction and balance
  parity checks, and add selector cases that would fail if a new internal mirror path bypasses them.
- Repair the scheduled doc-drift flow so a stale or diverged runner branch/worktree can recover onto
  current `origin/main` without losing generated work or remaining permanently wedged. Refresh the
  doc map for the current split config, protocol, replay, match-history, deployment, and other
  presently unmapped source families; make docs health reject source routes that match no tracked
  files. Cover the recovery and representative routing behavior with focused tests.
- Lower or remove stale simulation-architecture and source-size exceptions. Make materially stale
  baselines actionable rather than advisory while preserving modest growth buffers and explicit,
  reviewable exceptions for genuine legacy hotspots.
- Include CSS in the source-size inventory and freeze the existing large stylesheet at its current
  size; do not split or restyle it in this phase.
- Extend the existing architecture checks with proportionate no-growth boundaries for the public
  `Game` surface exposed outside `rts-sim`, Input/Renderer prototype-grafted methods, important
  client fan-out hotspots, and reads or publication of command policy through `GameState`. Baseline
  current intentional uses and require an explicit checker update for deliberate expansion.
- Keep checker ownership and policy in the existing scripts and architecture-check crate. Update
  the relevant testing or architecture documentation only where the enforced contract changes.

## Non-goals

- Do not narrow the `Game` API, extract command interaction, split client or server hotspots, or
  change gameplay/runtime behavior; later phases own those changes.
- Do not introduce a generalized dependency-analysis platform, CSS framework, code generator, or
  new CI lane.
- Do not chase every reported metric. Guard only the reviewed high-value boundaries above.

## Expected Touch Points

- `tests/select-suites.mjs` and its selector verification cases
- `scripts/check-faction-catalog-parity.mjs`
- `scripts/docdrift-*.mjs`, `scripts/docdrift-daily.sh`, `tests/docdrift_sweeper.mjs`
- `docs/doc-map.json`, `scripts/check-docs-health.mjs`, and focused docs-health tests
- `scripts/check-source-file-sizes.mjs` and `scripts/source-file-size-baseline.json`
- `scripts/check-client-architecture.mjs`
- `server/crates/archcheck/` and its checked-in baseline
- `docs/design/architecture.md`, `docs/design/client-ui.md`, or testing docs only as needed to
  describe newly enforced rules

## Verification

- `node tests/select-suites.mjs --verify`
- `node scripts/check-faction-catalog-parity.mjs`
- `node tests/docdrift_sweeper.mjs`
- `node scripts/check-docs-health.mjs`
- Run the doc-drift flow in its safe preview/dry-run mode against a realistic stale checkpoint and
  confirm it reaches current `origin/main` without opening a PR.
- `node scripts/check-source-file-sizes.mjs`
- `node scripts/check-client-architecture.mjs`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- Add focused negative fixtures for each new boundary so the checks are proven to fail on bypass or
  growth, then run `git diff --check`.

## Manual Test Focus

No gameplay test is expected. Inspect the selected-suite output for an internal client config
change, the doc-drift preview's base/head and recovery result, and checker output once to ensure a
developer receives a concise path and remedy rather than a wall of advisory notes.

## Handoff

Report the newly enforced boundaries, refreshed exceptions, doc-drift recovery evidence, and exact
verification results. Call out any intentionally retained baseline or route gap with its reason.
Tell the Phase 2 agent which `Game`, upgrade/ability, and command-limit surfaces are now guarded so
its source-of-truth cleanup can update them deliberately.

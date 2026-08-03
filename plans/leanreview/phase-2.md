# Phase 2 - Conservative Review Tiers

## Phase Status

- [ ] Ready after Phase 1 is merged.

## Objective

Stop every non-docs PR from inheriting the user's global Sol High configuration. Choose an explicit
model and reasoning effort from a deliberately small, conservative changed-path classifier while
keeping Sol High on the repository surfaces where a missed authority, secrecy, or workflow problem
is expensive.

## Tier Contract

The implementation may refine exact path spelling against current `origin/main`, but must preserve
these semantics:

| Tier | Explicit child configuration | Intended scope |
| --- | --- | --- |
| Low | `gpt-5.6-terra`, `medium` | Explicitly allowlisted presentation assets, styles, and other narrow mechanical surfaces with no executable or contract code. |
| Normal | `gpt-5.6-terra`, `high` | Default for all non-Markdown changes not proven low-risk or classified high-risk. |
| High | `gpt-5.6-sol`, `high` | Authority/security/fog, wire protocol, simulation seam/tick path, lobby/connection boundaries, CI/PR-review machinery, and the classifier itself. |

Markdown-only changes retain the existing skip and do not enter this classifier. Mixed or unknown
paths default to Normal; any High path raises the whole diff to High; Low applies only when every
non-Markdown path is explicitly low-risk.

## Work

- Implement one short pure classifier over the already collected changed paths. Prefer a small
  function beside existing workflow code over a new configuration format.
- Reuse `tests/select-suites.mjs --ci-policy` only for its existing broad classification; do not add
  review-risk semantics to the CI suite selector unless doing so clearly removes duplication.
- Keep the High rules tied to current repository invariants:
  - protocol/compact snapshot mirrors and protocol design tooling;
  - fog projection, per-player filtering, hardening, and untrusted-input boundaries;
  - `Game` API/tick orchestration and room/lobby/connection authority;
  - agent PR, adversarial pass, branch-protection/CI workflow, and review-risk classifier code.
- Keep Low deliberately narrow. Asset manifests or generators that execute code, deploy packaging,
  generated protocol/config artifacts, and mixed asset/code changes must not qualify merely because
  they contain assets.
- Extend `buildCodexArgs` to pass both `--model` and an explicit
  `model_reasoning_effort="..."` config override. The child must not silently inherit either field
  from global user configuration.
- Add the tier, model, effort, and one short reason to the normalized JSON/Markdown report and PR
  body. Keep the report useful for audit without adding token counts or session persistence.
- Print the selected tier before launching Codex so a mistaken route is visible immediately.
- Provide a narrowly scoped environment or CLI override only if existing tests need deterministic
  injection; do not make ordinary callers manually select a tier.
- Update `docs/design/testing.md` and `docs/pr-first-workflow.md` with the tier contract and
  conservative fallback.

## Expected Touch Points

- `scripts/adversarial-quality-pass.mjs`
- `scripts/adversarial-quality-pass.schema.json`
- `scripts/agent-pr.sh` only if it must forward classifier input
- `tests/adversarial_quality_pass.mjs`
- `docs/design/testing.md`
- `docs/pr-first-workflow.md`

## Required Tests

- An all-low allowlist fixture selects Terra Medium.
- An ordinary client or server implementation fixture selects Terra High.
- Each high-risk contract family selects Sol High.
- Mixed Low plus Normal selects Normal; mixed Low/Normal plus High selects High.
- An unknown path selects Normal.
- Markdown-only handling still skips the child entirely.
- Generated executable artifacts, deploy files, and workflow/classifier changes cannot be
  accidentally classified Low.
- Generated Codex arguments contain explicit model and effort even when the parent environment or
  user config says Sol High.
- JSON normalization, Markdown rendering, status posting, and existing reports remain compatible
  with the added audit fields.

## Verification

- `node tests/adversarial_quality_pass.mjs`
- `bash -n scripts/agent-pr.sh tests/run-all.sh`
- `node --check scripts/adversarial-quality-pass.mjs`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Use dry-run fixtures or disposable branches representing asset-only, ordinary client code, and a
protocol/fog/workflow change. Confirm the launch line and report show Terra Medium, Terra High, and
Sol High respectively, and confirm changing the user's global model setting does not alter the
selected child configuration.

## Handoff Expectations

Report the exact Low and High path families, the conservative fallback behavior, the child CLI
arguments, and where tier metadata appears in the PR body. Tell the Phase 3 agent how to add focused
verification context without disturbing classifier inputs or the bounded changed-path manifest.

## Deferred

- Per-task model benchmarking or automatic promotion based on review outcome.
- Luna routing, `xhigh`, `max`, or pro-mode review.
- Token-budget enforcement and usage telemetry.
- Incremental post-CI delta review.

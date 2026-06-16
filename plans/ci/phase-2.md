# Phase 2 - Branch protection and auto-merge settings

## Status

Pending.

## Goal

Make GitHub enforce the new repository contract: `main` updates happen through PRs, required PR CI
must pass, and auto-merge can complete green PRs without an agent spending tokens waiting.

## Scope

- Enable repository auto-merge.
- Enable delete branch on merge unless a blocker is discovered.
- Add branch protection or a repository ruleset for `main`.
- Require PRs before merge.
- Require the stable checks chosen in Phase 1.
- Require branches to be up to date before merge if available for the selected protection model.
- Block normal direct pushes to `main`; decide whether admins may bypass only for emergency
  repair.
- Document the exact GitHub settings and commands used so the setup can be recreated.

## Expected touch points

- GitHub repository settings or rulesets for `zvory/rts-0`
- `AGENTS.md`
- `CLAUDE.md`
- `README.md`
- `plans/ci/plan.md` if the chosen protection model changes the constraints

## Verification

- `gh api` or `gh repo view` confirms `autoMergeAllowed` is true.
- `gh api` confirms delete-branch-on-merge is enabled.
- GitHub branch protection or ruleset inspection confirms PR requirement, required status checks,
  and direct-push blocking for `main`.
- A test direct push to `main` is not attempted destructively; instead inspect settings or use a
  safe branch-protection API read to confirm the rule.
- A test PR can be set to auto-merge with `gh pr merge --auto`.

## Manual testing focus

Inspect the repository settings page after the API/configuration change. Confirm the UI shows
auto-merge availability on a PR and that the required checks match Phase 1's names.

## Handoff expectations

Include the exact settings evidence and any admin-bypass decision. If branch protection cannot
express a needed rule, document the gap and the compensating script or workflow check planned for
Phase 4.

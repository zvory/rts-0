# Phase 6 - Agent-PR Rust Migration

Status: Not started.

## Goal

Move `agent-pr` behavior into the Rust dev tool while keeping the existing PR lifecycle contract
unchanged. This phase should make PR creation, metadata generation, labels, and auto-merge arming
testable and cross-platform without replacing the GitHub CLI transport yet.

## Scope

- Implement `./tool agent-pr` and `tool.ps1 agent-pr`.
- Preserve all existing options from `scripts/agent-pr.sh`.
- Preserve the `rts-agent-pr:v1` metadata block fields and accepted values.
- Preserve owner detection through `gh api user` with git config fallback.
- Preserve branch validation requiring `zvorygin/*` heads unless a later explicit policy changes
  that rule.
- Preserve update-vs-create behavior for existing open PRs.
- Preserve label creation, label attachment, draft creation, `--no-auto-merge`, and `--dry-run`.
- Keep `scripts/agent-pr.sh` as a compatibility shim that calls the Rust implementation.
- Add fake `gh` runner tests for command construction and JSON parsing.

## Expected Touch Points

- `server/crates/devtool/src/agent_pr.rs`
- `server/crates/devtool/src/github.rs`
- `server/crates/devtool/src/git.rs`
- `scripts/agent-pr.sh`
- `scripts/check-pr-ownership.sh` only if the metadata contract must be clarified
- `docs/pr-first-workflow.md`
- `README.md`
- `CLAUDE.md` only if the preferred command path changes for agents

## Implementation Notes

Keep GitHub CLI as the initial transport. The reliability win comes from typed option parsing,
structured JSON parsing, test fixtures, and predictable command construction, not from taking on
GitHub REST authentication in the same phase. If a raw API client becomes desirable later, it should
be a follow-up after the CLI-backed behavior is stable.

The dry-run output should be stable enough for tests and human audit. Avoid printing secrets or
tokens; `agent-pr` should continue to rely on `gh` authentication rather than reading tokens itself.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-devtool agent_pr`
- `./tool agent-pr --dry-run --verification "devtool dry run"`
- `scripts/agent-pr.sh --dry-run --verification "devtool compatibility dry run"`
- `scripts/check-pr-ownership.sh` through a fixture or PR workflow canary when available
- `node scripts/check-docs-health.mjs`

## Manual Testing Focus

Run a dry-run from a `zvorygin/*` branch and confirm the metadata body, labels, create/edit command,
and auto-merge command match current expectations. If a real canary PR is opened, confirm the PR
ownership workflow accepts the body.

## Handoff Expectations

State whether `scripts/agent-pr.sh` is now only a shim and whether any option remains shell-backed.
Record any output changes that could affect `docdrift-sweep`, `phase-runner-agents.mjs`, or human
parsing of the helper output.

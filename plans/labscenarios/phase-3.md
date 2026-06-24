# Phase 3 - Server-Side PR Submission Service

## Phase Status

- [ ] Pending.

## Objective

Add a disabled-by-default server-side service that can turn one validated lab scenario submission
into a draft GitHub pull request.

## Work

- Choose the credential model. Prefer a GitHub App installation token or narrowly scoped server-side
  token loaded from environment/deployment secrets; never expose it to the browser.
- Add capability detection so deployments without credentials report scenario PR submission as
  unavailable while local export/import still works.
- Add a submission boundary that exports the current authoritative lab scenario, applies validated
  metadata, formats JSON, and writes only allowed files:
  `server/assets/lab-scenarios/<slug>.json` plus the catalog manifest/index from Phase 1.
- Create a branch name under a safe prefix such as `scenario/<slug>` or
  `zvorygin/lab-scenario-<slug>` after checking for collisions.
- Commit only the scenario files and open a draft PR with a deterministic title/body that includes
  author-provided notes, scenario metadata, validation summary, and manual review checklist.
- Do not auto-merge scenario PRs from the product. They need normal human review and CI.
- Add rate limiting or per-room/per-connection submission bounds so a bad client cannot spam PRs.
- Return structured success/failure data: PR URL, branch name, duplicate slug, credentials missing,
  GitHub API error, validation failure, or rate limit.
- Keep network/GitHub work out of the room tick path. If the request originates from a lab room,
  hand off to a bounded async job and report completion reliably.

## Expected Touch Points

- `server/src/main.rs` or a new server module for scenario submission routes/services
- `server/src/lobby/room_task/lab.rs` if submission starts as a lab request
- `server/src/lobby/session_policy.rs` if scenario submission becomes a room capability
- `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js` if using
  a WebSocket lab op/result shape
- GitHub submission helper module or script, kept server-side
- Deployment docs or `.env.example` if adding environment variables
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/context/deployment.md`

## Verification

- Unit tests for path allowlist, slug collisions, disabled credentials, branch naming, PR body
  generation, and validation failures.
- Mocked or dry-run GitHub service tests that prove no arbitrary paths or browser-supplied branch
  names are accepted.
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-protocol lab` if protocol changes.
- `node tests/protocol_parity.mjs` if protocol changes.
- `git diff --check`

Do not require real GitHub credentials for the normal test suite. If a live canary is added, keep it
manual or env-gated.

## Manual Test Focus

Run with submission credentials unset and confirm the UI/service reports unavailable. In a local or
staging environment with test credentials, submit one small scenario and confirm the draft PR changes
only the expected scenario files.

## Handoff Expectations

Name the credential environment variables, the exact allowlisted paths, the PR branch/title/body
format, and any operational risk Phase 4 must surface in the browser.

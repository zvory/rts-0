# Phase 6 - Reproduction, Capture, and Regression

## Phase Status

- [x] Done.

## Objective

Create the repeatable capture loop and regression checks that prove the diagnostics are useful
before any fixing plan uses them. This phase should package local and beta evidence into one
incident directory with raw inputs, parser outputs, an agent digest, and a short neutral analysis
template.

## Work

- Add or update scripts that collect bounded beta logs, run the parser, fetch or store replay/DB
  metadata when available, and write a complete incident directory.
- Add parser/package regression checks so future diagnostic changes cannot quietly degrade
  readability of the Soupman/Alex fixture or older incidents.
- Add an incident template that includes:
  - `README.md`
  - raw run-id logs
  - parser markdown/JSON/TSV
  - agent digest markdown/JSON
  - key metrics JSON
  - replay artifact or reason unavailable
  - DB summary or reason unavailable
  - player report notes with absolute timestamps
  - neutral `analysis.md`
- Add local harness coverage that drives a large-game or replay-backed diagnostic scenario and
  confirms the parser sees command, snapshot, pathing, and client-context sections.
- Add a beta evidence gate checklist for future incidents: deployed build, run id, UTC window,
  focused log query, parser command, source coverage, and unknowns.
- Do not require the user to manually spam commands for routine local validation.
- Do not prescribe fixes in the generated analysis template.

## Expected Touch Points

- `scripts/parse-net-report-logs.mjs`
- new or existing incident capture scripts under `scripts/`
- `scripts/client-perf-harness.mjs` or related harness code if needed
- `docs/perf-tracing.md`
- `docs/network-incident-examples/README.md` if an index is added
- `docs/network-incident-examples/<new-template-or-fixture>/`
- focused script/harness tests

## Agent-Readable Output Requirements

- The evidence package should be usable by a future agent with no private context.
- The generated README should state the match/run identity, exact UTC window, build, participants,
  source files, and one-paragraph neutral diagnosis.
- The generated analysis template should force supported, contradicted, unknown, and next diagnostic
  gap sections.
- Scripts should avoid printing secrets and should bound live log tailing.
- Harness output should compare diagnostic coverage, not claim a fix improved gameplay.

## Implementation Checklist

- [x] Define the incident package directory layout and template files.
- [x] Add capture/parsing script support for run-id evidence packages.
- [x] Add local harness or fixture validation for the new digest sections.
- [x] Add docs for beta evidence collection and local reproduction.
- [x] Add tests or dry-run checks for package generation where practical.
- [x] Use the Soupman/Alex incident as a regression fixture for package completeness.
- [x] Mark this phase as done in this file in the implementation commit.

## Verification

- capture script dry run against preserved Soupman/Alex logs
- `node scripts/parse-net-report-logs.mjs --out-dir <tmp-dir> <fixture-log.jsonl>`
- focused script/harness tests
- `node tests/net_report_log_parser.mjs`
- `node scripts/client-perf-harness.mjs --list` if harness code changes
- `git diff --check`

## Manual Test Focus

Run the package-generation flow on the preserved Soupman/Alex artifact and inspect the resulting
README, digest, and analysis template. If beta credentials are available, run a bounded recent-log
query and confirm the script can build an evidence package for a small fresh run without leaking
tokens. Confirm the package makes missing evidence explicit.

## Handoff Expectations

Provide the package command, output directory, and fixture used for validation. State whether the
capture flow can run entirely locally, requires beta/Fly access, or requires user-provided player
notes. Identify which later fixing plan should consume the resulting diagnostics.

# Phase 1 - Incident Package and Agent Digest

## Phase Status

- [ ] Not started.

## Objective

Upgrade the existing parser flow into an agent-readable incident package and digest using the
diagnostics that already exist. This phase should not add new telemetry fields; it should make
preserved artifacts more useful by summarizing source coverage, timelines, classifications,
correlations, bad windows, provenance, and unknowns in one repeatable format.

## Work

- Add an incident package output to `scripts/parse-net-report-logs.mjs` or a small companion module
  called by that parser. Build around the existing parser rather than creating a disconnected second
  parser.
- Standard package output should include `README.md`, `evidence-index.json`, `key-metrics.json`,
  parser markdown/JSON/TSV, filtered client rows, filtered server tick rows, provenance, missing-data
  warnings, and a concise "what this does and does not prove" section.
- Emit a source manifest that lists input files, row counts, match ids, run ids, build ids, room
  names, participants, UTC windows, and whether each evidence class is present.
- Emit a coverage matrix for client reports, tick rows, snapshot perf rows, writer rows, lifecycle
  rows, replay metadata, DB summary metadata when provided, and missing fields.
- Emit a timeline section with one-minute or configurable bands containing max command response,
  max snapshot gap, max RTT, max server queue, max payload p95 bucket, max frame/render work, and
  slow tick count.
- Emit top-N bad windows by issue class rather than forcing agents to sort TSV rows manually.
- Emit confidence-tagged classifications: indicated, contradicted, weak, unavailable, or unknown.
- Emit a section for "what this artifact cannot prove" based on missing diagnostics and threshold
  gated rows.
- Keep beta incident evidence, local replay/perf harness evidence, and synthetic stress evidence
  separated and labeled.
- Keep old markdown, JSON, and TSV outputs backwards-compatible; the digest can be an additional
  section/file.
- Use the `2026-06-30-beta-soupman-alex-lag` artifacts and at least one older incident as fixtures
  for parser behavior.

## Expected Touch Points

- `scripts/parse-net-report-logs.mjs`
- `tests/net_report_log_parser.mjs`
- `docs/perf-tracing.md`
- `docs/network-incident-examples/2026-06-30-beta-soupman-alex-lag/README.md` only if regenerating
  or documenting digest usage is useful

## Agent-Readable Output Requirements

- The first screen of markdown output should say the primary supported diagnosis and the biggest
  unknowns before detailed tables.
- Every classification must list the field evidence that triggered it and the field evidence that
  argues against it.
- Timeline bands must preserve absolute UTC timestamps.
- JSON output must be stable enough for future scripts to compare before/after diagnostic coverage.
- Missing snapshot/writer rows must be described as "not logged or unavailable," not as zero cost.
- Acceptance should prove the Soupman/Alex artifact surfaces the sustained middle window around
  `00:21Z` through `00:28Z` and the later pathing spike near `00:40Z`.

## Implementation Checklist

- [ ] Design the package layout, digest JSON shape, and markdown sections.
- [ ] Add `evidence-index.json` and `key-metrics.json` outputs.
- [ ] Add source manifest and coverage matrix output.
- [ ] Add timeline band aggregation.
- [ ] Add top-window extraction for command, network, snapshot, server tick, payload, frame/render,
      prediction, and outbound pressure groups.
- [ ] Add explicit unknowns and threshold-gating caveats.
- [ ] Add fixtures/tests for existing incident packages.
- [ ] Update docs with the digest interpretation flow.
- [ ] Mark this phase as done in this file in the implementation commit.

## Verification

- `node tests/net_report_log_parser.mjs`
- `node scripts/parse-net-report-logs.mjs --format markdown docs/network-incident-examples/2026-06-30-beta-soupman-alex-lag/match-103-runid-logs.jsonl`
- `node scripts/parse-net-report-logs.mjs --format json docs/network-incident-examples/2026-06-30-beta-soupman-alex-lag/match-103-runid-logs.jsonl`
- `node scripts/parse-net-report-logs.mjs --out-dir /tmp/lagdigest docs/network-incident-examples/2026-06-30-beta-soupman-alex-lag/match-103-runid-logs.jsonl`
- `git diff --check`

## Manual Test Focus

Open the generated markdown for the Soupman/Alex incident and confirm a future agent can see the
likely diagnosis, time windows, evidence coverage, and unknowns without sorting raw TSVs. Compare it
against an older incident with fewer fields and confirm missing diagnostics are called out as
unavailable rather than silently absent.

## Handoff Expectations

List every new digest section, its JSON path, and the fixture incidents used for coverage. Tell the
next phase which command-lifecycle gaps remain unresolved after the digest is generated from current
fields. Include the manual reading notes for the Soupman/Alex incident.

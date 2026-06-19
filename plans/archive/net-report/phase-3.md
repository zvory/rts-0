# Phase 3 - Incident Parser And Playbook

## Phase Status

- [x] Done.

## Objective

Make post-playtest lag investigation repeatable by adding a small parser and playbook for preserved Fly
logs. The output should summarize likely lag sources without requiring ad hoc manual extraction from
ANSI-decorated log text.

## Work

- Add a script that reads Fly JSONL log windows and extracts structured rows for:
  - `client_net_report`
  - `match_started`
  - `match_ended`
  - `performance tick summary`
  - `performance snapshot timing`
  - `performance writer timing`
  - malformed/missing rows as warnings, not crashes
- Emit machine-readable output such as TSV or JSON plus a compact markdown summary. The summary should
  include per-player maxima and p95/bucket approximations for RTT, snapshot gap/jitter, payload size,
  parse/decode/apply cost, frame work, command timing, server tick/lag, head-of-line/coalescing, and
  writer send/serialization when present.
- Add classification guidance that distinguishes:
  - server tick/scheduler pressure;
  - server snapshot projection/compact/serialization cost;
  - WebSocket writer/send pressure;
  - client network RTT/jitter/snapshot delivery gaps;
  - browser payload parsing/decode/apply/frame phase cost;
  - command upload, server receipt, sim ack, downstream delivery, and render delay.
- Keep classifications evidence-bounded. The parser should say when a transport/WebTransport theory is
  unsupported because packet loss, retransmit, or per-packet browser data is not available.
- Document the workflow in `docs/perf-tracing.md` and link the Matt/Alex incident as the example
  input/output style.
- Add sample fixture coverage using the preserved Matt/Alex log files or a minimized checked-in
  fixture derived from them. Avoid copying large generated parser outputs into the repo unless they are
  intentionally small and stable.

## Expected Touch Points

- new script such as `scripts/parse-net-report-logs.mjs`
- `docs/perf-tracing.md`
- `docs/network-incident-examples/2026-06-19-beta-matt-alex/README.md` if example usage is useful
- `tests/` parser fixture or script test
- `package.json` or existing test harness files only if a new dependency is truly necessary

## Implementation Checklist

- [x] Define the parser input and output contract.
- [x] Strip ANSI log decoration and parse structured field/value pairs robustly.
- [x] Aggregate per-match and per-player diagnostic maxima/buckets.
- [x] Add evidence-bounded classification and missing-data warnings.
- [x] Add focused parser tests or fixture validation.
- [x] Document local/Fly usage and interpretation.
- [x] Mark this phase as done in this file.

## Verification

- parser list/help command, for example `node scripts/parse-net-report-logs.mjs --help`
- parser run against the Matt/Alex incident logs or a minimized fixture
- focused parser test if added
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If the parser intentionally avoids a formal test because fixture shape is still changing, run it
against the Matt/Alex incident files and include the exact output path or key summary rows in the
handoff.

## Manual Test Focus

Run the parser against the preserved Matt/Alex log windows. Confirm it reproduces the known conclusion:
server health was clean, Matt had bad RTT/snapshot timing and low frame pacing, and the available
evidence does not prove a WebSocket transport rewrite would have fixed the incident. Confirm missing
payload/command fields from older logs are reported as unavailable instead of zero.

## Handoff Expectations

Provide the parser command, sample output location or excerpt, and the exact fields used for each
classification. Call out any incident question that remains impossible to answer from the current
diagnostic fields so the next diagnostics plan does not overclaim.

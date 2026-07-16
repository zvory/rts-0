# Phase 10 - Report Net Subscriber Failures Safely

Status: Incomplete.

## Objective

Keep `Net` subscribers isolated while making their exceptions observable. Reporting must be bounded,
non-recursive, useful for diagnosis, and incapable of blocking later subscribers.

## Work

- Add a narrow diagnostic or logging seam for exceptions thrown by a `Net` message subscriber.
- Include the message type and concise error context. Do not log full message payloads, snapshots,
  credentials, or other unnecessary wire data.
- Bound or deduplicate repeated reports so a subscriber that throws on a 30 Hz snapshot path cannot
  flood the console or diagnostics. Make the bound deterministic enough for focused tests.
- Ensure reporting does not dispatch another Net message, invoke subscribers recursively, or throw
  back into `_emit`.
- Continue invoking every later subscriber after an earlier subscriber throws, even when diagnostic
  reporting itself fails.
- Add focused tests for observability, repeated-error bounding, later-subscriber delivery, and a
  throwing reporter.

## Non-goals

- Do not change wire messages, subscription ordering, handler registration APIs, snapshot decoding,
  connection lifecycle, or match teardown.
- Do not add remote telemetry, a general logging framework, or payload capture.
- Do not revisit command interaction or startup rollback completed in Phases 8 and 9.

## Likely Touch Points

- `client/src/net.js`
- `tests/client_contracts/net_contracts.mjs`
- a small existing diagnostics seam if one already fits without coupling Net to Match
- `docs/design/client-ui.md` only if subscriber error handling is documented as a module contract

## Verification

- Focused test proving a throwing subscriber is reported and a later subscriber still receives the
  same event.
- Focused test proving repeated throws on one hot message type produce only the configured bounded
  number of reports.
- Focused test proving a throwing diagnostic reporter neither escapes `_emit` nor blocks later
  subscribers.
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

## Manual Test Focus

No gameplay behavior should change. In one local diagnostic run, trigger a synthetic throwing
subscriber and confirm the error is visible once or within the chosen bound while ordinary messages
continue to flow.

## Handoff

Mark this phase done in its implementation commit. Report the diagnostic seam, reporting bound,
context included and excluded, later-subscriber evidence, and focused verification. Because this is
the final phase, rerun the stewardship checkpoint, mark the plan complete, and allow the owned-PR
workflow to archive it after merge.

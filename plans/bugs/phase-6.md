# Phase 6 - End-to-End Hardening and Documentation

Status: planned.

## Goal

Close the V1 loop with focused integration coverage, deploy-drain verification, and documentation.
This phase should prove the playtester-to-reviewer workflow works in the contexts that matter most
without expanding into a production ticketing system.

## Scope

- Add end-to-end coverage for:
  - in-match report submission
  - replay-viewer report submission
  - human-vs-AI and solo report-backed replay resolution through the current deployed replay upload
    path
  - AI-only report-backed replay resolution where practical, including hidden-from-Recent-Matches
    behavior
  - pending-to-available replay resolution through `replay_key`
  - match ending after the report form opens but before submission, if earlier phases support that
    lifecycle directly
  - report dashboard replay launch near the report tick
- Add focused contract coverage for the architectural seams introduced by earlier phases:
  - `ReportStore` returns errors on required writes instead of swallowing them
  - `ReplayEvidenceRegistry` transitions pending to available or missing deterministically
  - `RoomReportContext` is captured by the room task and client-supplied ids remain hints
  - `ClientReportService` bounds payload size and composes snapshot providers through injection
  - `ReplayReviewLaunch` preserves existing replay compatibility checks
- Verify deploy drain waits for required report/replay writes or document the exact deadline
  behavior if the existing drain timeout wins.
- Update design docs and capsules to reflect the shipped report system.
- Enable lobby reporting only if the earlier phases established a clear nullable-replay product
  contract and the UI can explain that lobby reports may lack replay evidence.
- Remove temporary flags, harness-only endpoints, and debugging logs that are not part of V1.

## Touch Points

- `tests/server_integration.mjs`
- `tests/regression.mjs` if report payload bounds or hardening are covered there
- focused Rust tests under `server/src` or `server/crates/*`
- `tests/select-suites.mjs` if new files need suite mapping
- `docs/design/match-history.md`
- `docs/design/client-ui.md`
- `docs/design/protocol.md` if protocol changed
- `docs/context/*.md` capsules whose code maps or invariants changed
- `.env.example` or deployment docs only if a new env var was introduced

## Constraints

- Do not add screenshots, categories, auth, privacy controls, rate limiting, annotations, or
  advanced search during hardening.
- Do not broaden test scope into long AI/balance gates unless implementation touched AI strategy,
  deterministic replay semantics, or long self-play behavior.
- Keep dashboard UI utilitarian and compact.
- Do not collapse the new primitives into one broad "bug reports" module during cleanup. Cleanup
  should remove temporary scaffolding while preserving the explicit persistence, room-context,
  client-service, and replay-launch boundaries.

## Verification

- Run targeted Rust tests for report persistence/replay durability.
- Run the relevant Node live-server integration suite covering report submission and review launch.
- Run `node scripts/check-client-architecture.mjs` if client modules changed.
- Run `node tests/select-suites.mjs --verify` if selector rules changed.
- Let the normal commit hook provide full-suite coverage when the phase is ready to merge.

## Manual Testing Focus

- Friend/playtester flow: submit report from live match with empty text, copy/report id.
- Friend/playtester flow: submit report from replay viewer with text.
- Developer flow: open dashboard, find the report, open the replay near the report tick, and read
  context without leaving the review flow confused.
- Shutdown/deploy flow: start drain while a report write is in progress and confirm behavior matches
  the documented policy.

## Handoff

After implementation, mark this phase done and summarize the verified end-to-end flows, the final
manual review URL, the final architecture seams that future work should reuse, remaining known
limitations, and any later non-V1 ideas such as server-authoritative pause or screenshot capture.

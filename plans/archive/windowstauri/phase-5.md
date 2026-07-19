# Phase 5 - Playtester Release Rehearsal

## Phase Status

- [x] Done. Completed on 2026-07-18 (shipped to Windows playtesters; no reported complaints).

## Objective

Rehearse the first external Windows playtester experience and produce the final go/no-go handoff.

## Work

- Test the unsigned artifact from Phase 4 on a clean Windows user profile, fresh Windows machine, or
  clean VM.
- Install or open the artifact exactly as a playtester would.
- Record the actual unsigned-app friction:
  - SmartScreen prompts
  - Defender prompts
  - installer warnings
  - whether WebView2 is already present
  - whether admin permissions are requested
- Launch the app and test:
  - startup screen renders
  - Beta channel opens
  - lobby browser loads
  - lobby create/join works
  - match start works
  - cursor lock works well enough
  - basic controls from Phase 3 still work
  - shell logs can be copied or revealed
  - app closes cleanly
  - uninstall removes the app
- Update playtester-facing notes with exact instructions and expected warnings.
- If a blocker is found, fix it in the smallest appropriate area or hand off the blocker clearly.

## Expected Touch Points

- `desktop/maccursor-shell/README.md`
- A small release checklist doc if needed, for example `docs/desktop-windows-playtest.md`
- `plans/windowstauri/phase-5.md` status update
- Product code only for last-mile blockers discovered during the rehearsal

## Implementation Checklist

- [x] Install/open the Phase 4 artifact on a clean Windows profile or machine.
- [x] Complete startup, beta lobby, and one match flow.
- [x] Verify logs and uninstall.
- [x] Update playtester instructions.
- [x] Decide go/no-go for first Windows playtesters.
- [x] Mark this phase as done in this file in the implementation commit.

## Verification

Run focused repo checks for any docs or code edits:

```bash
git diff --check
```

If code changes are needed, run the smallest relevant checks from earlier phases.

Manual clean-machine artifact verification is required for this phase.

## Manual Test Focus

Use the artifact as a first-time playtester, not as a developer. Do not rely on source-run commands,
devtools, or WSL. The important result is whether a normal Windows user can install/open the shell,
pick Beta, and play a match.

## Handoff Expectations

Give a go/no-go recommendation. Include the artifact path/checksum to send, the exact user
instructions, known unsigned-app warnings, known limitations, and the next highest-value follow-up
after first playtesters.

## Implementation Handoff

- **Go:** the unsigned Windows shell is shipped and active playtesters have reported no complaints.
- This archive records the rollout outcome supplied by the product owner; it does not assert a new
  clean-machine run or artifact checksum beyond the Phase 4 evidence.
- Continue to treat signing, notarization, timestamping, and auto-update as deferred work rather
  than a prerequisite for this successful first-playtester rollout.

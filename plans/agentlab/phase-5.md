# Phase 5 - Real-Time Video And Contact Sheets

Status: planned.

## Prerequisite

Phase 3 must be approved and Phase 4 must have established stable artifact/session metadata before
video manifests depend on it.

## Goal

Let an agent record a short real-time clip while it continues to manipulate the scene with normal
MCP tools. The output should be easy for both humans and the model to review: a WebM for motion,
representative PNG frames/contact sheet for inspection, and a manifest tying the media to the
authoritative session.

## Scope

- Add bounded `lab_record_start` and `lab_record_stop` tools. Starting records the current clean
  viewport while later `lab_order`, `lab_time`, mutation, and camera calls continue through the same
  session; stopping finalizes the artifact and returns its metadata.
- Use Puppeteer's supported page screencast path and installed FFmpeg to produce a 30 FPS VP9 WebM
  with no audio. Record only the game viewport/clean presentation crop, not browser or OS chrome.
- Allow a small set of recording options: safe name, maximum duration, viewport/crop, and optional
  scale. Keep camera orchestration, scene mutation, and orders in their existing tools rather than
  embedding an action DSL in recording options.
- Enforce one active recorder per session, a short default maximum, a hard upper duration/size
  bound, and a watchdog that finalizes or aborts safely if the client disappears.
- Return recorder state from `lab_inspect` or a narrow status result so the agent can correct
  duplicate start/stop calls.
- Generate representative PNG frames and a contact sheet after finalization. Sampling should cover
  start, end, and bounded change/activity points where possible without decoding the entire video
  into model context.
- Return the contact sheet or representative frames as MCP image content plus absolute paths to the
  WebM, frames/contact sheet, and manifest. The model should inspect the stills; the user can open
  the video artifact for full motion.
- Record authoritative start/end ticks, room-time speed/state changes, camera changes, subject
  aliases, issued MCP operations during the window, dropped/duplicated frame diagnostics where
  available, Chrome/Puppeteer/FFmpeg versions, and media probe results in the manifest.
- Add FFmpeg/ffprobe capability checks with actionable errors before recording starts.
- Make recorder finalization and process teardown idempotent. Closing a session with an active
  recorder should finalize within a bound or delete a clearly marked partial file.

## Expected Touch Points

- Agent Lab driver/MCP media recorder and schemas
- shared clean presentation/readiness from Phase 3
- ignored target artifact structure and manifest helpers
- `tests/package.json`/lockfile only if current Puppeteer APIs require a pinned adjustment
- focused recording lifecycle tests and one live movement/combat recording smoke
- Agent Lab skill/docs for the start -> manipulate -> stop -> inspect workflow

## Constraints

- Do not use Computer Use, Browser Use, OS-level screen recording, microphone/system audio, or an
  interactive visible browser.
- Do not promise tick-perfect or frame-perfect determinism. This phase records real-time Chrome
  composition and is for visual confirmation.
- Do not add a monolithic `recordScenario(actions...)` tool or duplicate existing lab/order/time
  schemas inside recording options.
- Do not return video bytes directly to model context. Return paths/resource metadata and bounded
  still-image inspection aids.
- Do not record longer than the configured hard limit or leave FFmpeg running after the MCP/session
  closes.
- Keep all media under the ignored agent-lab target root.

## Verification

- Add lifecycle tests for start, duplicate start, stop, duplicate stop, timeout/watchdog, session
  close during recording, FFmpeg missing, Chrome/page failure, and partial-file cleanup.
- Add a live smoke that spawns an aliased unit, starts recording, issues a movement order, advances
  room time, stops recording, and verifies WebM existence, ffprobe codec/dimensions/duration, contact
  sheet dimensions, manifest ticks/actions, and zero page/frame/render errors.
- Add a bounded combat smoke or manual fixture with one attacker and one target so a firing event is
  represented in the contact sheet.
- Run the Phase 3 screenshot smoke to ensure video changes did not regress still capture.
- Run relevant client/MCP contracts, `node scripts/check-docs-health.mjs`, and suite-selector
  verification when mapped files change.

## Manual Testing Focus

- Record a tank moving and turning while the camera remains fixed, then inspect the contact sheet
  and open the WebM.
- Record one tank firing at another and confirm muzzle flash/tracer/recoil frames appear in the
  contact sheet or explain why real-time sampling missed them.
- Interrupt an active recording and confirm no Chrome/FFmpeg/server process or misleading completed
  artifact remains.

## Handoff

After implementation, mark this phase done and report recording tool schemas, duration/size bounds,
codec/crop behavior, contact-sheet sampling, manifest diagnostics, FFmpeg lifecycle, focused media
probe results, and manually reviewed clips. Identify every observed source of nondeterminism that
Phase 6 must address.

# Phase 5 - Real-Time Video And Contact Sheets

Status: done.

## Prerequisite

Phase 0 and the migrated screenshot review must be approved, and Phase 4 must establish stable
artifact/session metadata before video manifests depend on it.

## Goal

Let an agent record a short real-time clip while continuing to manipulate the persistent scene with
normal CLI commands. Produce a WebM for motion, representative PNG frames/contact sheet for
inspection, and a manifest tied to authoritative session evidence.

## Scope

- Add bounded `interact lab record-start` and `interact lab record-stop` commands requiring the
  current opaque `sessionId`. Starting records the clean viewport while later order, time,
  mutation, and camera commands continue through the same open daemon session; stopping finalizes
  and returns artifact metadata and absolute paths.
- Use Puppeteer's supported page screencast path and installed FFmpeg to produce a 30 FPS VP9 WebM
  without audio. Record only the game viewport/clean presentation crop, not browser or OS chrome.
- Accept only safe name, maximum duration, viewport/crop, and optional scale. Keep scene mutation,
  camera orchestration, and orders in their existing commands rather than adding an action DSL.
- Enforce one active recorder for the open session, a short default maximum, hard duration/size
  limits, and a watchdog that finalizes or aborts safely if the client disappears.
- Include recorder state in `status` or bounded `inspect` output so duplicate start/stop calls
  produce correctable errors.
- Generate representative PNG frames and a contact sheet after finalization. Sample start, end, and
  bounded change/activity points where practical without decoding the entire video into CLI output.
- Return JSON paths to the WebM, frames/contact sheet, and manifest. The agent should inspect the
  contact sheet or representative PNGs with the local image viewer and share the video path for
  full motion.
- Record authoritative start/end ticks, room-time changes, camera changes, aliases, accepted CLI
  operations during the window, dropped/duplicated-frame diagnostics, runtime versions, and media
  probe results in the manifest.
- Check FFmpeg/ffprobe capabilities before recording and return actionable failures.
- Make finalization idempotent. Session `close` should finalize within a bound or delete a clearly
  marked partial; `shutdown` and idle teardown must additionally remove recorder processes and
  transient runtime files.

## Expected Touch Points

- Interact daemon/driver recorder and CLI schemas
- shared clean presentation and renderer readiness from Phase 3
- ignored `target/interact/lab/` artifact structure and manifest helpers
- pinned browser dependencies only if the current Puppeteer API requires adjustment
- recording lifecycle tests and a live movement/combat smoke
- Interact skill and documentation for start, manipulate, stop, inspect, and share

## Constraints

- Do not use Computer Use, Browser Use, OS-level screen recording, microphone/system audio, or an
  interactive visible browser.
- Do not promise tick-perfect or frame-perfect determinism. This phase records real-time Chrome
  composition for visual review.
- Do not add a monolithic `record-scenario` action language or duplicate existing operation schemas
  inside recorder options.
- Do not print video bytes or large frame data. Return bounded metadata and confined absolute paths.
- Do not exceed hard duration/size limits or leave FFmpeg running after session close, daemon
  shutdown, or idle teardown.
- Keep every media artifact under `target/interact/lab/`.

## Verification

- Test start, duplicate start, stop, duplicate stop, watchdog timeout, session close while
  recording, daemon shutdown/idle, missing FFmpeg, page failure, and partial-file cleanup.
- Add a live CLI smoke that opens, spawns an aliased unit, starts recording, orders movement,
  advances room time, stops, and verifies WebM codec/dimensions/duration, contact sheet dimensions,
  manifest ticks/actions, and zero page/frame/render errors.
- Add a bounded combat fixture covering one attacker and target so a firing event appears in
  representative frames when sampling permits.
- Re-run the Phase 3 screenshot smoke and Phase 4 reopen flow to catch capture/session regressions.
- Run focused client/CLI contracts, `node scripts/check-docs-health.mjs`, and suite-selection
  verification when mapped files change.

## Manual Testing Focus

- Record a tank moving and turning with a fixed camera, inspect the contact sheet, and open the WebM.
- Record one tank firing at another and confirm muzzle flash/tracer/recoil appears in sampled frames
  or document why real-time sampling missed it.
- Interrupt recording through close and shutdown and confirm no recorder, Chrome, Rust server, or
  misleading completed artifact remains.

## Handoff

After implementation, mark this phase done and report CLI schemas, duration/size bounds,
codec/crop behavior, contact-sheet sampling, manifest diagnostics, FFmpeg lifecycle, focused media
probe results, and manually reviewed clips. Identify every observed source of nondeterminism that
Phase 6 must address and confirm normal daemon idle semantics remain intact.

# Tauri MVP Shell Shipping Plan

## Phase Summaries

### Phase 1 - Startup Server Picker

Replace immediate server launch with a shell-owned startup screen that lets players choose beta,
mainline, a local dev URL, or a custom server URL before the game loads. The default choices should
include beta and mainline, while custom URLs are normalized, validated, persisted, and shown again on
later launches. The shell must load the selected website and provide native cursor support without
owning the game server or static assets.

### Phase 2 - Thin Shell Runtime Boundary

Remove the shell's current server-spawning assumption from the shippable path. The app should never
bundle or launch `rts-server`, `client/`, maps, or other game assets; those are fetched from the
selected server origin just like a browser would. The local option should be a convenience profile
for a developer who has already checked out the repo and started their own local server.

### Phase 3 - Basic Logs And Failure Surfaces

Add basic persistent logs for shell startup, selected server profile, navigation/connectivity
failures, and native cursor failures. Startup and connection failures should be visible inside the
shell instead of only in a terminal, and the app should make the log location easy to find or copy.
Keep this small: the goal is enough evidence for playtester support, not a full telemetry system.

### Phase 4 - Unsigned Release Build Path

Add a repeatable command that builds an unsigned macOS playtest artifact and records exactly what is
inside it. The build should include only the Tauri shell, version/build metadata, and a minimal
README covering how to open the unsigned app, choose a server, and find logs. Do not add bundled game
assets, notarization, signing, auto-update, or tester-facing "what to test" notes in this phase.

### Phase 5 - Final Manual Gate

Run the delayed in-game plausibility gate only after the startup picker, packaging, and logging work
has landed. The final pass should use the built app, verify beta/mainline/custom/local startup paths,
then do the native cursor gameplay checks from the original maccursor Phase 4. Any small fixes found
here can land in this phase, but do not start this phase until someone can actually test on macOS.

## Cross-Phase Constraints

- Keep this macOS-only. Do not design Windows or Linux support for the MVP shell.
- Do not add signing, notarization, auto-update, crash reporting services, or release workflow
  automation beyond a local unsigned artifact command.
- Preserve normal browser play and the current browser Pointer Lock path.
- Keep the desktop native cursor path feature-gated through the Tauri runtime bridge.
- Remote server URLs must be explicit player choices. Allow `https://` for remote servers and allow
  plain `http://` only for loopback/local development.
- The shipped shell must be thin. Do not bundle `rts-server`, `client/`, maps, lab scenarios, or
  other game assets; load all game content from the selected website.
- The startup selector should ship with beta and mainline defaults. Verify the exact URLs from repo
  deploy config or current deployment evidence during implementation.
- Custom server entries should persist in the user's app config, not in repo files.
- A remote custom URL is a trust boundary. Do not send secrets, local file paths, or elevated shell
  capabilities to arbitrary sites; keep Tauri command access limited to the native cursor and
  desktop-shell commands required by this app.
- The local profile is only a URL shortcut to a server the user started separately. The app must not
  start, stop, configure, or package that local server.
- Keep the final manual gate at the end. Earlier phases may include small smoke checks, but they
  should not depend on the user being home to do gameplay testing.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what changed, what the next agent should do, and the core behavior that should be manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Suggested Phase Runner Usage

Run phases one at a time from a clean checkout:

```bash
scripts/phase-runner.sh --plan taurimvp 1 --pr --wait
scripts/phase-runner.sh --plan taurimvp 2 --pr --wait
scripts/phase-runner.sh --plan taurimvp 3 --pr --wait
scripts/phase-runner.sh --plan taurimvp 4 --pr --wait
```

Do not run Phase 5 until a macOS manual playtest is available:

```bash
scripts/phase-runner.sh --plan taurimvp 5 --pr --wait
```

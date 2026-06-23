# Tauri MVP Shell Shipping Plan

## Phase Summaries

### Phase 1 - Startup Server Picker

Replace immediate server launch with a shell-owned startup screen that lets players choose beta,
mainline, local, or a custom server URL before the game loads. The default choices should include
beta and mainline, while custom URLs are normalized, validated, persisted, and shown again on later
launches. Remote servers must load with the same native cursor bridge and desktop runtime metadata
that local mode uses today.

### Phase 2 - Packaged Local Runtime

Make local mode work from a copied app bundle instead of requiring Cargo and a source checkout. The
Tauri shell should launch a bundled optimized `rts-server` binary, point it at bundled client and map
assets, and keep the current dev `cargo run` path only for source-tree development. This phase should
turn the shell from a developer spike into an unsigned app bundle shape that can run on another Mac.

### Phase 3 - Basic Logs And Failure Surfaces

Add basic persistent logs for shell startup, selected server profile, local server stdout/stderr, and
native cursor failures. Startup and connection failures should be visible inside the shell instead of
only in a terminal, and the app should make the log location easy to find or copy. Keep this small:
the goal is enough evidence for playtester support, not a full telemetry system.

### Phase 4 - Unsigned Release Build Path

Add a repeatable command that builds an unsigned macOS playtest artifact and records exactly what is
inside it. The build should include the Tauri app, bundled local runtime resources, version/build
metadata, and a minimal README covering how to open the unsigned app and where logs live. Do not add
notarization, signing, auto-update, or tester-facing "what to test" notes in this phase.

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
- The startup selector should ship with beta and mainline defaults. Verify the exact URLs from repo
  deploy config or current deployment evidence during implementation.
- Custom server entries should persist in the user's app config, not in repo files.
- A remote custom URL is a trust boundary. Do not send secrets, local file paths, or elevated shell
  capabilities to arbitrary sites; keep Tauri command access limited to the native cursor and
  desktop-shell commands required by this app.
- Local mode must not write match history unless the bundled server is explicitly configured to do
  so; preserve the existing server env-gated behavior.
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

# Phase 4 - Opportunistic audio unlock

WKWebView and WebView2 both refuse to start an `AudioContext` until a user
gesture. The current client already tries to satisfy this by listening for the
player's first interaction anywhere in the page. Keep that behavior and harden
it for the Tauri shell instead of adding a first-launch prompt.

The target experience is: launch the app, use the lobby normally, enter a match,
and audio works after any ordinary interaction. No modal. No separate "enable
sound" step. If a player needs to inspect or change audio state, use the
existing in-game settings panel that already owns the volume controls.

## What to add

Make audio unlock opportunistic and session-wide:

- Keep `AudioContext` construction lazy. Do not create it at module load.
- Treat any trusted user interaction as an unlock opportunity:
  `pointerdown`, `pointerup`, `click`, `mousedown`, `touchstart`, `keydown`,
  and the first interaction with any lobby, HUD, canvas, or settings control.
- Register the unlock listeners once at app/audio construction time, in capture
  phase where useful, so they run before navigation, lobby actions, pointer lock,
  fullscreen, or command handling consumes the event.
- After the first successful unlock, remove the one-shot listeners and decode
  queued manifest entries.
- If the browser creates the context in a suspended state, call `resume()` from
  the same gesture handler and treat the unlock as successful only once the
  context is running or resumable without throwing.
- Expose a small `audio.isUnlocked()` / `audio.unlockFromGesture()` style API if
  the settings UI needs to show state or retry explicitly. Keep the API local to
  client wiring; this is not a wire-protocol change.

Use the existing settings menu as the manual fallback:

- Add a compact non-modal audio status row above the volume sliders only when
  audio is still locked after the settings menu is opened.
- The row should offer a normal settings control such as "Start audio" or
  "Retry audio"; activating it calls the same unlock path.
- Hide the row once audio is unlocked. Do not block match start, lobby controls,
  hotkeys, or pointer lock.

## Why not a modal

The modal makes first launch feel broken even when the player was already about
to provide a valid gesture by joining a lobby, starting a match, clicking the
map, pressing a hotkey, opening settings, or using pointer lock. It also creates
a second audio settings surface despite the game already having one.

The right failure mode is silent until the first ordinary interaction, with a
retry available in settings if the platform refuses the first attempt.

## Verification

1. Cold launch of the Tauri app: no modal appears; lobby is immediately usable.
2. Join/start using mouse only: the first click unlocks audio, queued sounds
   decode, and the first match event with a loaded sound plays.
3. Join/start using keyboard only: the first key press unlocks audio and match
   audio works.
4. Open the in-game settings menu before audio unlock: a compact retry/status
   row appears above the existing sliders; activating it unlocks audio and hides
   the row.
5. Reload via devtools in a debug build: no prompt appears; the next ordinary
   interaction unlocks audio again for that page session.
6. Browser fallback opening the URL: same no-modal behavior works in Safari,
   Chromium, and Firefox where supported.

## Exit criteria

- No first-launch modal or blocking overlay exists.
- Audio unlock is attempted from normal user interactions across lobby, HUD,
  canvas, settings, and keyboard flows.
- The only explicit retry surface is inside the existing settings menu.
- No "silent first match after normal interaction" reports across two
  playtesters' first launches on the Tauri app.

## Risks

- Some engines require `AudioContext.resume()` even immediately after context
  creation. The unlock helper must handle both create-and-running and
  create-then-resume flows.
- If a first gesture also triggers pointer lock/fullscreen, unlock must run
  early enough that both browser policies see the same trusted event.
- Manifest decoding may still take time after unlock. Keep `play()` tolerant of
  missing buffers and verify important UI/alert sounds are decoded quickly after
  the first gesture.
- Do not add DOM/window listeners without matching teardown in `Audio.destroy()`.

## Out of scope

- New modal, splash screen, or first-launch audio prompt.
- New standalone audio preferences UI. The game already has an audio settings
  panel with category sliders.

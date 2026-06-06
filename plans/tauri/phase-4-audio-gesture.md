# Phase 4 — Audio gesture prompt

WKWebView and WebView2 both refuse to start an `AudioContext` until a user
gesture. The current client `audio.js` may rely on the player's first click in
the lobby satisfying this implicitly. In the Tauri shell that has held up in
manual testing on Safari, but it is fragile: any future flow that auto-starts a
match (a watch-link, a quickstart) would land in-game with silence and no log.

## What to add

An explicit one-time gesture gate before the lobby is interactable.

- A small modal on first launch of the app session: title "Enable sound",
  one button "Continue". Clicking it resumes the `AudioContext` and dismisses
  the modal.
- Persist a `sessionStorage` flag so it shows once per launch, not once per
  match.
- Wire it into `client/src/bootstrap.js` before `main.js` mounts the lobby.

## Why a modal rather than "click anywhere"

Playtesters will not know that the silence is a gesture-policy problem. A modal
is explicit, takes one click, and the same code path covers macOS, Windows, and
the browser fallback.

## Verification

1. Cold launch of the Tauri app: modal appears, click Continue, lobby loads,
   audio plays at the first event after entering a match.
2. Reload via devtools (debug build): modal does not reappear within the same
   session.
3. Browser (non-Tauri) opening the URL: same modal appears and works.

## Exit criteria

- No "silent first match" reports across two playtesters' first launches.

## Risks

- The modal must not block hotkeys before being dismissed. Trap focus inside it
  so Enter / Space activates Continue.
- The `AudioContext` must be created lazily after the gesture, not at module
  load — verify `audio.js` does not eagerly construct it. If it does, defer
  construction.

## Out of scope

A real audio settings panel (volume sliders per category). Already present or
deferred — not in this plan.

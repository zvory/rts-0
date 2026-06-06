# Phase 1 — Point at the live server

Replace the placeholder page with a navigation to the live fly.io server. After
this phase the desktop app is a usable (if rough) playtest client. Mouse lock and
fullscreen still go through the existing browser-API paths from `match.js`; the
native versions land in Phase 2 and Phase 3.

## The one constant

A single Rust constant in `desktop/src-tauri/src/main.rs`:

```rust
const SERVER_URL: &str = "https://rts-0-zvorygin.fly.dev/";
```

This is the only place to change before cutting a playtest build. When the
`bewegungskrieg.net` domain comes online, edit this line and rebuild. No env vars,
no config file, no UI picker — the user explicitly asked for one hardcoded spot.

## Webview wiring

In `tauri.conf.json`, set the main window's `url` to `SERVER_URL` (Tauri v2
supports passing an external URL directly as the window URL via `app.windows[].url`).
If template expansion of the constant into the JSON is awkward, instead:

- Leave `tauri.conf.json` URL as `index.html`.
- In `dist/index.html`, ship a one-line redirect: `<script>location.replace("…")</script>`.
- Have `main.rs` inject the URL at build time via an environment variable read by a
  small build script that writes the redirect target into `dist/index.html`.

Pick whichever path keeps the URL in exactly one source file. Default to the
`tauri.conf.json` approach if Tauri v2's external-URL support is stable; fall back
to the redirect if not.

## Window config

- `width: 1600, height: 900`, `minWidth: 1024, minHeight: 640`.
- `resizable: true`, `fullscreen: false`, `decorations: true`.
- `title: "Bewegungskrieg!"`.
- Disable the default Tauri menu on macOS (no File/Edit menu we don't need); keep
  the standard window controls.
- Right-click context menu is already suppressed by the existing
  `client/index.html:173` handler — verify it survives in the webview.

## CSP

`tauri.conf.json` `app.security.csp`:

```
default-src 'self' https://rts-0-zvorygin.fly.dev;
connect-src 'self' https://rts-0-zvorygin.fly.dev wss://rts-0-zvorygin.fly.dev;
script-src 'self' https://rts-0-zvorygin.fly.dev https://cdn.jsdelivr.net 'unsafe-inline';
style-src 'self' https://rts-0-zvorygin.fly.dev 'unsafe-inline';
img-src 'self' https://rts-0-zvorygin.fly.dev data:;
media-src 'self' https://rts-0-zvorygin.fly.dev;
font-src 'self' https://rts-0-zvorygin.fly.dev data:;
```

`cdn.jsdelivr.net` stays until Phase 6 vendors Pixi.

## Verification

1. `cargo tauri dev` — window opens directly into the live lobby UI served by
   fly.io.
2. Type a name, hit Join — confirm WebSocket connects (network panel via webview
   devtools, enabled in dev builds).
3. Start a 1-player sandbox; click around; confirm audio plays after the first
   click (this is the gesture-policy issue Phase 4 addresses).
4. Build `cargo tauri build`, install, repeat the smoke test from the bundled app.

## Exit criteria

- Bundled `.app` launches directly into the fly.io lobby.
- A full match (1-player sandbox is enough) plays without errors in the webview
  console.
- Existing browser-API pointer lock and `requestFullscreen` from `match.js` still
  work as they did in Safari — these are placeholders to be replaced in Phase 2/3.

## Risks

- **Mixed-content nothing**: the server is HTTPS, so `wss://` is fine. Good.
- **Webview console for production builds**: Tauri v2 disables devtools in release
  by default. Keep it that way for playtester builds; expose it only via a debug
  feature flag in the cargo build for local debugging.
- **Pixi CDN dependency**: a jsdelivr outage at launch bricks the app. Tolerated
  until Phase 6.
- **localStorage scoping**: under `https://rts-0-zvorygin.fly.dev` origin the
  client's stored settings persist correctly across launches. Verify, because the
  earlier "load from `tauri://localhost`" alternative would have lost them.

## Out of scope

Native cursor handling. Native fullscreen. Windows builds. Vendoring Pixi.

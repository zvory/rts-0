# Phase 6 — Vendor PixiJS

Remove the runtime dependency on `cdn.jsdelivr.net`. After this phase, a jsdelivr
outage cannot brick a desktop launch or a browser load.

This is intentionally last because every prior phase works fine with the CDN
script tag, and vendoring touches a contract (`index.html`) that everyone — desktop
shell, browser players, dev pages — shares.

## What to change

1. Download `pixi.js@7.4.2` UMD build, place at `client/vendor/pixi-7.4.2.min.js`.
   Include the matching license file alongside it.
2. Replace `client/index.html:17`'s `<script src="https://cdn.jsdelivr.net/...">`
   with `<script src="./vendor/pixi-7.4.2.min.js"></script>`.
3. Same swap in `client/map-editor.html` and any `/dev/*` page that pulls Pixi.
4. Drop `https://cdn.jsdelivr.net` from the CSP `script-src` in
   `desktop/src-tauri/tauri.conf.json`.
5. Drop the `cdn.jsdelivr.net` line from `docs/design/client-ui.md` if it's mentioned there as a
   contract, and from `CLAUDE.md` if relevant.

## Verification

1. Disconnect network briefly after the server is reached — confirm Pixi still
   loads (it should, because the URL is `https://rts-0-zvorygin.fly.dev/vendor/...`
   and the server is what brought us here; the point is no jsdelivr fetch).
2. Network panel shows no request to `cdn.jsdelivr.net`.
3. All client tests (the three Node integration scripts and `client_smoke.mjs`)
   still pass.

## Exit criteria

- No `cdn.jsdelivr.net` reference anywhere in `client/` or `desktop/`.
- Bundle size of `client/` increases by ~500 KB minified — acceptable.

## Risks

- Pixi's UMD vs. ESM split. The current setup loads UMD and uses the global
  `PIXI`. Stay on UMD; do not switch loader styles in this phase.
- A future Pixi v8 migration is already tracked elsewhere — not in scope here.

## Out of scope

Any other CDN or third-party fetch. PixiJS v8 upgrade.

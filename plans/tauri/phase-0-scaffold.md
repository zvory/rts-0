# Phase 0 — Scaffold and macOS smoke build

Stand up a Tauri v2 project alongside the existing repo and prove a `.app` opens a
window showing arbitrary HTML. No server integration yet. Goal: kill all toolchain
risk before touching any client code.

## Layout

A new top-level directory `desktop/` (sibling of `server/` and `client/`). It owns:

```
desktop/
  src-tauri/
    Cargo.toml
    tauri.conf.json
    src/main.rs
    icons/
  dist/
    index.html        # placeholder; replaced in Phase 1
```

Rationale for a top-level dir rather than nesting under `client/`: the Rust
toolchain in `desktop/src-tauri/` is independent of the game server crate, and the
GitHub Actions workflow can target it without pulling in the server build.

## Steps

1. Install `cargo install create-tauri-app` (or use `npm create tauri-app`) on
   macOS. Pick the **vanilla / no frontend framework** template, since the client
   is plain HTML + ES modules.
2. Set `productName = "Bewegungskrieg"`, `identifier = "net.bewegungskrieg.client"`,
   version `0.1.0`.
3. Bundle config: enable `dmg` for macOS; leave Windows bundle config in place but
   unused locally. Disable updater.
4. `dist/index.html`: a one-page placeholder that says "Bewegungskrieg shell —
   Phase 0" in big text. No JS.
5. `cargo tauri dev` from `desktop/` — confirm a window opens.
6. `cargo tauri build` — confirm a `.dmg` and `.app` land in
   `desktop/src-tauri/target/release/bundle/`.
7. Drag the `.app` to `/Applications`, double-click. Expect Gatekeeper to refuse.
   Run `xattr -dr com.apple.quarantine "/Applications/Bewegungskrieg.app"` and
   relaunch — confirm it opens.

## Exit criteria

- Unsigned `.app` launches from `/Applications` after one `xattr` command.
- The window shows the placeholder HTML.
- `desktop/` is committed; `desktop/src-tauri/target/` is gitignored.

## Risks

- Tauri v2 dependency drift on macOS — pin the exact CLI version in a note in
  `desktop/README.md` so the Windows runner can match it later.
- Icon assets: Tauri needs `.icns` and `.ico`. Generate from `client/cover-art.png`
  via `cargo tauri icon` so we don't block on art.

## Out of scope

Anything client-related. Anything Windows-related. CI. Server URL.

# Bewegungskrieg Desktop

Phase 1 Tauri shell for the native desktop client. It opens the live
Bewegungskrieg server directly inside the desktop webview.

## Toolchain

- Tauri CLI: `tauri-cli 2.11.2`
- Rust: edition 2021, minimum `1.77.2` for the Tauri crate
- Bundle identifier: `net.bewegungskrieg.client`

Install the pinned CLI with:

```bash
cargo install tauri-cli --version 2.11.2 --locked
```

## Commands

Run the shell:

```bash
cargo tauri dev
```

Run the shell with desktop debugging enabled:

```bash
../scripts/run-desktop-debug.sh
```

Build the unsigned macOS bundle:

```bash
cargo tauri build
```

Or use the helper from the repo root, which builds and prints the exact output
paths:

```bash
./scripts/build-desktop.sh
```

Build a debuggable internal bundle:

```bash
./scripts/build-desktop-debug.sh
```

The macOS `.app` and `.dmg` are written under
`src-tauri/target/release/bundle/`.

See `../docs/desktop-debugging.md` for Web Inspector, URL override, and client timing marks.

## Windows CI

GitHub Actions builds an unsigned Windows NSIS `.exe` on pushes to `main` that
touch `desktop/**`, on manual workflow dispatches, and when a GitHub Release is
published. The workflow uses `tauri-cli 2.11.2`, builds only the NSIS target, and
uploads the installer as a workflow artifact. On release events, it also attaches
the installer to the published release.

The Windows installer embeds the WebView2 bootstrapper so older Windows 10
machines can install the runtime if it is missing. First-time testers may need
to choose "More info" then "Run anyway" in SmartScreen for the unsigned
installer.

Release flow:

1. Bump `version` in `src-tauri/tauri.conf.json`.
2. Tag `desktop-v0.X.Y`.
3. Publish a GitHub Release from the tag; CI attaches the `.exe`.
4. Build the macOS `.dmg` locally with `cargo tauri build` and upload it to the
   same release manually.

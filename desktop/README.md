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

## Release CI

GitHub Actions builds unsigned desktop bundles when a GitHub Release is
published, and on manual workflow dispatches for packaging checks. The workflow
uses `tauri-cli 2.11.2` and uploads the build outputs as workflow artifacts.
On release events, it also attaches them to the published release:

- macOS `.dmg`
- macOS `.app.zip` containing the `.app` bundle
- Windows NSIS `.exe`

The Windows installer embeds the WebView2 bootstrapper so older Windows 10
machines can install the runtime if it is missing. First-time testers may need
to choose "More info" then "Run anyway" in SmartScreen for the unsigned
installer.

Release flow:

1. Bump `version` in `src-tauri/tauri.conf.json`.
2. Tag `desktop-v0.X.Y`.
3. Publish a GitHub Release from the tag; CI attaches the `.dmg`, `.app.zip`,
   and `.exe`.

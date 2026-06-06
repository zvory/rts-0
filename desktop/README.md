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

Build the unsigned macOS bundle:

```bash
cargo tauri build
```

Or use the helper from the repo root, which builds and prints the exact output
paths:

```bash
./scripts/build-desktop.sh
```

The macOS `.app` and `.dmg` are written under
`src-tauri/target/release/bundle/`.

# Bewegungskrieg Desktop

Phase 0 Tauri shell for the native desktop client. It intentionally loads only
`dist/index.html`; game client integration starts in a later phase.

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

The macOS `.app` and `.dmg` are written under
`src-tauri/target/release/bundle/`.

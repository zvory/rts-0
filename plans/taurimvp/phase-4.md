# Phase 4 - Unsigned Release Build Path

## Phase Status

- [ ] Planned.

## Plain-Language Summary

Create one repeatable command for producing the unsigned macOS MVP shell artifact. The output should
be a copied app bundle or zip for the thin shell and enough metadata to tell which build a tester
ran. Keep the instructions focused on opening the unsigned app, choosing a server, and finding logs.

## Objective

Make the playtester artifact reproducible without adding signing, notarization, auto-update, or a
full release workflow.

## Scope

- Add a local build command or script for the unsigned macOS playtest artifact.
- Include build metadata such as git SHA, date, target architecture, and shell version in an
  artifact manifest.
- Produce an artifact name that includes version/SHA/architecture.
- Include a minimal README beside the artifact covering:
  - how to open the unsigned app on macOS,
  - beta/mainline startup choices,
  - where logs live.
- Confirm the artifact does not include `rts-server`, client assets, maps, lab scenarios, or other
  game runtime assets.
- Do not include tester-facing gameplay "what to test" notes.
- Document prerequisites for the developer building the artifact.
- Keep the existing PR full gate as the authoritative broad test gate; add only focused local checks
  needed by this release script.

## Expected Touch Points

- New build script under `desktop/maccursor-shell/` or `scripts/`
- `desktop/maccursor-shell/README.md`
- `desktop/maccursor-shell/src-tauri/tauri.conf.json`
- build metadata helper files if needed

Avoid touching:

- GitHub release automation
- signing or notarization config
- updater config
- unrelated deploy scripts

## Verification

- Run the new unsigned artifact build command.
- Run `cargo test --manifest-path desktop/maccursor-shell/src-tauri/Cargo.toml`.
- Run focused checks named by previous phases.
- Confirm the artifact manifest and README are present in the output.
- Inspect the artifact contents and confirm it is a thin shell only.

## Manual Testing Focus

Open the produced unsigned artifact on the build Mac and confirm the startup selector appears.

## Handoff Expectations

The handoff must include the exact build command, artifact path, artifact contents, target
architecture, confirmation that no game server/assets are bundled, and any known issue a playtester
may hit while opening an unsigned app.

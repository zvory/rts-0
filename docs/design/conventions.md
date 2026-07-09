## 6. Conventions
- Rust: edition 2021; owned PRs format only their touched Rust files through the pinned toolchain,
  `#![deny(warnings)]` off (warnings ok), no `unwrap()` on
  network/parse paths — handle errors and keep the room alive. Prefer small pure functions in
  `services/`. Avoid panics in the tick loop.
- JS: ES2020 modules, no framework, small classes per §4, JSDoc on public methods, no global
  state except `PIXI`. Pure helpers where possible.
- Both: names match this doc. Document any deviation here in the same change.
- Coordinates: world pixels everywhere on the wire; tiles only where a field ends in `Tile`.

---

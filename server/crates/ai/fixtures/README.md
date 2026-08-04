# AI controller transcript fixtures

`jeff_live_oracle_v1.jsonl` is the immutable schema-1 production-controller specimen for Jeff's
AI. It runs two `jeffs_ai` controllers on authored `Chokes` with seed `0x4a45_4646`; ordinary tests
compare ticks 0-3599 and `RTS_FULL_AI_TESTS=1` compares ticks 0-8999.

Generate a review candidate, which writes only below this crate's `target/` directory, with:

```bash
RTS_FULL_AI_TESTS=1 RTS_GENERATE_JEFF_ORACLE_CANDIDATE=1 \
  cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default -p rts-ai \
  -E 'test(/jeff_live_oracle/)'
```

The resulting file is
`server/crates/ai/target/jeff-live-oracle/candidate-v1.jsonl`. Generation never updates this
directory. The checked-in fixture must not be regenerated to make behavior-preserving refactors
pass; changing it requires an approved production behavior change with a transcript review. The
depot-extractor economy migration is such a behavior change: candidate review must confirm the new
starting loadout and initial extractor-repeat commands before replacing the fixture.

Canonical records are compact UTF-8 JSON Lines with one LF-terminated serde struct record per
line. Field order is the Rust schema declaration order. Fingerprinted floating-point values are
rounded to 1/1024 of a world unit before compact `serde_json` encoding so harmless CPU-specific
subpixel arithmetic does not masquerade as AI drift. Integer, string, ordering, and larger numeric
changes remain visible in the lowercase `fnv1a64:<hex>` values; exact emitted `SimCommand` values
remain in the fixture so command drift is reviewable without reversing a hash.
The manifest's profile fingerprint is independently derived from an explicit serde policy record,
not compiler-dependent Rust `Debug` text.

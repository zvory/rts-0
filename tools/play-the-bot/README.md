# play_the_bot

`play_the_bot.exe` creates a fresh lobby on the current Beta server, joins as the spectator host,
adds **Jeff's AI** (not AI 2.1), and waits for one human opponent. As soon as that opponent readies,
the launcher requests the match start. A one-human-plus-AI room skips the normal countdown, so the
game begins immediately.

The console prints the room name to join. Keep it open while playing; it reports the match result
and the observation/replay id when the server provides one.

Build from the repository root:

```powershell
$env:CARGO_TARGET_DIR = "target/play-the-bot"
cargo build --release --manifest-path tools/play-the-bot/Cargo.toml
```

The executable is written to `target/play-the-bot/release/play_the_bot.exe`.

Optional command-line overrides:

```powershell
play_the_bot.exe --room "Practice Room" --server beta
play_the_bot.exe --room "Practice Room" --server https://example.test/
```

Automatic entry is intentionally limited to servers that expose the `jeffs_ai` live profile.

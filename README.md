# Bewegungskrieg

![Bewegungskrieg cover art](cover-art.png)

Bewegungskrieg is a small real-time-strategy game inspired by classic base-building RTS games.
Gather steel and oil, expand your base, train an army, scout through fog of war, and defeat the
other players.

The game is server-authoritative: a Rust server owns the simulation, serves the static browser
client, and streams fog-filtered snapshots over WebSocket. The client is plain HTML/CSS/JS with
PixiJS loaded from a CDN, so there is no JavaScript build step.

For architecture, protocol, balance, hardening rules, and module contracts, start with
[DESIGN.md](DESIGN.md), then follow the relevant `docs/design/` link. This README is only the
quickstart.

## Play Locally

Requirements:

- A recent Rust toolchain with `cargo`
- A browser with WebGL enabled

Start the server:

```bash
./runserver
```

Open the URL printed by the server, usually `http://127.0.0.1:8080/` or
`http://localhost:8080/`.

To change the bind address:

```bash
RTS_ADDR=127.0.0.1:8090 ./runserver
```

## Starting A Match

Open the game in one or more browser windows.

- Join the same room to play together.
- The host can add AI opponents from the lobby.
- All human players must ready up before the host starts the match.
- A one-player match with no AI is a never-ending sandbox.
- The lobby's "Start with more money mode" gives every player a larger opening economy.

Matches support up to four total players, counting humans and AI.

## Game Basics

- Engineers gather steel and oil.
- Buildings let you increase supply and unlock new units.
- Units can move, attack, attack-move, stop, gather, build, and train through the command card.
- Fog of war is enforced by the server, so hidden enemies are not sent to the client.
- Last player standing wins in matches with two or more players.

## Controls

| Action | Input |
|--------|-------|
| Select unit or building | Left-click |
| Box-select | Left-drag |
| Move, gather, or attack | Right-click |
| Command card | Q W E / A S D / Z X C |
| Pan camera | WASD, arrow keys, screen edge, or minimap drag |
| Zoom | Mouse wheel |
| Cancel placement | Esc or right-click |

## Developer Quickstart

Useful entry points:

- `server/` contains the Rust server and authoritative simulation.
- `client/` contains the static browser client served by the Rust process.
- `tests/` contains live-server integration and browser smoke tests.
- [DESIGN.md](DESIGN.md) indexes the source-of-truth contract docs under `docs/design/`.
- [tests/README.md](tests/README.md) explains the test suites in detail.

Common commands:

```bash
# Run the game.
./runserver

# Build, lint, and format the Rust crate.
cd server && cargo build && cargo clippy && cargo fmt

# Run the full local test orchestrator from the repo root.
tests/run-all.sh
```

The full test runner starts or reuses a local server, runs the Rust simulation tests, runs the
WebSocket/API suites, and runs the headless client smoke test when Chrome is available.

For focused test runs, see [tests/README.md](tests/README.md).

## Main-Branch Test Gate

Anything landing on `main` must pass the canonical local CI command:

```bash
./tests/run-all.sh
```

Install the tracked Git hooks in each checkout:

```bash
./scripts/install-hooks.sh
```

The hooks run `./tests/run-all.sh` before direct commits to `main` and before non-fast-forward
merge commits into `main`. Feature-branch commits stay fast, but merging them into `main` should
use a merge commit so the gate can run:

```bash
git merge --no-ff <branch>
```

Git does not distribute active local hook configuration through clones, so GitHub Actions also runs
`./tests/run-all.sh` on pushes and pull requests targeting `main`. To require this for everyone on
other machines, protect `main` in GitHub and require the `./tests/run-all.sh` status check.

## Deploy

The app is configured for Fly.io through [fly.toml](fly.toml):

```bash
flyctl deploy --ha=false
```

First-time setup and operational notes live in [docs/fly.md](docs/fly.md).

# Lag Elimination Requirements

This is a product requirements artifact, not an implementation plan. It defines the player-facing
lag standard without prescribing protocol, rollback, simulation, or rendering architecture.

## Purpose

Severe command lag is killing the game. A normal online match must feel playable on home Wi-Fi, not
only on ideal wired connections. A healthy server is not enough if the player feels that commands
are delayed, swallowed, or confirmed only after a remote echo.

The product target is the feel of a responsive competitive RTS: predictable, low command delay;
immediate confidence that orders are accepted; and no large confusing snapbacks.

## Product Model

Adopt the StarCraft-style feel: predictable latency is acceptable when it is short, consistent, and
absorbed into the command cadence. Unpredictable latency is not acceptable. The player can adapt to
a tiny stable delay; they cannot play around commands that sometimes happen instantly, sometimes
half a second later, and sometimes appear to undo themselves.

Core commands should not wait silently for authoritative snapshot confirmation. They should be
accepted into a near-future command cadence and then produce provisional local world response for
player-owned state. This means more than a sound, marker, or UI flash: owned units, queues, rally
state, build intent, and other local command surfaces should begin behaving as if the accepted
command is real when the cadence reaches it.

Server authority still wins. The product requirement is not that the client becomes authoritative,
nor that this document chooses a rollback or local-simulation architecture. The requirement is that
authoritative correction must be bounded, legible, and rare enough that players trust the local
world they are controlling.

## Requirements

- Commands use a short, predictable response cadence rather than variable remote-echo latency.
- Core player commands produce provisional owned-world response, not only UI acknowledgement.
- Move, stop, hold, attack, attack-move, rally, build, train, research, gather, and ability intents
  must all feel accepted and locally legible.
- Local feedback must make pending commands feel accepted, not lost.
- Large snapbacks, repeated undo/replay behavior, and half-second delayed restarts are not an
  acceptable solution.
- Corrections must be understandable from the player's point of view when they happen.
- Local frame pacing is part of the lag problem; command responsiveness is not solved if the client
  cannot display the response promptly.
- Evaluation must include normal home Wi-Fi conditions, network jitter, bursty delivery, and weaker
  client machines.

## Non-Goals

- Do not make this only a diagnostics effort.
- Do not define success by server tick health alone.
- Do not solve only movement smoothing while other commands remain delayed.
- Do not accept UI-only feedback as a substitute for owned-world response.
- Do not choose a specific implementation architecture in this requirements document.

## Success Criteria

- Players can micro, retreat, attack, build, rally, and produce without feeling blocked by the
  network.
- Command response feels stable and predictable during ordinary online play.
- Poor conditions degrade gracefully instead of making commands feel random or swallowed.
- Playtest reports no longer describe normal matches as unplayable because of command response.

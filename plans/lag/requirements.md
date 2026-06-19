# Lag Elimination Requirements

This is a product requirements artifact, not an implementation plan. It describes the player
problem, the target feel, and the standard future technical work must meet without prescribing how
the system should be built.

## Purpose

Severe lag is a game-killing problem. A real playtest between nearby North American players made a
match feel unplayable even though the server itself stayed healthy. That outcome is unacceptable:
the game has to feel playable on normal home Wi-Fi, not only on ideal wired connections.

This effort exists to make command response, visual feedback, and moment-to-moment control feel
decisive under realistic network and client conditions. It is not enough to explain lag, label it,
or smooth only the parts players already tolerate. The player must feel that their commands are
heard immediately and that the game remains controllable during ordinary online play.

## Problem

The current product can fail in a way that is worse than visible movement stutter: commands can feel
late, uncertain, or swallowed. When that happens, players cannot trust micro, production, retreats,
attacks, or emergency reactions. The match stops feeling competitive and starts feeling broken.

This problem has several player-visible faces:

- command input feels delayed after clicks or hotkeys;
- the game can appear to ignore or defer decisions until later confirmation;
- local frame stalls amplify every network delay;
- smooth unit movement is not enough if command response still feels bad;
- a healthy server does not protect the product from feeling unplayable.

The product standard must be based on perceived control, not just server metrics. A match can be
technically authoritative, internally consistent, and still fail players if the command loop feels
mushy.

## Product Feel

The target feel is a responsive competitive RTS.

When a player gives a command, the game should immediately communicate that the command was
received by the local experience. The player should not have to wait for a remote update before the
interface, audio, cursor state, command markers, selected-unit affordances, or other visible
feedback tells them the action registered.

Ordinary Wi-Fi should feel playable. Moderate RTT, jitter, and bursty snapshot delivery should not
turn basic command response into a half-second uncertainty loop. Poor network conditions may still
cause correction, degraded precision, or warnings, but they must not make normal play feel like the
client is frozen behind the server.

The game should preserve trust. If an action cannot happen, the player should learn that quickly and
clearly. If an action is pending, the player should feel that it is pending, not lost. If the
authoritative outcome differs from the player's immediate local experience, the correction should
feel understandable and bounded rather than random.

## Severity

Treat this as a top-tier product reliability issue. Lag that makes the game unplayable is not a
polish bug, not a diagnostics gap, and not something to defer behind feature work. It directly
blocks the game from being fun, testable, and credible in multiplayer.

Future work should be judged against whether it aggressively reduces the player's felt loss of
control. Changes that merely document the problem, expose another debug number, or improve one
narrow animation while leaving commands feeling delayed do not satisfy this requirement.

## Requirements

### Command Responsiveness

- Player commands must produce immediate local feedback.
- Feedback must cover core live-match actions, not only unit movement.
- The player should never wonder whether a click, hotkey, or command-card action was accepted by
  the client.
- Pending command state should be visible or otherwise legible when the authoritative result is not
  available yet.
- Rejected or impossible actions should fail quickly and clearly.

### Match Control

- Players must be able to micro units under realistic online conditions.
- Emergency actions such as retreating, stopping, attacking, rallying, building, and producing must
  feel responsive enough to trust during fights.
- The game should remain playable when network timing is imperfect but still within normal consumer
  Wi-Fi expectations.
- The product should not rely on players interpreting status badges or debug values to understand
  whether their commands are working.

### Frame Pacing

- Local frame stalls are part of the lag problem because they directly affect input feel.
- The product must remain responsive enough on ordinary player machines to show command feedback
  promptly.
- Network responsiveness work is incomplete if local frame pacing still makes input feel delayed.

### Authority and Trust

- Authoritative outcomes must remain trustworthy.
- Responsiveness work must not create a product where players routinely see large, confusing
  reversals of recent actions.
- Corrections should be rare, bounded, and understandable from the player's point of view.
- The player experience should prefer quick acknowledgement and clear correction over silence.

### Realistic Conditions

- The target environment includes home Wi-Fi.
- A player geographically close to the server should not experience unplayable command lag.
- Testing and evaluation must include network jitter, bursty delivery, and weaker client machines,
  not only local ideal paths.
- Success should be validated through player-visible feel as well as telemetry.

## Non-Goals

- Do not make this only a diagnostics effort.
- Do not treat a healthy server tick rate as sufficient success.
- Do not solve only movement smoothing while leaving core commands delayed.
- Do not define success around ideal wired-network conditions.
- Do not accept a product where the player must tolerate severe command uncertainty in normal
  multiplayer.
- Do not prescribe implementation architecture in this requirements document.

## Success Criteria

- In realistic online play, commands feel accepted immediately.
- Players can make tactical decisions without feeling that the game is fighting their input.
- Movement, combat control, production, construction, rallying, and other common actions all have
  credible local responsiveness.
- Poor conditions degrade gracefully instead of making the game feel broken.
- Playtest reports no longer describe ordinary matches as unplayable because of command response.
- Telemetry can explain remaining lag, but the primary success signal is that players trust the
  command loop.

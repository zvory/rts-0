# Tournament Grounds

## Phase 0: building brief — complete

User authorized implementation and supplied the fantasy, input/output, footprint, cost, prerequisite,
rally behavior and art direction. Tournament Grounds is the initial player-facing name. Cultivator
Engineers build an arena that trades four ordinary Disciples for one randomly specialized
Disciple. The opportunity cost is three army bodies and the risk of losing entrants when the
building is destroyed. Opponents can punish the five-second conversion window or destroy the arena.
Description: “Send four base Disciples inside to begin a free five-second tournament. One winner
emerges with a random Dao specialization. Supports a rally point.”

## Phase 1: rules and balance — complete

- Cultivators only; Engineer build card; 100 steel, zero oil; completed Portal prerequisite.
- 4×4 tiles, ordinary solid building placement/pathing; 200 ticks construction (6.67 seconds).
- 120 HP, armored, one-tile sight, no attack, no supply provided/used; ordinary construction,
  cancellation/refund, damage, fog memory and destruction behavior.
- Right-click own completed Grounds with base Disciples (Warrior kind, no Dao). Entrants walk to
  the perimeter and are consumed on arrival. Other units and enemy/allied disciples are rejected.
- Capacity four. The fourth entrant starts 150 ticks (five seconds), with no resource charge,
  research prerequisite or additional command. While running it stages further entrants outside for the next tournament.
- One full-health Disciple emerges, drawing Wind, War or Magic with
  equal probability from the simulation RNG. Existing first Dao evolution rules apply, including
  War HP. Winners start with zero insight and the evolved-Disciple cap of 200.
- Entrants immediately release supply. The winner uses normal three supply; its three supply are
  reserved when the tournament starts, preventing other production from claiming their slots.
  The fourth entrant waits outside if replacing its two supply with the winner's three would
  exceed the cap; earlier entrants' released supply may already have been used by other production.
- Spawn through normal legal building exits and follow the full rally plan. Blocked exits retain
  pending winners for retry. Destruction loses entrants/pending winners and releases reservations.
- Entrant count is team-private. Tournament progress is visible whenever the building is visible.
  Progress/fullness bars and selected-building text explain the two stages.
- Durable state belongs to each entity and is checkpoint/replay serialized. Entry intent cancels
  when another order replaces it. No new public Game API is needed.
- Available in Lab; AI can encounter/attack it but autonomous AI tournament strategy is deferred.
- White-painted, low-detail isometric generated arena art, using Nexus/Pavilion references and
  magenta source background; existing team tint, construction and damage presentation apply.
- Audio: existing generic building feedback only; bespoke tournament audio deferred.

## Verification / delivery checklist

- [x] Phase 0/1 reviewed against the user's request before implementation.
- [x] Rules/catalog/client/protocol mirrors and design references updated.
- [x] Admission, timing, output, random-specialization eligibility, rally, blocked exits, interruption/destruction,
  supply, fog and checkpoint behavior tested.
- [x] In-game Lab inspection completed, including blue/orange art, three entrants, running timer and one output.
- [ ] Focused checks and authoritative PR gate passed; PR merged.

Initial art review used two team-colored grounds. Final Interact review trained four Disciples
at a Cultivator Portal rallied into the grounds and confirmed one upgraded winner at its rally point.
The normal-command Game tests additionally cover Cultivator rally orders and checkpoint restore
both during a tournament and after winners spawn. Bespoke audio and AI strategy remain deferred.

Portal rally: right-click owned completed Grounds while a Portal is selected. The client snaps
the rally point to the arena center. Newly trained base Disciples use normal legal perimeter
approaches and enter automatically; additional arrivals wait through a running tournament.

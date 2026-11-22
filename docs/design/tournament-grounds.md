# Tournament Grounds

## Phase 0: building brief — complete

User authorized implementation and supplied the fantasy, input/output, footprint, cost, prerequisite,
rally behavior and art direction. Tournament Grounds is the initial player-facing name. Cultivator
Engineers build an arena that trades eight ordinary Disciples for three randomly specialized
Disciples. The opportunity cost is five army bodies and the risk of losing entrants when the
building is destroyed. Opponents can punish the ten-second conversion window or destroy the arena.
Description: “Send eight base Disciples inside to begin a free ten-second tournament. Three winners
emerge with random Dao specializations. Supports a rally point.”

## Phase 1: rules and balance — complete

- Cultivators only; Engineer build card; 100 steel, zero oil; completed Portal prerequisite.
- 3×3 tiles, ordinary solid building placement/pathing; 200 ticks construction (6.67 seconds).
- 120 HP, armored, one-tile sight, no attack, no supply provided/used; ordinary construction,
  cancellation/refund, damage, fog memory and destruction behavior.
- Right-click own completed Grounds with base Disciples (Warrior kind, no Dao). Entrants walk to
  the perimeter and are consumed on arrival. Other units and enemy/allied disciples are rejected.
- Capacity eight. The eighth entrant starts 300 ticks (ten seconds), with no resource charge,
  research prerequisite or additional command. While running it rejects further entrants.
- Three full-health Disciples emerge, each independently drawing Lightness, War or Magic with
  equal probability from the simulation RNG. Existing first Dao evolution rules apply, including
  War HP. Winners carry the existing evolved-Disciple 60-insight invariant. Duplicates are possible.
- Entrants immediately release supply. Winners use normal two supply each; their six supply are
  reserved when the tournament starts, preventing other production from claiming their slots.
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
- [x] In-game Lab inspection completed, including blue/orange art, seven entrants, running timer and three outputs.
- [ ] Focused checks and authoritative PR gate passed; PR merged.

Local review used the Interact Lab scene with two team-colored grounds and eight Disciples.
The normal-command Game tests additionally cover Cultivator rally orders and checkpoint restore
both during a tournament and after winners spawn. Bespoke audio and AI strategy remain deferred.

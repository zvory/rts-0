# Jeff attack-path defense

- Jeff now builds his home infantry line across the map analyzer's likely attack corridors instead
  of projecting a straight line from his steel field toward the enemy start.
- His first two Machine Gunners anchor the center of the primary approach. On a single lane they
  are separated by 12 tiles, giving their seven-tile entrenched ranges about two tiles of overlap.
- Riflemen occupy alternating left and right flank slots. The first Rifleman sits 11 tiles beyond
  the outer Machine Gunner. Extras form staggered `010101` and `101010` ranks on a triangular
  lattice: same-rank and diagonal neighbors remain 10 tiles apart, maintaining about two tiles of
  overlap between entrenched attack ranges in every direction.
- Later Rifleman pairs rotate across primary and alternate analyzed approaches instead of widening
  one formation until units clamp against the map edge. Alternate lanes without an MG anchor begin
  with a centered Rifleman pair whose entrenched ranges retain the same two-tile overlap.
- With two completed Resource Depots, the two Machine Gunners split across the primary approaches
  to the home and defensibility-ranked expansion sites.
- Entrenched Riflemen and Machine Gunners no longer abandon their wall to chase a single local
  contact; mobile Scout Cars may still intercept while the line engages automatically.
- Jeff now classifies a push as a breakthrough only after it crosses an analyzed interception line
  or enters the immediate defense radius of an owned building.
- A breakthrough first counts the combat value already covered by entrenched infantry, deployed
  Anti-Tank Guns, and the home Tank's stationary 14-tile firing envelope. Jeff pulls only enough
  additional uncovered combat value to match the visible penetration, preferring mobile reserves
  and preserving the home Tank and infantry anchors as long as their existing fire covers it.
- Borrowed defenders are excluded from offensive waves after the attack. Infantry is sent back to
  its analyzed wall slots, the home Tank returns to its charged-range line, and other responders
  form a reserve rank four tiles behind the primary interception line. They remain recovery-locked
  until they have had at least three seconds to settle, with a 30-second failsafe for unreachable
  positions.
- Economy, costs, damage, durability, and production targets are unchanged.
- No new UI controls are added. Watch for terrain-constrained flank slots that become too wide or
  for enemies that penetrate behind a static line without entering its overlapping firing arcs.

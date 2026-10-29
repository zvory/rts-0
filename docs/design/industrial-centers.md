# Industrial Center

## Approved brief and rules (Phases 0 and 1)

The requested Industrial Center is an in-place Resource Depot upgrade, available in normal play.
It trades 800 steel, 300 oil and 60 seconds (1,800 ticks) for 100 global supply while alive.
Kriegsia starts at 100 supply; the absolute maximum remains 300. At most two owned centers,
including pending upgrades, may exist. A destroyed center frees its slot; existing units survive
being over cap, but new supply cannot be reserved until room is available.

The source depot keeps its identity, HP, armor, footprint, sight, mining-anchor status, worker
training, free extractor jobs, rally orders, repair, elimination and targeting behavior. There is
no new weapon, capture rule, aura or research prerequisite. Upgrade through the normal research
command/card (first free research slot); cancellation uses ordinary research refunds. Pending
unpaid upgrades reserve a center slot. Completion is local to the depot, not permanent player
research. Starting a second upgrade on the same depot or a third across the player is rejected.

The completed sprite uses the simplified Refinery Works art: three white-painted storage tanks,
a processing tower, dark pipes and an orange furnace. White paint receives owner tint; there is
no gold footprint rim. The ordinary Resource Depot keeps its worksite art. Existing construction,
damage, selection, minimap and death visuals stay; dedicated audio is deferred. Completion is
visible to anyone who can see the building, retained in fog memory, and preserved in checkpoints
and replays; queue/payment details retain existing owner-only projection. AI should buy this
upgrade when approaching its current supply cap.

This change applies only to Kriegsia. Ekat and Cultivators retain their existing 300 supply
allowance and cannot research Industrial Centers.


## Delivery checklist

- [x] Phase 0: purpose, tradeoff, source, interactions and placeholder presentation recorded.
- [x] Phase 1: cost, timing, limits, inherited stats, loss/cancellation and AI rules specified.
- [x] Contracts, simulation, UI, visuals and focused checks complete.
- Owned PR delivery is tracked by the PR lifecycle, not this specification.

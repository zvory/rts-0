# Small Fixes

## Resource-node harvest slot ownership

The single-miner resource-node slot is split between `ResourceNodeState.miner` on the node and the worker's current gather order/phase. The validity predicate is duplicated in `world_query::node_holder` and `economy::slot_held`, while `EntityStore::release_miner` mutates the node by inspecting the worker's order.

This should be collapsed into one authoritative helper/service so commands, economy, and cleanup all agree on when a node slot is held or released.

## Path cache empty-path entries

`PathingService` caches every path returned by pathfinding, including `Vec::new()`. Pathfinding uses an empty vector both for "already at the goal" and for "no useful path/fallback failed", and `cache_lookup` treats an empty cached path as valid because there are no tiles to revalidate.

This should stop caching failed empty paths, or encode the path result so "already there" and "failed to find a path" are distinct states.

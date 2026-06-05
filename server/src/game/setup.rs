use super::*;

impl Game {
    #[allow(dead_code)]
    pub fn new(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(
            players,
            true,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            AiProfileSelection::Default,
        )
    }

    /// Create a live lobby match where each AI picks one strategy from the live profile pool.
    pub fn new_with_random_ai_profiles(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(
            players,
            true,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            AiProfileSelection::Random,
        )
    }

    /// Create a match with explicit starting resources for every player.
    #[allow(dead_code)]
    pub fn new_with_starting_resources(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(players, true, steel, oil, seed, AiProfileSelection::Default)
    }

    /// Create a live lobby match with explicit starting resources and randomized AI strategies.
    pub fn new_with_starting_resources_and_random_ai_profiles(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(players, true, steel, oil, seed, AiProfileSelection::Random)
    }

    #[cfg(test)]
    pub(crate) fn new_for_replay(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_without_ai_controllers(players, seed)
    }

    /// Like [`Game::new_for_replay`] but with explicit starting resources. Used when replaying a
    /// match that was originally created in quickstart ("start with more money") mode so the
    /// initial player economy matches the live recording.
    pub(crate) fn new_for_replay_with_starting_resources(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(
            players,
            false,
            steel,
            oil,
            seed,
            AiProfileSelection::Default,
        )
    }

    /// Create a match that preserves player identity flags but does not attach live
    /// [`AiController`]s. Used by command-log replay and scripted self-play, where commands come
    /// from an external driver.
    pub(crate) fn new_without_ai_controllers(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(
            players,
            false,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            AiProfileSelection::Default,
        )
    }

    pub(crate) fn seed(&self) -> u32 {
        self.seed
    }

    pub(crate) fn starting_steel(&self) -> u32 {
        self.starting_steel
    }

    pub(crate) fn starting_oil(&self) -> u32 {
        self.starting_oil
    }

    #[cfg(test)]
    pub(super) fn ai_profile_ids(&self) -> Vec<&'static str> {
        self.ai.iter().map(AiController::profile_id).collect()
    }

    fn new_inner(
        players: &[PlayerInit],
        enable_ai: bool,
        steel: u32,
        oil: u32,
        seed: u32,
        ai_profile_selection: AiProfileSelection,
    ) -> Game {
        let map = Map::generate(players.len(), seed);
        let fog = Fog::new(map.size);
        let mut entities = EntityStore::new();
        let mut ai_profile_rng = SmallRng::seed_from_u64((seed as u64) ^ 0xA17E_5EED);

        let mut player_states = Vec::with_capacity(players.len());
        let mut ai = Vec::new();
        for (i, p) in players.iter().enumerate() {
            let start = map.starts.get(i).copied().unwrap_or((0, 0));
            if enable_ai && p.is_ai {
                let profile_id = match ai_profile_selection {
                    AiProfileSelection::Default => ai::DEFAULT_LIVE_PROFILE_ID,
                    AiProfileSelection::Random => ai::random_live_profile_id(&mut ai_profile_rng),
                };
                ai.push(AiController::with_profile_id(p.id, profile_id));
            }
            let mut ps = PlayerState {
                id: p.id,
                name: p.name.clone(),
                color: p.color.clone(),
                start_tile: start,
                steel,
                oil,
                supply_used: 0,
                supply_cap: 0,
                is_ai: p.is_ai,
                score: ScoreState::default(),
            };
            spawn_player_start(&mut entities, &map, &mut ps, start);
            // The starting City Centre contributes supply immediately.
            ps.supply_cap = config::CITY_CENTRE_SUPPLY.min(config::SUPPLY_CAP_MAX);
            player_states.push(ps);
        }

        // Always spawn resources on the neutral expansion sites. Claimed sites get a full start;
        // unclaimed sites still get their resource clusters so every player has somewhere to
        // expand.
        for site in &map.expansion_sites {
            if !map.starts.contains(site) {
                spawn_base_resources(&mut entities, &map, *site);
            }
        }

        let spatial = services::spatial::SpatialIndex::build(&entities, map.size);
        let pathing = services::pathing::PathingService::new(8_192, 256);
        let rng = SmallRng::seed_from_u64(seed as u64);
        let mut game = Game {
            map,
            entities,
            fog,
            players: player_states,
            ai,
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
            seed,
            starting_steel: steel,
            starting_oil: oil,
            rng,
        };
        // Initialize supply accounting and fog so the very first snapshot is correct.
        systems::recompute_supply(&mut game.players, &game.entities);
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog.recompute(&ids, &game.entities, &game.map);
        game
    }

    /// Static info for the `start` message: terrain grid + each player's start tile. The
    /// `player_id` is left 0; the networking layer overwrites it per recipient.
    pub fn start_payload(&self) -> StartPayload {
        let resources = self
            .entities
            .iter()
            .filter(|e| e.kind.is_node())
            .map(|e| ResourceNode {
                id: e.id,
                kind: e.kind.to_protocol_str().to_string(),
                x: e.pos_x,
                y: e.pos_y,
            })
            .collect();
        let map = MapInfo {
            width: self.map.size,
            height: self.map.size,
            tile_size: config::TILE_SIZE,
            terrain: self.map.terrain.clone(),
            resources,
        };
        let players = self
            .players
            .iter()
            .map(|p| PlayerStart {
                id: p.id,
                name: p.name.clone(),
                color: p.color.clone(),
                start_tile_x: p.start_tile.0,
                start_tile_y: p.start_tile.1,
            })
            .collect();
        StartPayload {
            player_id: 0,
            spectator: false,
            tick: self.tick,
            map,
            players,
        }
    }
}

/// Spawn a player's full starting layout: a free, fully-built City Centre on the start tile, a ring of
/// workers around it, and a nearby neutral resource cluster (steel + one oil node).
///
/// Spawn the steel and oil clusters for a base site. The clusters point inward toward the map
/// center so the layout is the same regardless of whether a player occupies the site.
fn spawn_base_resources(entities: &mut EntityStore, map: &Map, tile: (u32, u32)) {
    let (tx, ty) = tile;
    let (hx, hy) = map.tile_center(tx, ty);
    let ts = config::TILE_SIZE as f32;

    let center = map.world_size_px() * 0.5;
    let dx = center - hx;
    let dy = center - hy;
    let base_angle = dy.atan2(dx);

    let block_dist = config::STEEL_BLOCK_DIST_TILES * ts;
    let block_cx = hx + block_dist * base_angle.cos();
    let block_cy = hy + block_dist * base_angle.sin();
    let perp_x = -base_angle.sin();
    let perp_y = base_angle.cos();

    let patches = config::STEEL_PATCHES_PER_BASE;
    let cols = 6u32;
    let rows = patches.div_ceil(cols);
    let row_center = (rows - 1) as f32 / 2.0;
    for i in 0..patches {
        let col = (i % cols) as f32;
        let row = (i / cols) as f32;
        let off_x = (col - 2.5) * ts;
        let off_y = (row - row_center) * ts;
        let px = block_cx + off_x * perp_x + off_y * base_angle.cos();
        let py = block_cy + off_x * perp_y + off_y * base_angle.sin();
        let dist_tiles = ((px - hx).powi(2) + (py - hy).powi(2)).sqrt() / ts;
        debug_assert!(
            (config::CC_RESOURCE_MIN_DIST_TILES..=config::CC_RESOURCE_MAX_DIST_TILES)
                .contains(&dist_tiles),
            "steel patch {i} at {dist_tiles:.2} tiles from City Centre is out of [{:.1}, {:.1}] bounds",
            config::CC_RESOURCE_MIN_DIST_TILES,
            config::CC_RESOURCE_MAX_DIST_TILES
        );
        entities.spawn_node(EntityKind::Steel, px, py);
    }

    let oil_angle = base_angle + std::f32::consts::FRAC_PI_2;
    let oil_perp_x = -oil_angle.sin();
    let oil_perp_y = oil_angle.cos();
    let oil_dist = config::OIL_DIST_TILES * ts;
    let oil_cx = hx + oil_dist * oil_angle.cos();
    let oil_cy = hy + oil_dist * oil_angle.sin();
    for i in 0..config::OIL_PATCHES_PER_BASE {
        let (off_x, off_y) = match i {
            0 => (-0.5 * ts, -0.5 * ts),
            1 => (0.5 * ts, -0.5 * ts),
            _ => (0.0, 0.5 * ts),
        };
        let px = oil_cx + off_x * oil_perp_x + off_y * oil_angle.cos();
        let py = oil_cy + off_x * oil_perp_y + off_y * oil_angle.sin();
        let dist_tiles = ((px - hx).powi(2) + (py - hy).powi(2)).sqrt() / ts;
        debug_assert!(
            (config::CC_RESOURCE_MIN_DIST_TILES..=config::CC_RESOURCE_MAX_DIST_TILES)
                .contains(&dist_tiles),
            "oil patch {i} at {dist_tiles:.2} tiles from City Centre is out of [{:.1}, {:.1}] bounds",
            config::CC_RESOURCE_MIN_DIST_TILES,
            config::CC_RESOURCE_MAX_DIST_TILES
        );
        entities.spawn_node(EntityKind::Oil, px, py);
    }
}

/// Spawn a City Centre, starting workers, and resource clusters for one player.
fn spawn_player_start(
    entities: &mut EntityStore,
    map: &Map,
    player: &mut PlayerState,
    start: (u32, u32),
) {
    let (stx, sty) = start;
    let (hx, hy) = map.tile_center(stx, sty);

    if entities
        .spawn_building(player.id, EntityKind::CityCentre, hx, hy, true)
        .is_some()
    {
        player.record_entity_created(EntityKind::CityCentre);
    }

    let ts = config::TILE_SIZE as f32;
    let ring_r = ts * 2.5;
    let count = config::STARTING_WORKERS;
    for i in 0..count {
        let ang = std::f32::consts::TAU * (i as f32) / (count.max(1) as f32);
        let wx = hx + ring_r * ang.cos();
        let wy = hy + ring_r * ang.sin();
        if entities
            .spawn_unit(player.id, EntityKind::Worker, wx, wy)
            .is_some()
        {
            player.record_entity_created(EntityKind::Worker);
        }
    }

    spawn_base_resources(entities, map, start);
}

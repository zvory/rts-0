//! Detached magic-orb impacts. Launch facts survive caster death and checkpoint restore.
use rts_rules::math::{Coord, GameplayMath as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::game) struct DiscipleShot {
    pub owner: u32,
    pub attacker: u32,
    pub target: u32,
    pub source: [Coord; 2],
    pub endpoint: [Coord; 2],
    pub damage: u32,
    pub penetration: f32,
    pub impact_tick: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::game) struct DiscipleShotStore {
    pub shots: Vec<DiscipleShot>,
}

impl DiscipleShotStore {
    #[allow(clippy::too_many_arguments)]
    pub fn schedule(
        &mut self,
        owner: u32,
        attacker: u32,
        target: u32,
        source: (f32, f32),
        endpoint: (f32, f32),
        damage: u32,
        penetration: f32,
        tick: u32,
    ) {
        // Match disciple_projectiles.js: 100 ms release, at least 60 ms flight, 650 px/s.
        let dx = endpoint.0 - source.0;
        let dy = endpoint.1 - source.1;
        let seconds = 0.1 + ((dx * dx + dy * dy).det_sqrt() / 650.0).max(0.06);
        let delay = (seconds * crate::config::TICK_HZ as f32).ceil() as u32;
        self.shots.push(DiscipleShot {
            owner,
            attacker,
            target,
            source: [Coord::from_world(source.0), Coord::from_world(source.1)],
            endpoint: [Coord::from_world(endpoint.0), Coord::from_world(endpoint.1)],
            damage,
            penetration,
            impact_tick: tick.saturating_add(delay),
        });
    }
}

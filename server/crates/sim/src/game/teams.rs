use super::Game;

pub type TeamId = rts_contract::TeamId;

pub(crate) fn normalize_team_id(player_id: u32, team_id: TeamId) -> TeamId {
    if team_id == 0 {
        player_id
    } else {
        team_id
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TeamRelations {
    players: Vec<(u32, TeamId)>,
}

impl TeamRelations {
    pub(crate) fn from_player_teams(players: impl IntoIterator<Item = (u32, TeamId)>) -> Self {
        Self {
            players: players.into_iter().collect(),
        }
    }

    fn team_of_player(&self, player_id: u32) -> Option<TeamId> {
        self.players
            .iter()
            .find(|(id, _)| *id == player_id)
            .map(|(_, team_id)| *team_id)
    }

    pub(crate) fn same_team_player(&self, a: u32, b: u32) -> bool {
        let Some(team_a) = self.team_of_player(a) else {
            return false;
        };
        let Some(team_b) = self.team_of_player(b) else {
            return false;
        };
        team_a != 0 && team_a == team_b
    }

    fn is_enemy_player(&self, a: u32, b: u32) -> bool {
        a != b
            && self.team_of_player(a).is_some()
            && self.team_of_player(b).is_some()
            && !self.same_team_player(a, b)
    }

    pub(crate) fn is_enemy_owner(&self, player_id: u32, owner: u32) -> bool {
        owner != 0 && self.is_enemy_player(player_id, owner)
    }

    pub(crate) fn same_team_or_same_owner(&self, a: u32, b: u32) -> bool {
        a == b || self.same_team_player(a, b)
    }
}

impl Game {
    pub fn team_of_player(&self, player_id: u32) -> Option<TeamId> {
        self.players
            .iter()
            .find(|player| player.id == player_id)
            .map(|player| player.team_id)
    }

    pub fn same_team_player(&self, a: u32, b: u32) -> bool {
        let Some(team_a) = self.team_of_player(a) else {
            return false;
        };
        let Some(team_b) = self.team_of_player(b) else {
            return false;
        };
        team_a != 0 && team_a == team_b
    }

    pub fn same_team_owner(&self, player_id: u32, owner: u32) -> bool {
        owner != 0 && self.same_team_player(player_id, owner)
    }

    pub fn is_enemy_player(&self, a: u32, b: u32) -> bool {
        a != b
            && self.team_of_player(a).is_some()
            && self.team_of_player(b).is_some()
            && !self.same_team_player(a, b)
    }

    pub fn is_enemy_owner(&self, player_id: u32, owner: u32) -> bool {
        owner != 0 && self.is_enemy_player(player_id, owner)
    }

    pub fn allied_player_ids(&self, player_id: u32) -> Vec<u32> {
        let Some(team_id) = self.team_of_player(player_id) else {
            return Vec::new();
        };
        self.players
            .iter()
            .filter(|player| player.id != player_id && player.team_id == team_id && team_id != 0)
            .map(|player| player.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::game::{Game, PlayerInit};

    #[test]
    fn missing_team_ids_default_to_singleton_ffa() {
        let players = [
            PlayerInit {
                id: 1,
                team_id: 0,
                name: "Alpha".to_string(),
                color: "#4878c8".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                team_id: 0,
                name: "Bravo".to_string(),
                color: "#c84848".to_string(),
                is_ai: false,
            },
        ];
        let game = Game::new_for_replay(&players, 0x7E_AA);
        assert_eq!(game.team_of_player(1), Some(1));
        assert_eq!(game.team_of_player(2), Some(2));
        assert!(game.is_enemy_player(1, 2));
        assert!(!game.same_team_owner(1, 0));
        assert!(!game.is_enemy_owner(1, 0));
        assert!(game.allied_player_ids(1).is_empty());
    }

    #[test]
    fn relationship_helpers_detect_allies_and_enemies() {
        let players = [
            PlayerInit {
                id: 1,
                team_id: 10,
                name: "Alpha".to_string(),
                color: "#4878c8".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                team_id: 10,
                name: "Bravo".to_string(),
                color: "#c84848".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 3,
                team_id: 30,
                name: "Charlie".to_string(),
                color: "#48a868".to_string(),
                is_ai: false,
            },
        ];
        let game = Game::new_for_replay(&players, 0x7E_AB);
        assert!(game.same_team_player(1, 2));
        assert!(game.same_team_owner(1, 2));
        assert!(!game.is_enemy_player(1, 2));
        assert!(game.is_enemy_player(1, 3));
        assert_eq!(game.allied_player_ids(1), vec![2]);
    }
}

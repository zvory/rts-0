use super::*;

impl Passability for TerrainPassability<'_> {
    fn dimensions(&self) -> (u32, u32) {
        (self.map.width, self.map.height)
    }

    fn passable(&self, tx: i32, ty: i32) -> bool {
        let r = self.radius_tiles as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if !self.tile_passable(tx + dx, ty + dy) {
                    return false;
                }
            }
        }
        if self.avoid_diagonal_pinch {
            let nw = !self.tile_passable(tx - 1, ty - 1);
            let ne = !self.tile_passable(tx + 1, ty - 1);
            let sw = !self.tile_passable(tx - 1, ty + 1);
            let se = !self.tile_passable(tx + 1, ty + 1);
            if (nw && se) || (ne && sw) {
                return false;
            }
        }
        true
    }

    fn movement_cost(&self, tx: i32, ty: i32, base_step_cost: u32) -> u32 {
        tree_detours::movement_cost(self, tx, ty, base_step_cost)
    }
}

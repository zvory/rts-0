use std::collections::BinaryHeap;

use super::{Node, SearchKey, NO_INCOMING_DIR};

#[derive(Clone, Default)]
struct DenseState {
    g_score: Vec<u32>,
    parent: Vec<u32>,
    stamp: Vec<u32>,
    generation: u32,
}

impl DenseState {
    fn begin(&mut self, states: usize) {
        if self.stamp.len() < states {
            self.g_score.resize(states, 0);
            self.parent.resize(states, 0);
            self.stamp.resize(states, 0);
        }
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamp.fill(0);
            self.generation = 1;
        }
    }

    #[inline]
    fn get_g(&self, index: usize) -> Option<u32> {
        (self.stamp.get(index).copied() == Some(self.generation)).then(|| self.g_score[index])
    }

    #[inline]
    fn set(&mut self, index: usize, g: u32, parent: usize) {
        self.stamp[index] = self.generation;
        self.g_score[index] = g;
        self.parent[index] = parent as u32;
    }

    #[inline]
    fn parent(&self, index: usize) -> Option<usize> {
        (self.stamp.get(index).copied() == Some(self.generation) && self.parent[index] != u32::MAX)
            .then(|| self.parent[index] as usize)
    }

    #[cfg(test)]
    fn retained_bytes(&self) -> usize {
        self.g_score.capacity() * std::mem::size_of::<u32>()
            + self.parent.capacity() * std::mem::size_of::<u32>()
            + self.stamp.capacity() * std::mem::size_of::<u32>()
    }
}

/// Reusable A* working storage owned by the pathing service.
///
/// Searches are strictly sequential inside one room. Generation stamps logically clear the dense
/// arrays while retaining their allocations; the heap still clears between requests.
#[derive(Default)]
pub(in crate::game) struct SearchScratch {
    pub(super) open: BinaryHeap<Node>,
    ordinary: DenseState,
    directional: DenseState,
    width: u32,
    height: u32,
    start: (i32, i32),
    direction_sensitive: bool,
}

impl Clone for SearchScratch {
    fn clone(&self) -> Self {
        debug_assert!(self.open.is_empty());
        Self::default()
    }
}

impl SearchScratch {
    pub(super) fn begin(
        &mut self,
        dimensions: (u32, u32),
        start: (i32, i32),
        direction_sensitive: bool,
    ) {
        self.open.clear();
        self.width = dimensions.0;
        self.height = dimensions.1;
        self.start = start;
        self.direction_sensitive = direction_sensitive;
        let tiles = (self.width as usize).saturating_mul(self.height as usize);
        if direction_sensitive {
            self.directional
                .begin(tiles.saturating_mul(8).saturating_add(1));
        } else {
            self.ordinary.begin(tiles);
        }
    }

    pub(super) fn finish(&mut self) {
        self.open.clear();
    }

    #[inline]
    fn state(&self) -> &DenseState {
        if self.direction_sensitive {
            &self.directional
        } else {
            &self.ordinary
        }
    }

    #[inline]
    fn state_mut(&mut self) -> &mut DenseState {
        if self.direction_sensitive {
            &mut self.directional
        } else {
            &mut self.ordinary
        }
    }

    #[inline]
    pub(super) fn index(&self, key: SearchKey) -> Option<usize> {
        if self.direction_sensitive && key.2 == NO_INCOMING_DIR {
            return (key.0 == self.start.0 && key.1 == self.start.1)
                .then(|| self.width as usize * self.height as usize * 8);
        }
        if key.0 < 0 || key.1 < 0 || key.0 as u32 >= self.width || key.1 as u32 >= self.height {
            return None;
        }
        let tile = key.1 as usize * self.width as usize + key.0 as usize;
        if self.direction_sensitive {
            (key.2 < 8).then(|| tile * 8 + key.2 as usize)
        } else {
            (key.2 == NO_INCOMING_DIR).then_some(tile)
        }
    }

    #[inline]
    fn key(&self, index: usize) -> SearchKey {
        let tiles = self.width as usize * self.height as usize;
        if self.direction_sensitive && index == tiles * 8 {
            return (self.start.0, self.start.1, NO_INCOMING_DIR);
        }
        let (tile, dir) = if self.direction_sensitive {
            (index / 8, (index % 8) as u8)
        } else {
            (index, NO_INCOMING_DIR)
        };
        (
            (tile % self.width as usize) as i32,
            (tile / self.width as usize) as i32,
            dir,
        )
    }

    #[inline]
    pub(super) fn get_g(&self, key: SearchKey) -> Option<u32> {
        self.index(key).and_then(|index| self.state().get_g(index))
    }

    #[inline]
    pub(super) fn set(&mut self, key: SearchKey, g: u32, parent: SearchKey) {
        if let (Some(index), Some(parent)) = (self.index(key), self.index(parent)) {
            self.state_mut().set(index, g, parent);
        }
    }

    pub(super) fn set_start(&mut self, index: usize) {
        self.state_mut().set(index, 0, index);
        self.state_mut().parent[index] = u32::MAX;
    }

    pub(super) fn reconstruct(&self, goal: SearchKey) -> Vec<(i32, i32)> {
        let Some(mut current) = self.index(goal) else {
            return Vec::new();
        };
        let mut path = vec![(goal.0, goal.1)];
        while let Some(previous) = self.state().parent(current) {
            let key = self.key(previous);
            path.push((key.0, key.1));
            current = previous;
        }
        path.pop();
        path.reverse();
        path
    }

    #[cfg(test)]
    pub(in crate::game) fn retained_capacity(&self) -> usize {
        self.ordinary.retained_bytes() + self.directional.retained_bytes()
    }

    #[cfg(test)]
    pub(super) fn force_generation_wrap(&mut self, direction_sensitive: bool) {
        if direction_sensitive {
            self.directional.generation = u32::MAX;
        } else {
            self.ordinary.generation = u32::MAX;
        }
    }

    #[cfg(test)]
    pub(super) fn generation(&self, direction_sensitive: bool) -> u32 {
        if direction_sensitive {
            self.directional.generation
        } else {
            self.ordinary.generation
        }
    }
}

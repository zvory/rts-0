//! Portable replay randomness. Changes to this contract require a new checkpoint RNG identifier.

use rand::{Error, RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

/// Xoshiro256++ on every target, with the historical 64-bit SmallRng PCG32 seed expansion.
/// Do not substitute Xoshiro's `seed_from_u64`: that uses SplitMix64 and changes replays.
#[derive(Clone, Debug)]
pub struct PortableRng(Xoshiro256PlusPlus);

impl SeedableRng for PortableRng {
    type Seed = [u8; 32];

    fn from_seed(seed: Self::Seed) -> Self {
        Self(Xoshiro256PlusPlus::from_seed(seed))
    }

    fn seed_from_u64(mut state: u64) -> Self {
        let mut seed = [0; 32];
        for chunk in seed.chunks_exact_mut(4) {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(11_634_580_027_462_260_723);
            let word = (((state >> 18) ^ state) >> 27) as u32;
            chunk.copy_from_slice(&word.rotate_right((state >> 59) as u32).to_le_bytes());
        }
        Self::from_seed(seed)
    }
}

impl RngCore for PortableRng {
    fn next_u32(&mut self) -> u32 {
        // One whole Xoshiro transition; discard the low 32 bits.
        (self.0.next_u64() >> 32) as u32
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        // Pin full-word and tail consumption rather than inherit a target-dependent helper.
        let mut chunks = dest.chunks_exact_mut(8);
        for chunk in &mut chunks {
            chunk.copy_from_slice(&self.next_u64().to_le_bytes());
        }
        let tail = chunks.into_remainder();
        if tail.len() > 4 {
            tail.copy_from_slice(&self.next_u64().to_le_bytes()[..tail.len()]);
        } else if !tail.is_empty() {
            tail.copy_from_slice(&self.next_u32().to_le_bytes()[..tail.len()]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

//! The simulation's only source of randomness: ChaCha8 seeded once per run.
//! Every roll goes through [`SimRng`] in a deterministic order, so a seed replays a run exactly.

use bevy_ecs::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

#[derive(Resource, Debug, Clone)]
pub struct SimRng {
    seed: u64,
    rng: ChaCha8Rng,
}

impl SimRng {
    pub fn new(seed: u64) -> Self {
        Self { seed, rng: ChaCha8Rng::seed_from_u64(seed) }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// How many 32-bit words have been drawn (part of the state hash).
    pub fn word_pos(&self) -> u128 {
        self.rng.get_word_pos()
    }

    pub fn next_u32(&mut self) -> u32 {
        self.rng.next_u32()
    }

    pub fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }

    /// Uniform in [0, 1), 24 bits of precision.
    pub fn unit_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Uniform in [0, 1), 53 bits of precision.
    pub fn unit_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// True with probability `p` (clamped to [0, 1]).
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit_f64() < p.clamp(0.0, 1.0)
    }

    /// Uniform integer in `lo..hi` (`hi > lo`), without modulo bias.
    pub fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        assert!(hi > lo, "empty range {lo}..{hi}");
        let span = hi - lo;
        let zone = u32::MAX - (u32::MAX - span + 1) % span;
        loop {
            let v = self.next_u32();
            if v <= zone {
                return lo + v % span;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let (mut a, mut b) = (SimRng::new(7), SimRng::new(7));
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(SimRng::new(7).next_u64(), SimRng::new(8).next_u64());
    }

    #[test]
    fn helpers_stay_in_range() {
        let mut r = SimRng::new(1);
        let mut hits = 0;
        for _ in 0..100_000 {
            let u = r.unit_f32();
            assert!((0.0..1.0).contains(&u));
            let v = r.range_u32(3, 10);
            assert!((3..10).contains(&v));
            hits += u32::from(r.chance(0.25));
        }
        let rate = f64::from(hits) / 100_000.0;
        assert!((rate - 0.25).abs() < 0.005, "chance(0.25) gave {rate}");
    }
}

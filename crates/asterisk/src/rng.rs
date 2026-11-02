//! A tiny, portable, seeded generator: `SplitMix64`.
//!
//! Chosen because it is four lines in any language, so the JavaScript port in
//! the artifact draws the same numbers.

use serde::{Deserialize, Serialize};

/// A seed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Seed(pub u64);

impl Seed {
    /// A seed from a word: FNV-1a over its UTF-8 bytes.
    pub fn from_word(word: &str) -> Self {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in word.trim().to_lowercase().bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        Self(h)
    }
}

/// `SplitMix64`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng(u64);

impl Rng {
    /// Seeded.
    #[must_use]
    pub const fn new(seed: Seed) -> Self {
        Self(seed.0)
    }

    /// The next 64 bits.
    pub const fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform-ish in `0..n` (modulo bias below 2⁻³² for the sizes used here).
    /// `n == 0` gives 0.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        let n64 = n as u64;
        // The remainder is < n, which came from a usize.
        usize::try_from(self.next_u64() % n64).unwrap_or(0)
    }

    /// True with probability `per_mille / 1000`.
    pub fn chance(&mut self, per_mille: u32) -> bool {
        self.next_u64() % 1000 < u64::from(per_mille)
    }

    /// One element.
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> Option<&'a T> {
        if xs.is_empty() {
            None
        } else {
            xs.get(self.below(xs.len()))
        }
    }

    /// Index chosen in proportion to `weights`.
    pub fn weighted(&mut self, weights: &[u32]) -> usize {
        let total: u64 = weights.iter().map(|&w| u64::from(w)).sum();
        if total == 0 {
            return 0;
        }
        let mut x = self.next_u64() % total;
        for (i, &w) in weights.iter().enumerate() {
            let w = u64::from(w);
            if x < w {
                return i;
            }
            x -= w;
        }
        weights.len().saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_reference_values() {
        // SplitMix64 seeded with 0: the published first outputs.
        let mut r = Rng::new(Seed(0));
        assert_eq!(r.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(r.next_u64(), 0x6e78_9e6a_a1b9_65f4);
    }

    #[test]
    fn words_are_seeds() {
        assert_eq!(Seed::from_word("Moon"), Seed::from_word(" moon "));
        assert_ne!(Seed::from_word("moon"), Seed::from_word("sun"));
    }

    #[test]
    fn below_stays_below() {
        let mut r = Rng::new(Seed(7));
        for n in 1..50 {
            assert!(r.below(n) < n);
        }
        assert_eq!(r.below(0), 0);
    }

    #[test]
    fn weighted_respects_zero_weights() {
        let mut r = Rng::new(Seed(3));
        for _ in 0..200 {
            assert_eq!(r.weighted(&[0, 5, 0]), 1);
        }
    }
}

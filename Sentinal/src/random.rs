use sha2::{Digest, Sha256};

/// Deterministic pseudo-random number generator using xorshift64.
///
/// NOT cryptographically secure — used only for deterministic shuffles
/// in obfuscation (Fisher-Yates). For cryptographic randomness, use
/// `getrandom` or the functions in `key_derivation`.
#[derive(Debug, Clone)]
pub struct SeededRandom {
    state: u64,
}

impl SeededRandom {
    /// Create a new SeededRandom from arbitrary seed bytes.
    ///
    /// The seed is SHA-256 hashed, and the first 8 bytes are used as
    /// the initial u64 state (little-endian). If the resulting state
    /// is 0, it is set to 1 (xorshift requires non-zero state).
    pub fn new(seed: &[u8]) -> Self {
        let hash = Sha256::digest(seed);
        let bytes: [u8; 8] = hash[..8].try_into().expect("SHA-256 produces 32 bytes");
        let mut state = u64::from_le_bytes(bytes);
        if state == 0 {
            state = 1;
        }
        Self { state }
    }

    /// Generate the next random value (non-negative).
    ///
    /// Uses xorshift64 algorithm. Returns a value in the positive
    /// range `0..=i64::MAX` (masks off the sign bit).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> usize {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state & 0x7FFFFFFFFFFFFFFF) as usize
    }

    /// Generate a random value in `0..bound`.
    ///
    /// Uses rejection sampling to eliminate modulo bias. The threshold
    /// `(2^63 - bound) % bound` ensures every value in `0..bound` has
    /// equal probability. (We use 2^63 because `next()` masks to the
    /// positive range `0..=i64::MAX`.)
    pub fn next_bounded(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        // threshold = (2^63 - bound) % bound, computed via wrapping_neg on the
        // positive-range maximum. Values below the threshold are rejected to
        // remove modulo bias.
        let range_max = 0x7FFF_FFFF_FFFF_FFFFusize; // i64::MAX as usize, matches next()'s mask
        let usable = range_max - (range_max % bound); // largest multiple of bound <= range_max
        loop {
            let r = self.next();
            if r < usable {
                return r % bound;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_output() {
        let mut rng1 = SeededRandom::new(b"test-seed");
        let mut rng2 = SeededRandom::new(b"test-seed");
        for _ in 0..100 {
            assert_eq!(rng1.next(), rng2.next());
        }
    }

    #[test]
    fn different_seeds_different_output() {
        let mut rng1 = SeededRandom::new(b"seed-a");
        let mut rng2 = SeededRandom::new(b"seed-b");
        // With overwhelmingly high probability, at least one of the
        // first 10 outputs will differ.
        let different = (0..10).any(|_| rng1.next() != rng2.next());
        assert!(different);
    }

    #[test]
    fn bounded_range() {
        let mut rng = SeededRandom::new(b"bounded");
        for _ in 0..1000 {
            let val = rng.next_bounded(10);
            assert!(val < 10);
        }
    }

    #[test]
    fn zero_seed_produces_nonzero_state() {
        // All-zero input should still work (state forced to 1).
        let mut rng = SeededRandom::new(&[0u8; 32]);
        let val = rng.next();
        assert!(val > 0);
    }
}

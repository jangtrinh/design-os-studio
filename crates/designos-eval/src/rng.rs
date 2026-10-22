//! Deterministic SplitMix64 and xoshiro256** PRNG seeded from SHA-1 (E15-25 §3.9).

use crate::hash::sha1_bytes;

/// SplitMix64 state expansion generator used to initialize [`Xoshiro256StarStar`].
#[derive(Clone, Debug)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Deterministic `xoshiro256**` pseudo-random number generator (Blackman & Vigna).
#[derive(Clone, Debug)]
pub struct Xoshiro256StarStar {
    s: [u64; 4],
}

impl Xoshiro256StarStar {
    /// Seeds `xoshiro256**` from a 64-bit integer via [`SplitMix64`].
    pub fn seed_from_u64(seed: u64) -> Self {
        let mut sm = SplitMix64::new(seed);
        Self { s: [sm.next_u64(), sm.next_u64(), sm.next_u64(), sm.next_u64()] }
    }

    /// Generates the next 64-bit word.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;

        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];

        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);

        result
    }

    /// Draws an index in `0..upper` deterministically.
    pub fn gen_index(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        (self.next_u64() % (upper as u64)) as usize
    }
}

/// Computes the 64-bit seed from the first 8 big-endian bytes of `sha1(release || suite || metric)`.
pub fn seed_for_metric(release: &str, suite: &str, metric: &str) -> u64 {
    let mut input = Vec::with_capacity(release.len() + suite.len() + metric.len());
    input.extend_from_slice(release.as_bytes());
    input.extend_from_slice(suite.as_bytes());
    input.extend_from_slice(metric.as_bytes());
    let digest = sha1_bytes(&input);
    u64::from_be_bytes([digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7]])
}

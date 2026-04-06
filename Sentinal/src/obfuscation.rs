use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::random::SeededRandom;

type HmacSha256 = Hmac<Sha256>;

/// Obfuscate binary data by XOR-ing with an HMAC-SHA256 keystream.
///
/// The keystream is deterministic: same seed + same data length always
/// produces the same keystream. This is NOT encryption — it's a
/// defense-in-depth layer for protecting data in memory after decryption.
pub fn obfuscate(data: &[u8], seed: &[u8]) -> Vec<u8> {
    xor_with_keystream(data, seed)
}

/// Deobfuscate binary data. XOR is self-reversing, so this is identical
/// to `obfuscate`.
pub fn deobfuscate(data: &[u8], seed: &[u8]) -> Vec<u8> {
    xor_with_keystream(data, seed)
}

/// Scramble pixel color values using a seed-deterministic byte permutation.
///
/// Generates a Fisher-Yates shuffle of 0..255 using the seed, then maps
/// every byte through the permutation. Red becomes purple, blue becomes
/// green, etc. The scrambling is deterministic and reversible.
pub fn scramble_colors(pixel_data: &[u8], seed: &[u8]) -> Vec<u8> {
    let color_map = generate_color_map(seed);
    pixel_data.iter().map(|&b| color_map[b as usize]).collect()
}

/// Unscramble pixel colors using the reverse permutation.
pub fn unscramble_colors(pixel_data: &[u8], seed: &[u8]) -> Vec<u8> {
    let reverse_map = generate_reverse_color_map(seed);
    pixel_data.iter().map(|&b| reverse_map[b as usize]).collect()
}

/// Generate a Fisher-Yates pixel position shuffle pattern.
///
/// Given N pixels, returns a permutation `[0..N)` determining where
/// each pixel should be moved. The pattern is deterministic from
/// `seed + idea_id`, so each .idea gets a unique shuffle.
pub fn generate_shuffle_pattern(pixel_count: usize, seed: &[u8], idea_id: &Uuid) -> Vec<usize> {
    let mut rng = seeded_rng_for_idea(seed, idea_id);
    let mut pattern: Vec<usize> = (0..pixel_count).collect();
    // Fisher-Yates shuffle.
    for i in (1..pixel_count).rev() {
        let j = rng.next_bounded(i + 1);
        pattern.swap(i, j);
    }
    pattern
}

/// Generate the reverse shuffle pattern for restoring pixel positions.
pub fn generate_reverse_shuffle_pattern(
    pixel_count: usize,
    seed: &[u8],
    idea_id: &Uuid,
) -> Vec<usize> {
    let forward = generate_shuffle_pattern(pixel_count, seed, idea_id);
    let mut reverse = vec![0usize; pixel_count];
    for (original, &shuffled) in forward.iter().enumerate() {
        reverse[shuffled] = original;
    }
    reverse
}

// --- Internal ---

fn xor_with_keystream(data: &[u8], seed: &[u8]) -> Vec<u8> {
    let keystream = generate_keystream(seed, data.len());
    data.iter()
        .zip(keystream.iter())
        .map(|(&d, &k)| d ^ k)
        .collect()
}

/// Generate a keystream using HMAC-SHA256 in counter mode.
///
/// Each block: `HMAC-SHA256(counter_be32, key=seed)` produces 32 bytes.
/// Blocks are concatenated and truncated to the requested length.
fn generate_keystream(seed: &[u8], length: usize) -> Vec<u8> {
    let mut keystream = Vec::with_capacity(length);
    let mut counter: u32 = 0;

    while keystream.len() < length {
        let mut mac =
            HmacSha256::new_from_slice(seed).expect("HMAC accepts any key length");
        mac.update(&counter.to_be_bytes());
        let block = mac.finalize().into_bytes();
        keystream.extend_from_slice(&block);
        counter += 1;
    }

    keystream.truncate(length);
    keystream
}

fn generate_color_map(seed: &[u8]) -> [u8; 256] {
    let mut map: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut rng = SeededRandom::new(seed);

    // Fisher-Yates shuffle.
    for i in (1..256).rev() {
        let j = rng.next_bounded(i + 1);
        map.swap(i, j);
    }

    map
}

fn generate_reverse_color_map(seed: &[u8]) -> [u8; 256] {
    let forward = generate_color_map(seed);
    let mut reverse = [0u8; 256];
    for (original, &mapped) in forward.iter().enumerate() {
        reverse[mapped as usize] = original as u8;
    }
    reverse
}

fn seeded_rng_for_idea(seed: &[u8], idea_id: &Uuid) -> SeededRandom {
    let mut combined = Vec::with_capacity(seed.len() + 36);
    combined.extend_from_slice(seed);
    combined.extend_from_slice(idea_id.to_string().as_bytes());
    SeededRandom::new(&combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obfuscate_deobfuscate_round_trip() {
        let seed = b"obfuscation-seed";
        let data = b"Doctor appointment Tuesday";
        let obfuscated = obfuscate(data, seed);
        assert_ne!(obfuscated, data);
        let restored = deobfuscate(&obfuscated, seed);
        assert_eq!(restored, data);
    }

    #[test]
    fn obfuscate_deterministic() {
        let seed = b"deterministic";
        let data = b"same input same output";
        let a = obfuscate(data, seed);
        let b = obfuscate(data, seed);
        assert_eq!(a, b);
    }

    #[test]
    fn obfuscate_different_seeds() {
        let data = b"secret data";
        let a = obfuscate(data, b"seed-1");
        let b = obfuscate(data, b"seed-2");
        assert_ne!(a, b);
    }

    #[test]
    fn obfuscate_empty_data() {
        let result = obfuscate(b"", b"seed");
        assert!(result.is_empty());
    }

    #[test]
    fn color_scramble_unscramble_round_trip() {
        let seed = b"color-seed";
        // Simulate a small "image" of pixel bytes.
        let pixels: Vec<u8> = (0..=255).collect();
        let scrambled = scramble_colors(&pixels, seed);
        assert_ne!(scrambled, pixels);
        let restored = unscramble_colors(&scrambled, seed);
        assert_eq!(restored, pixels);
    }

    #[test]
    fn color_map_is_permutation() {
        let map = generate_color_map(b"perm-test");
        let mut sorted = map.to_vec();
        sorted.sort();
        let expected: Vec<u8> = (0..=255).collect();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn shuffle_pattern_reverse_round_trip() {
        let seed = b"shuffle-seed";
        let idea_id = Uuid::new_v4();
        let pixel_count = 100;

        let forward = generate_shuffle_pattern(pixel_count, seed, &idea_id);
        let reverse = generate_reverse_shuffle_pattern(pixel_count, seed, &idea_id);

        // Apply forward then reverse: should get original order.
        let mut shuffled = vec![0usize; pixel_count];
        for (i, &dest) in forward.iter().enumerate() {
            shuffled[dest] = i;
        }
        let mut restored = vec![0usize; pixel_count];
        for (i, &dest) in reverse.iter().enumerate() {
            restored[dest] = shuffled[i]; // reversed to original position
        }

        // Verify: applying forward[i] then reverse on the result gives identity.
        for i in 0..pixel_count {
            assert_eq!(reverse[forward[i]], i);
        }
    }

    #[test]
    fn shuffle_pattern_deterministic() {
        let seed = b"det-shuffle";
        let idea_id = Uuid::new_v4();
        let a = generate_shuffle_pattern(50, seed, &idea_id);
        let b = generate_shuffle_pattern(50, seed, &idea_id);
        assert_eq!(a, b);
    }

    #[test]
    fn shuffle_pattern_different_ideas() {
        let seed = b"same-seed";
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let a = generate_shuffle_pattern(50, seed, &id1);
        let b = generate_shuffle_pattern(50, seed, &id2);
        assert_ne!(a, b);
    }
}

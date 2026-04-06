//! PKCS#7-style block padding for fixed-size alignment.
//!
//! Pads data to a multiple of a given block size, making it impossible
//! to determine the original data length from the padded output. This is
//! the low-level primitive — Globe's privacy module builds bucket-based
//! padding on top of it.
//!
//! # Example
//!
//! ```
//! use sentinal::padding::{pad_to_multiple, unpad_from_multiple};
//!
//! let data = b"hello";
//! let padded = pad_to_multiple(data, 16);
//! assert_eq!(padded.len() % 16, 0);
//!
//! let recovered = unpad_from_multiple(&padded).unwrap();
//! assert_eq!(recovered, data);
//! ```

use crate::error::SentinalError;

/// Pad `data` to the next multiple of `block_size` using PKCS#7 padding.
///
/// Each padding byte contains the number of padding bytes added (1..=block_size).
/// If the data is already aligned, a full block of padding is appended so that
/// unpadding is always unambiguous.
///
/// # Panics
///
/// Panics if `block_size` is 0 or greater than 255 (PKCS#7 limit).
#[must_use]
pub fn pad_to_multiple(data: &[u8], block_size: usize) -> Vec<u8> {
    assert!(block_size > 0 && block_size <= 255, "block_size must be 1..=255");

    let pad_len = block_size - (data.len() % block_size);
    let mut out = Vec::with_capacity(data.len() + pad_len);
    out.extend_from_slice(data);
    out.extend(std::iter::repeat_n(pad_len as u8, pad_len));
    out
}

/// Remove PKCS#7 padding and recover the original data.
///
/// Returns an error if the padding is invalid (zero pad byte, inconsistent
/// pad bytes, or pad length exceeds the data length).
pub fn unpad_from_multiple(data: &[u8]) -> Result<Vec<u8>, SentinalError> {
    if data.is_empty() {
        return Err(SentinalError::InvalidPadding(
            "padded data is empty".into(),
        ));
    }

    let pad_byte = *data.last().expect("checked non-empty above");
    let pad_len = pad_byte as usize;

    if pad_len == 0 || pad_len > data.len() {
        return Err(SentinalError::InvalidPadding(format!(
            "invalid pad byte {pad_byte} for data of length {}",
            data.len()
        )));
    }

    // Verify all padding bytes are consistent.
    let start = data.len() - pad_len;
    for &b in &data[start..] {
        if b != pad_byte {
            return Err(SentinalError::InvalidPadding(
                "inconsistent padding bytes".into(),
            ));
        }
    }

    Ok(data[..start].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic() {
        let data = b"hello";
        let padded = pad_to_multiple(data, 16);
        assert_eq!(padded.len(), 16);
        let recovered = unpad_from_multiple(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_aligned_data_gets_full_block() {
        // 16 bytes of data with block_size=16 -> 32 bytes (full extra block)
        let data = [0xAA_u8; 16];
        let padded = pad_to_multiple(&data, 16);
        assert_eq!(padded.len(), 32);
        let recovered = unpad_from_multiple(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_empty_data() {
        let padded = pad_to_multiple(b"", 8);
        assert_eq!(padded.len(), 8);
        assert!(padded.iter().all(|&b| b == 8));
        let recovered = unpad_from_multiple(&padded).unwrap();
        assert!(recovered.is_empty());
    }

    #[test]
    fn roundtrip_single_byte_block() {
        let data = b"abc";
        let padded = pad_to_multiple(data, 1);
        // block_size=1: data is always aligned, so 1 full block of padding appended
        assert_eq!(padded.len(), 4);
        let recovered = unpad_from_multiple(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_various_sizes() {
        for block in [1_usize, 4, 8, 16, 32, 64, 128, 255] {
            for len in 0..block * 3 {
                let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
                let padded = pad_to_multiple(&data, block);
                assert_eq!(padded.len() % block, 0, "block={block} len={len}");
                let recovered = unpad_from_multiple(&padded).unwrap();
                assert_eq!(recovered, data, "block={block} len={len}");
            }
        }
    }

    #[test]
    fn unpad_empty_fails() {
        assert!(unpad_from_multiple(b"").is_err());
    }

    #[test]
    fn unpad_bad_pad_byte_zero_fails() {
        let data = vec![1, 2, 3, 0];
        assert!(unpad_from_multiple(&data).is_err());
    }

    #[test]
    fn unpad_inconsistent_padding_fails() {
        let data = vec![0xAA, 3, 2, 3]; // last 3 bytes should all be 3
        assert!(unpad_from_multiple(&data).is_err());
    }

    #[test]
    fn unpad_pad_byte_exceeds_length_fails() {
        let data = vec![5]; // pad_byte=5 but data len=1
        assert!(unpad_from_multiple(&data).is_err());
    }

    #[test]
    #[should_panic]
    fn block_size_zero_panics() {
        let _ = pad_to_multiple(b"data", 0);
    }

    #[test]
    #[should_panic]
    fn block_size_over_255_panics() {
        let _ = pad_to_multiple(b"data", 256);
    }
}

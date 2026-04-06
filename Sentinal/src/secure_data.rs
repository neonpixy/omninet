use std::fmt;

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::SentinalError;

/// A memory-safe container for sensitive data that zeros its contents on drop.
///
/// - All data is zeroed when the container is dropped (via `zeroize`).
/// - Debug output never exposes the contents.
/// - Equality comparison is constant-time (prevents timing attacks).
/// - Access requires calling `expose()` explicitly — no accidental leaks.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecureData {
    data: Vec<u8>,
}

impl SecureData {
    /// Create a SecureData from raw bytes. The input vector is consumed.
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Create a SecureData from a byte slice (copies the data).
    pub fn from_slice(bytes: &[u8]) -> Self {
        Self {
            data: bytes.to_vec(),
        }
    }

    /// Create a SecureData filled with cryptographically random bytes.
    pub fn random(length: usize) -> Result<Self, SentinalError> {
        let mut data = vec![0u8; length];
        getrandom::fill(&mut data).map_err(|e| {
            SentinalError::RandomGenerationFailed(format!("getrandom failed: {e}"))
        })?;
        Ok(Self { data })
    }

    /// Explicitly access the inner bytes.
    ///
    /// Every call site becomes auditable — you can grep for `expose()`
    /// to find every place sensitive data is accessed.
    pub fn expose(&self) -> &[u8] {
        &self.data
    }

    /// The length of the contained data in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the container is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl PartialEq for SecureData {
    /// Constant-time equality comparison. Prevents timing side-channel attacks.
    fn eq(&self, other: &Self) -> bool {
        if self.data.len() != other.data.len() {
            return false;
        }
        self.data.ct_eq(&other.data).into()
    }
}

impl Eq for SecureData {}

impl fmt::Debug for SecureData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecureData([REDACTED; {} bytes])", self.data.len())
    }
}

impl fmt::Display for SecureData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecureData([REDACTED; {} bytes])", self.data.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_expose() {
        let data = SecureData::new(vec![1, 2, 3, 4]);
        assert_eq!(data.expose(), &[1, 2, 3, 4]);
        assert_eq!(data.len(), 4);
        assert!(!data.is_empty());
    }

    #[test]
    fn from_slice() {
        let data = SecureData::from_slice(&[10, 20, 30]);
        assert_eq!(data.expose(), &[10, 20, 30]);
    }

    #[test]
    fn random_generation() {
        let data = SecureData::random(32).unwrap();
        assert_eq!(data.len(), 32);
        // Random data should not be all zeros (with overwhelming probability).
        assert!(data.expose().iter().any(|&b| b != 0));
    }

    #[test]
    fn constant_time_equality() {
        let a = SecureData::new(vec![1, 2, 3]);
        let b = SecureData::new(vec![1, 2, 3]);
        let c = SecureData::new(vec![1, 2, 4]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn different_lengths_not_equal() {
        let a = SecureData::new(vec![1, 2, 3]);
        let b = SecureData::new(vec![1, 2]);
        assert_ne!(a, b);
    }

    #[test]
    fn debug_redacted() {
        let data = SecureData::new(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let debug = format!("{data:?}");
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("DE"));
        assert!(!debug.contains("DEAD"));
    }
}

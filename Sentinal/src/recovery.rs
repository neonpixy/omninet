use bip39::Mnemonic;

use crate::error::SentinalError;
use crate::secure_data::SecureData;

/// Generate a 24-word BIP-39 recovery phrase from 32 bytes of entropy.
///
/// The phrase can reconstruct a master key via `phrase_to_seed`.
pub fn generate_phrase() -> Result<Vec<String>, SentinalError> {
    let mut entropy = [0u8; 32];
    getrandom::fill(&mut entropy).map_err(|e| {
        SentinalError::RandomGenerationFailed(format!("entropy generation: {e}"))
    })?;

    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| SentinalError::InvalidRecoveryPhrase(format!("mnemonic creation: {e}")))?;

    Ok(mnemonic.words().map(String::from).collect())
}

/// Validate a recovery phrase (checksum verification).
pub fn validate_phrase(words: &[String]) -> bool {
    let phrase = words.join(" ");
    Mnemonic::parse_normalized(&phrase).is_ok()
}

/// Derive a 64-byte seed from a recovery phrase and optional passphrase.
///
/// Uses PBKDF2-HMAC-SHA512 per BIP-39 spec (2048 iterations,
/// salt = "mnemonic" + passphrase).
pub fn phrase_to_seed(words: &[String], passphrase: &str) -> Result<SecureData, SentinalError> {
    let phrase = words.join(" ");
    let mnemonic = Mnemonic::parse_normalized(&phrase)
        .map_err(|e| SentinalError::InvalidRecoveryPhrase(format!("parse failed: {e}")))?;

    let seed = mnemonic.to_seed(passphrase);
    Ok(SecureData::new(seed.to_vec()))
}

/// Get the BIP-39 English wordlist (2048 words).
pub fn wordlist() -> Vec<&'static str> {
    bip39::Language::English.word_list().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_validate() {
        let phrase = generate_phrase().unwrap();
        assert_eq!(phrase.len(), 24);
        assert!(validate_phrase(&phrase));
    }

    #[test]
    fn invalid_phrase() {
        let bad = vec!["not".to_string(); 24];
        assert!(!validate_phrase(&bad));
    }

    #[test]
    fn phrase_to_seed_produces_64_bytes() {
        let phrase = generate_phrase().unwrap();
        let seed = phrase_to_seed(&phrase, "").unwrap();
        assert_eq!(seed.len(), 64);
    }

    #[test]
    fn phrase_to_seed_deterministic() {
        let phrase = generate_phrase().unwrap();
        let seed1 = phrase_to_seed(&phrase, "passphrase").unwrap();
        let seed2 = phrase_to_seed(&phrase, "passphrase").unwrap();
        assert_eq!(seed1, seed2);
    }

    #[test]
    fn different_passphrase_different_seed() {
        let phrase = generate_phrase().unwrap();
        let seed1 = phrase_to_seed(&phrase, "alpha").unwrap();
        let seed2 = phrase_to_seed(&phrase, "beta").unwrap();
        assert_ne!(seed1, seed2);
    }

    #[test]
    fn wordlist_has_2048_entries() {
        assert_eq!(wordlist().len(), 2048);
    }
}

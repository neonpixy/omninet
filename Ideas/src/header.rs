use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::IdeasError;

/// The Header.json file — the only unencrypted file in a .idea package.
///
/// Contains metadata needed to identify, decrypt, and verify the idea.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub version: String,
    pub id: Uuid,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended_type: Option<String>,
    pub creator: Creator,
    pub content: ContentMetadata,
    pub encryption: EncryptionConfig,
    pub babel: BabelConfig,
}

/// Information about the idea's creator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Creator {
    pub public_key: String,
    pub signature: String,
}

/// Metadata about the encrypted content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentMetadata {
    pub root_digit_id: Uuid,
    pub digit_count: u32,
    #[serde(default)]
    pub types: Vec<String>,
}

/// Encryption configuration with key slots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub algorithm: String,
    pub key_slots: Vec<KeySlot>,
}

/// A key slot that can unlock an idea's content key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum KeySlot {
    /// Unlocked with a user-supplied password.
    Password(PasswordKeySlot),
    /// Unlocked with a recipient's private key (ECDH).
    PublicKey(PublicKeySlot),
    /// Unlocked via an internal/system key reference.
    Internal(InternalKeySlot),
}

/// A password-derived key slot. The content key is wrapped using a key
/// derived from a user-supplied password via the salt and nonce.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasswordKeySlot {
    pub salt: String,
    pub nonce: String,
    pub wrapped_key: String,
}

/// A public-key key slot. The content key is wrapped for a specific
/// recipient using ephemeral key exchange (ECDH).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicKeySlot {
    /// Crown public key of the recipient who can unwrap this slot.
    pub recipient: String,
    /// Ephemeral public key used in the key exchange.
    pub ephemeral_public_key: String,
    pub wrapped_key: String,
}

/// An internal key slot, used for system-managed key references
/// (e.g., Vault-stored keys or device keys).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InternalKeySlot {
    /// Identifier for the key in the internal key store.
    pub key_id: String,
    pub wrapped_key: String,
}

/// Babel obfuscation configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BabelConfig {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vocabulary_seed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation_kit: Option<TranslationKitReference>,
}

/// A reference to a Babel translation kit distributed via gospel event.
///
/// Translation kits allow authorized recipients to de-obfuscate
/// Babel-protected content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslationKitReference {
    /// Whether the translation kit has been published.
    pub available: bool,
    /// The gospel event ID where the kit was distributed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gospel_event_id: Option<String>,
    /// Crown public keys of authorized recipients.
    #[serde(default)]
    pub recipients: Vec<String>,
}

impl Header {
    /// Creates a new header for a fresh idea.
    pub fn create(
        creator_public_key: String,
        signature: String,
        root_digit_id: Uuid,
        key_slot: KeySlot,
    ) -> Self {
        let now = Utc::now();
        Header {
            version: "1.0".to_string(),
            id: Uuid::new_v4(),
            created: now,
            modified: now,
            extended_type: None,
            creator: Creator {
                public_key: creator_public_key,
                signature,
            },
            content: ContentMetadata {
                root_digit_id,
                digit_count: 1,
                types: Vec::new(),
            },
            encryption: EncryptionConfig {
                algorithm: "AES-256-GCM".to_string(),
                key_slots: vec![key_slot],
            },
            babel: BabelConfig {
                enabled: false,
                vocabulary_seed: None,
                translation_kit: None,
            },
        }
    }

    /// Validates header structure.
    pub fn validate(&self) -> Result<(), IdeasError> {
        if self.version != "1.0" {
            return Err(IdeasError::UnsupportedVersion(self.version.clone()));
        }
        if self.encryption.key_slots.is_empty() {
            return Err(IdeasError::NoKeySlots);
        }
        if self.encryption.algorithm != "AES-256-GCM" {
            return Err(IdeasError::UnsupportedAlgorithm(
                self.encryption.algorithm.clone(),
            ));
        }
        if self.content.digit_count == 0 {
            return Err(IdeasError::InvalidDigitCount);
        }
        Ok(())
    }

    /// Updates the modified timestamp.
    pub fn touched(&self) -> Self {
        let mut copy = self.clone();
        copy.modified = Utc::now();
        copy
    }

    /// Updates content metadata.
    pub fn with_content(&self, content: ContentMetadata) -> Self {
        let mut copy = self.clone();
        copy.content = content;
        copy.modified = Utc::now();
        copy
    }

    /// Whether Babel obfuscation is enabled.
    pub fn is_babel_enabled(&self) -> bool {
        self.babel.enabled
    }

    /// Whether this idea can be unlocked with a password.
    pub fn has_password_slot(&self) -> bool {
        self.encryption
            .key_slots
            .iter()
            .any(|s| matches!(s, KeySlot::Password(_)))
    }

    /// Recipients who have public key access.
    pub fn shared_with(&self) -> Vec<&str> {
        self.encryption
            .key_slots
            .iter()
            .filter_map(|s| {
                if let KeySlot::PublicKey(pk) = s {
                    Some(pk.recipient.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// File extension for this idea.
    pub fn file_extension(&self) -> String {
        if let Some(ext) = &self.extended_type {
            format!(".idea.{ext}")
        } else {
            ".idea".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key_slot() -> KeySlot {
        KeySlot::Password(PasswordKeySlot {
            salt: "dGVzdHNhbHQ=".into(),
            nonce: "dGVzdG5vbmNl".into(),
            wrapped_key: "d3JhcHBlZA==".into(),
        })
    }

    #[test]
    fn create_header() {
        let root_id = Uuid::new_v4();
        let h = Header::create("cpub1test".into(), "sig_test".into(), root_id, test_key_slot());
        assert_eq!(h.version, "1.0");
        assert_eq!(h.creator.public_key, "cpub1test");
        assert_eq!(h.content.root_digit_id, root_id);
        assert_eq!(h.content.digit_count, 1);
        assert!(h.validate().is_ok());
    }

    #[test]
    fn validate_version() {
        let root_id = Uuid::new_v4();
        let mut h =
            Header::create("cpub1test".into(), "sig".into(), root_id, test_key_slot());
        h.version = "2.0".into();
        assert!(h.validate().is_err());
    }

    #[test]
    fn validate_no_key_slots() {
        let root_id = Uuid::new_v4();
        let mut h =
            Header::create("cpub1test".into(), "sig".into(), root_id, test_key_slot());
        h.encryption.key_slots.clear();
        assert!(h.validate().is_err());
    }

    #[test]
    fn validate_algorithm() {
        let root_id = Uuid::new_v4();
        let mut h =
            Header::create("cpub1test".into(), "sig".into(), root_id, test_key_slot());
        h.encryption.algorithm = "ROT13".into();
        assert!(h.validate().is_err());
    }

    #[test]
    fn key_slot_variants_serde() {
        let slots = vec![
            test_key_slot(),
            KeySlot::PublicKey(PublicKeySlot {
                recipient: "cpub1bob".into(),
                ephemeral_public_key: "ZXBo".into(),
                wrapped_key: "d3I=".into(),
            }),
            KeySlot::Internal(InternalKeySlot {
                key_id: "key-1".into(),
                wrapped_key: "d3I=".into(),
            }),
        ];
        for slot in &slots {
            let json = serde_json::to_string(slot).unwrap();
            let rt: KeySlot = serde_json::from_str(&json).unwrap();
            assert_eq!(&rt, slot);
        }
    }

    #[test]
    fn header_serde_round_trip() {
        let root_id = Uuid::new_v4();
        let h = Header::create("cpub1test".into(), "sig_test".into(), root_id, test_key_slot());
        let json = serde_json::to_string_pretty(&h).unwrap();
        let rt: Header = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.id, h.id);
        assert_eq!(rt.version, h.version);
        assert_eq!(rt.creator, h.creator);
        assert_eq!(rt.content.root_digit_id, h.content.root_digit_id);
    }

    #[test]
    fn shared_with() {
        let root_id = Uuid::new_v4();
        let mut h =
            Header::create("cpub1test".into(), "sig".into(), root_id, test_key_slot());
        h.encryption.key_slots.push(KeySlot::PublicKey(PublicKeySlot {
            recipient: "cpub1bob".into(),
            ephemeral_public_key: "ZXBo".into(),
            wrapped_key: "d3I=".into(),
        }));
        assert_eq!(h.shared_with(), vec!["cpub1bob"]);
        assert!(h.has_password_slot());
    }

    #[test]
    fn file_extension() {
        let root_id = Uuid::new_v4();
        let h = Header::create("cpub1test".into(), "sig".into(), root_id, test_key_slot());
        assert_eq!(h.file_extension(), ".idea");

        let mut h2 = h;
        h2.extended_type = Some("music".into());
        assert_eq!(h2.file_extension(), ".idea.music");
    }
}

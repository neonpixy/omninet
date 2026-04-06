use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Public-facing user profile information.
///
/// All fields except `language` and `updated_at` are optional -- a fresh
/// identity starts empty and fills in over time.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    /// Human-readable name shown on the profile.
    pub display_name: Option<String>,
    /// Unique handle (no enforcement here -- Jail validates uniqueness).
    pub username: Option<String>,
    /// Free-form biography text.
    pub bio: Option<String>,
    /// Profile picture. Can be inline data, an .idea asset, or an external URL.
    pub avatar: Option<AvatarReference>,
    /// Banner image displayed behind the profile.
    pub banner: Option<AvatarReference>,
    /// Personal website or homepage URL.
    pub website: Option<String>,
    /// BCP 47 language code (default `"en"`).
    pub language: String,
    /// Lightning address for tips (e.g., `user@example.com`).
    pub lightning_address: Option<String>,
    /// Verification address in `user@domain.com` format.
    pub verification_address: Option<String>,
    /// When this profile was last modified.
    pub updated_at: DateTime<Utc>,
}

impl Profile {
    /// Create an empty profile with defaults.
    pub fn empty() -> Self {
        Self {
            display_name: None,
            username: None,
            bio: None,
            avatar: None,
            banner: None,
            website: None,
            language: "en".into(),
            lightning_address: None,
            verification_address: None,
            updated_at: Utc::now(),
        }
    }
}

/// Reference to an avatar or banner image.
///
/// Serializes with a `"type"` discriminator field matching the quarry format:
/// ```json
/// {"type": "url", "url": "https://..."}
/// {"type": "asset", "idea_id": "uuid", "asset_name": "avatar.png"}
/// {"type": "data", "data": "base64...", "mime_type": "image/png"}
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum AvatarReference {
    /// Inline image data with MIME type.
    Data { bytes: Vec<u8>, mime_type: String },
    /// Reference to an asset in an .idea file.
    Asset { idea_id: Uuid, asset_name: String },
    /// External URL string.
    Url(String),
}

// Custom serde for tagged enum matching quarry JSON format.

impl Serialize for AvatarReference {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            AvatarReference::Data { bytes, mime_type } => {
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "data")?;
                map.serialize_entry("data", &encoded)?;
                map.serialize_entry("mime_type", mime_type)?;
                map.end()
            }
            AvatarReference::Asset {
                idea_id,
                asset_name,
            } => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "asset")?;
                map.serialize_entry("idea_id", idea_id)?;
                map.serialize_entry("asset_name", asset_name)?;
                map.end()
            }
            AvatarReference::Url(url) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "url")?;
                map.serialize_entry("url", url)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for AvatarReference {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;

        let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
        let obj = value.as_object().ok_or_else(|| D::Error::custom("expected object"))?;

        let type_str = obj
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| D::Error::custom("missing 'type' field"))?;

        match type_str {
            "data" => {
                use base64::Engine;
                let encoded = obj
                    .get("data")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| D::Error::custom("missing 'data' field"))?;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|e| D::Error::custom(format!("invalid base64: {e}")))?;
                let mime_type = obj
                    .get("mime_type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| D::Error::custom("missing 'mime_type' field"))?
                    .to_string();
                Ok(AvatarReference::Data { bytes, mime_type })
            }
            "asset" => {
                let idea_id: Uuid = obj
                    .get("idea_id")
                    .ok_or_else(|| D::Error::custom("missing 'idea_id'"))?
                    .as_str()
                    .ok_or_else(|| D::Error::custom("idea_id must be a string"))?
                    .parse()
                    .map_err(|e| D::Error::custom(format!("invalid uuid: {e}")))?;
                let asset_name = obj
                    .get("asset_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| D::Error::custom("missing 'asset_name'"))?
                    .to_string();
                Ok(AvatarReference::Asset {
                    idea_id,
                    asset_name,
                })
            }
            "url" => {
                let url = obj
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| D::Error::custom("missing 'url' field"))?
                    .to_string();
                Ok(AvatarReference::Url(url))
            }
            other => Err(D::Error::custom(format!("unknown avatar type: {other}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_profile_defaults() {
        let profile = Profile::empty();
        assert_eq!(profile.language, "en");
        assert!(profile.display_name.is_none());
        assert!(profile.username.is_none());
        assert!(profile.bio.is_none());
        assert!(profile.avatar.is_none());
        assert!(profile.website.is_none());
        assert!(profile.lightning_address.is_none());
        assert!(profile.verification_address.is_none());
    }

    #[test]
    fn profile_serde_round_trip() {
        let profile = Profile {
            display_name: Some("Sam".into()),
            username: Some("sam".into()),
            bio: Some("Builder".into()),
            avatar: Some(AvatarReference::Url("https://example.com/me.png".into())),
            banner: None,
            website: Some("https://omnidea.co".into()),
            language: "en".into(),
            lightning_address: Some("sam@example.com".into()),
            verification_address: Some("sam@example.com".into()),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&profile).unwrap();
        let loaded: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, loaded);
    }

    #[test]
    fn avatar_data_serde() {
        let avatar = AvatarReference::Data {
            bytes: vec![0xFF, 0xD8, 0xFF, 0xE0],
            mime_type: "image/jpeg".into(),
        };
        let json = serde_json::to_string(&avatar).unwrap();
        assert!(json.contains("\"type\":\"data\""));
        assert!(json.contains("\"mime_type\":\"image/jpeg\""));

        let loaded: AvatarReference = serde_json::from_str(&json).unwrap();
        assert_eq!(avatar, loaded);
    }

    #[test]
    fn avatar_asset_serde() {
        let id = Uuid::new_v4();
        let avatar = AvatarReference::Asset {
            idea_id: id,
            asset_name: "avatar.png".into(),
        };
        let json = serde_json::to_string(&avatar).unwrap();
        assert!(json.contains("\"type\":\"asset\""));
        assert!(json.contains("avatar.png"));

        let loaded: AvatarReference = serde_json::from_str(&json).unwrap();
        assert_eq!(avatar, loaded);
    }

    #[test]
    fn avatar_url_serde() {
        let avatar = AvatarReference::Url("https://example.com/pic.jpg".into());
        let json = serde_json::to_string(&avatar).unwrap();
        assert!(json.contains("\"type\":\"url\""));

        let loaded: AvatarReference = serde_json::from_str(&json).unwrap();
        assert_eq!(avatar, loaded);
    }

    #[test]
    fn avatar_unknown_type_fails() {
        let json = r#"{"type": "unknown", "data": "test"}"#;
        let result = serde_json::from_str::<AvatarReference>(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown"));
    }
}

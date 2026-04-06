//! App manifest types — the metadata that describes an application.
//!
//! An [`AppManifest`] is the canonical description of an app: its identity,
//! author, supported platforms, required permissions, and version history.

use chrono::{DateTime, Utc};
use crown::Signature;
use serde::{Deserialize, Serialize};

use crate::error::AppCatalogError;

/// A platform that an app can target.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Platform {
    /// macOS (Apple desktop).
    #[serde(rename = "macos")]
    MacOS,
    /// iOS (Apple mobile).
    #[serde(rename = "ios")]
    IOS,
    /// Android (Google mobile).
    #[serde(rename = "android")]
    Android,
    /// Linux (desktop/server).
    #[serde(rename = "linux")]
    Linux,
    /// Windows (Microsoft desktop).
    #[serde(rename = "windows")]
    Windows,
    /// GrapheneOS (privacy-focused Android).
    #[serde(rename = "grapheneos")]
    GrapheneOS,
    /// Web (browser-based).
    #[serde(rename = "web")]
    Web,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MacOS => write!(f, "macOS"),
            Self::IOS => write!(f, "iOS"),
            Self::Android => write!(f, "Android"),
            Self::Linux => write!(f, "Linux"),
            Self::Windows => write!(f, "Windows"),
            Self::GrapheneOS => write!(f, "GrapheneOS"),
            Self::Web => write!(f, "Web"),
        }
    }
}

/// A permission that an app can request.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Access to network resources.
    Network,
    /// Access to local storage.
    Storage,
    /// Access to the camera.
    Camera,
    /// Access to the microphone.
    Microphone,
    /// Access to location services.
    Location,
    /// Access to the user's contacts.
    Contacts,
    /// Ability to send notifications.
    Notifications,
    /// Ability to run in the background.
    BackgroundExecution,
    /// A custom permission not covered by the standard set.
    Custom(String),
}

/// The canonical metadata for an application.
///
/// Contains identity, authorship, platform support, permission requirements,
/// and full version history. Manifests are signed by the author's Crown
/// keypair and verified before admission to the catalog.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppManifest {
    /// Reverse-domain app identifier (e.g., `"com.omnidea.throne"`).
    pub id: String,
    /// Human-readable app name.
    pub name: String,
    /// Current version (semver string).
    pub version: String,
    /// Short description of what the app does.
    pub description: String,
    /// Author's Crown public key (bech32 crown_id).
    pub author_crown_id: String,
    /// Platforms this app supports.
    pub platforms: Vec<Platform>,
    /// Permissions this app requires.
    pub permissions: Vec<Permission>,
    /// All released versions, newest first by convention.
    pub versions: Vec<AppVersion>,
    /// When the manifest was first created.
    pub created_at: DateTime<Utc>,
    /// When the manifest was last updated.
    pub updated_at: DateTime<Utc>,
    /// Optional BIP-340 Schnorr signature by the author, covering the
    /// manifest's canonical JSON. Present when the manifest has been
    /// signed by its `author_crown_id`.
    pub signature: Option<Signature>,
}

impl AppManifest {
    /// Whether this manifest declares support for the given platform.
    pub fn supports_platform(&self, platform: &Platform) -> bool {
        self.platforms.contains(platform)
    }

    /// The latest version entry, determined by the most recent `released_at`.
    ///
    /// Returns `None` if the versions list is empty.
    pub fn latest_version(&self) -> Option<&AppVersion> {
        self.versions.iter().max_by_key(|v| v.released_at)
    }

    /// Verify the manifest's signature against its `author_crown_id`.
    ///
    /// The signature must cover the manifest's canonical signable bytes
    /// (id + name + version, concatenated). Returns `Ok(())` if the
    /// signature is valid, or an error if absent/invalid.
    ///
    /// # Errors
    ///
    /// - [`AppCatalogError::ManifestInvalid`] if no signature is present.
    /// - [`AppCatalogError::SignatureInvalid`] if the signature doesn't
    ///   match the author's public key.
    pub fn verify_signature(&self) -> Result<(), AppCatalogError> {
        let sig = self.signature.as_ref().ok_or_else(|| {
            AppCatalogError::ManifestInvalid("manifest has no signature".into())
        })?;

        let signable = self.signable_bytes();
        if sig.verify_crown_id(&signable, &self.author_crown_id) {
            Ok(())
        } else {
            Err(AppCatalogError::SignatureInvalid)
        }
    }

    /// Produce the canonical bytes that should be signed.
    ///
    /// Deterministic: `id || name || version` as UTF-8.
    #[must_use]
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.id.as_bytes());
        bytes.extend_from_slice(self.name.as_bytes());
        bytes.extend_from_slice(self.version.as_bytes());
        bytes
    }
}

/// A single released version of an application.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppVersion {
    /// Semver version string (e.g., `"1.2.3"`).
    pub version: String,
    /// Optional release notes describing changes.
    pub release_notes: Option<String>,
    /// SHA-256 hex digest of the binary artifact.
    pub content_hash: String,
    /// URL to download the binary (absent for store-only apps).
    pub download_url: Option<String>,
    /// Minimum OS version required (e.g., `"14.0"`).
    pub min_os_version: Option<String>,
    /// When this version was released.
    pub released_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_manifest() -> AppManifest {
        let now = Utc::now();
        AppManifest {
            id: "com.omnidea.throne".into(),
            name: "Throne".into(),
            version: "1.0.0".into(),
            description: "The sovereign creation platform".into(),
            author_crown_id: "cpub1testauthor".into(),
            platforms: vec![Platform::MacOS, Platform::IOS, Platform::Linux],
            permissions: vec![Permission::Network, Permission::Storage],
            versions: vec![
                AppVersion {
                    version: "0.9.0".into(),
                    release_notes: Some("Beta release".into()),
                    content_hash: "abc123".into(),
                    download_url: Some("https://example.com/throne-0.9.0".into()),
                    min_os_version: Some("13.0".into()),
                    released_at: now - chrono::Duration::days(30),
                },
                AppVersion {
                    version: "1.0.0".into(),
                    release_notes: Some("Initial release".into()),
                    content_hash: "def456".into(),
                    download_url: Some("https://example.com/throne-1.0.0".into()),
                    min_os_version: Some("14.0".into()),
                    released_at: now,
                },
            ],
            created_at: now - chrono::Duration::days(60),
            updated_at: now,
            signature: None,
        }
    }

    #[test]
    fn supports_platform_true() {
        let manifest = make_test_manifest();
        assert!(manifest.supports_platform(&Platform::MacOS));
        assert!(manifest.supports_platform(&Platform::IOS));
        assert!(manifest.supports_platform(&Platform::Linux));
    }

    #[test]
    fn supports_platform_false() {
        let manifest = make_test_manifest();
        assert!(!manifest.supports_platform(&Platform::Windows));
        assert!(!manifest.supports_platform(&Platform::Android));
        assert!(!manifest.supports_platform(&Platform::Web));
    }

    #[test]
    fn latest_version_returns_newest() {
        let manifest = make_test_manifest();
        let latest = manifest.latest_version().expect("should have versions");
        assert_eq!(latest.version, "1.0.0");
    }

    #[test]
    fn latest_version_empty() {
        let now = Utc::now();
        let manifest = AppManifest {
            id: "com.test.empty".into(),
            name: "Empty".into(),
            version: "0.0.0".into(),
            description: "No versions".into(),
            author_crown_id: "cpub1test".into(),
            platforms: vec![],
            permissions: vec![],
            versions: vec![],
            created_at: now,
            updated_at: now,
            signature: None,
        };
        assert!(manifest.latest_version().is_none());
    }

    #[test]
    fn platform_display() {
        assert_eq!(Platform::MacOS.to_string(), "macOS");
        assert_eq!(Platform::IOS.to_string(), "iOS");
        assert_eq!(Platform::Android.to_string(), "Android");
        assert_eq!(Platform::GrapheneOS.to_string(), "GrapheneOS");
        assert_eq!(Platform::Web.to_string(), "Web");
    }

    #[test]
    fn manifest_serde_round_trip() {
        let manifest = make_test_manifest();
        let json = serde_json::to_string(&manifest).expect("serialize");
        let loaded: AppManifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(manifest.id, loaded.id);
        assert_eq!(manifest.name, loaded.name);
        assert_eq!(manifest.version, loaded.version);
        assert_eq!(manifest.platforms.len(), loaded.platforms.len());
        assert_eq!(manifest.versions.len(), loaded.versions.len());
    }

    #[test]
    fn platform_serde_rename() {
        let json = serde_json::to_string(&Platform::MacOS).expect("serialize");
        assert_eq!(json, "\"macos\"");
        let json = serde_json::to_string(&Platform::GrapheneOS).expect("serialize");
        assert_eq!(json, "\"grapheneos\"");
    }

    #[test]
    fn permission_custom_variant() {
        let perm = Permission::Custom("bluetooth".into());
        let json = serde_json::to_string(&perm).expect("serialize");
        let loaded: Permission = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(perm, loaded);
    }

    #[test]
    fn verify_signature_no_signature() {
        let manifest = make_test_manifest();
        let err = manifest.verify_signature().unwrap_err();
        assert!(matches!(err, AppCatalogError::ManifestInvalid(_)));
    }

    #[test]
    fn verify_signature_valid() {
        let kp = crown::CrownKeypair::generate();
        let mut manifest = make_test_manifest();
        manifest.author_crown_id = kp.crown_id().to_string();

        let signable = manifest.signable_bytes();
        let sig = Signature::sign(&signable, &kp).expect("sign");
        manifest.signature = Some(sig);

        manifest.verify_signature().expect("should verify");
    }

    #[test]
    fn verify_signature_wrong_key() {
        let kp1 = crown::CrownKeypair::generate();
        let kp2 = crown::CrownKeypair::generate();

        let mut manifest = make_test_manifest();
        // Signed by kp1, but author_crown_id is kp2.
        manifest.author_crown_id = kp2.crown_id().to_string();

        let signable = manifest.signable_bytes();
        let sig = Signature::sign(&signable, &kp1).expect("sign");
        manifest.signature = Some(sig);

        let err = manifest.verify_signature().unwrap_err();
        assert_eq!(err, AppCatalogError::SignatureInvalid);
    }

    #[test]
    fn signable_bytes_deterministic() {
        let m1 = make_test_manifest();
        let m2 = make_test_manifest();
        assert_eq!(m1.signable_bytes(), m2.signable_bytes());
    }
}

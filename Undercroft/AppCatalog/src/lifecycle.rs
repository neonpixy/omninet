//! Lifecycle types — install, update, and uninstall request handling.
//!
//! These types model the *intent* to change an app's state. The actual
//! download, verification, and platform integration are the caller's
//! responsibility. [`resolve_install_action`] determines the platform-
//! appropriate strategy for a given install request.

use serde::{Deserialize, Serialize};

use crate::error::AppCatalogError;
use crate::manifest::{AppManifest, Platform};

/// A request to install an application.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallRequest {
    /// The app to install.
    pub app_id: String,
    /// The version to install.
    pub version: String,
    /// The target platform.
    pub platform: Platform,
}

/// A request to update an installed application.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateRequest {
    /// The app to update.
    pub app_id: String,
    /// The currently installed version.
    pub from_version: String,
    /// The target version.
    pub to_version: String,
}

/// A request to uninstall an application.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UninstallRequest {
    /// The app to uninstall.
    pub app_id: String,
}

/// The platform-appropriate installation strategy.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum InstallAction {
    /// Direct install on open platforms (macOS, Linux, Windows, GrapheneOS).
    Direct {
        /// URL to download the binary.
        download_url: String,
        /// SHA-256 hex digest for verification.
        content_hash: String,
    },
    /// Redirect to a platform app store (iOS, Android).
    StoreRedirect {
        /// URL of the store listing.
        store_url: String,
    },
    /// Web-based install.
    WebInstall {
        /// URL to the web application.
        url: String,
    },
}

/// Determine the platform-appropriate install strategy for a request.
///
/// Checks that the manifest supports the requested platform and that the
/// requested version exists, then returns the appropriate [`InstallAction`].
///
/// # Platform strategies
///
/// | Platform | Strategy |
/// |----------|----------|
/// | macOS, Linux, Windows, GrapheneOS | [`InstallAction::Direct`] |
/// | iOS | [`InstallAction::StoreRedirect`] (App Store) |
/// | Android | [`InstallAction::StoreRedirect`] (Play Store) |
/// | Web | [`InstallAction::WebInstall`] |
///
/// # Errors
///
/// - [`AppCatalogError::IncompatiblePlatform`] if the manifest doesn't
///   support the requested platform.
/// - [`AppCatalogError::VersionNotFound`] if the requested version doesn't
///   exist in the manifest.
/// - [`AppCatalogError::ManifestInvalid`] if a direct-install platform has
///   no download URL for the requested version.
pub fn resolve_install_action(
    request: &InstallRequest,
    manifest: &AppManifest,
) -> Result<InstallAction, AppCatalogError> {
    // Verify platform support.
    if !manifest.supports_platform(&request.platform) {
        return Err(AppCatalogError::IncompatiblePlatform(
            request.platform.to_string(),
        ));
    }

    // Find the requested version.
    let version_entry = manifest
        .versions
        .iter()
        .find(|v| v.version == request.version)
        .ok_or_else(|| AppCatalogError::VersionNotFound(request.version.clone()))?;

    match &request.platform {
        // Open platforms: direct download.
        Platform::MacOS | Platform::Linux | Platform::Windows | Platform::GrapheneOS => {
            let download_url = version_entry
                .download_url
                .as_ref()
                .ok_or_else(|| {
                    AppCatalogError::ManifestInvalid(format!(
                        "no download URL for version {} on {}",
                        request.version, request.platform,
                    ))
                })?
                .clone();

            Ok(InstallAction::Direct {
                download_url,
                content_hash: version_entry.content_hash.clone(),
            })
        }

        // iOS: redirect to App Store.
        Platform::IOS => {
            let store_url = format!(
                "https://apps.apple.com/app/{}",
                manifest.id.replace('.', "-")
            );
            Ok(InstallAction::StoreRedirect { store_url })
        }

        // Android: redirect to Play Store.
        Platform::Android => {
            let store_url = format!(
                "https://play.google.com/store/apps/details?id={}",
                manifest.id
            );
            Ok(InstallAction::StoreRedirect { store_url })
        }

        // Web: direct URL.
        Platform::Web => {
            let url = version_entry
                .download_url
                .as_ref()
                .ok_or_else(|| {
                    AppCatalogError::ManifestInvalid(format!(
                        "no URL for version {} on Web",
                        request.version,
                    ))
                })?
                .clone();

            Ok(InstallAction::WebInstall { url })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{AppVersion, Permission};
    use chrono::Utc;

    fn make_full_manifest() -> AppManifest {
        let now = Utc::now();
        AppManifest {
            id: "com.omnidea.throne".into(),
            name: "Throne".into(),
            version: "1.0.0".into(),
            description: "The sovereign creation platform".into(),
            author_crown_id: "cpub1testauthor".into(),
            platforms: vec![
                Platform::MacOS,
                Platform::IOS,
                Platform::Android,
                Platform::Linux,
                Platform::Windows,
                Platform::GrapheneOS,
                Platform::Web,
            ],
            permissions: vec![Permission::Network, Permission::Storage],
            versions: vec![AppVersion {
                version: "1.0.0".into(),
                release_notes: Some("Initial release".into()),
                content_hash: "sha256abcdef".into(),
                download_url: Some("https://example.com/throne-1.0.0".into()),
                min_os_version: Some("14.0".into()),
                released_at: now,
            }],
            created_at: now,
            updated_at: now,
            signature: None,
        }
    }

    #[test]
    fn resolve_macos_direct() {
        let manifest = make_full_manifest();
        let request = InstallRequest {
            app_id: "com.omnidea.throne".into(),
            version: "1.0.0".into(),
            platform: Platform::MacOS,
        };

        let action = resolve_install_action(&request, &manifest).unwrap();
        assert_eq!(
            action,
            InstallAction::Direct {
                download_url: "https://example.com/throne-1.0.0".into(),
                content_hash: "sha256abcdef".into(),
            }
        );
    }

    #[test]
    fn resolve_linux_direct() {
        let manifest = make_full_manifest();
        let request = InstallRequest {
            app_id: "com.omnidea.throne".into(),
            version: "1.0.0".into(),
            platform: Platform::Linux,
        };

        let action = resolve_install_action(&request, &manifest).unwrap();
        assert!(matches!(action, InstallAction::Direct { .. }));
    }

    #[test]
    fn resolve_windows_direct() {
        let manifest = make_full_manifest();
        let request = InstallRequest {
            app_id: "com.omnidea.throne".into(),
            version: "1.0.0".into(),
            platform: Platform::Windows,
        };

        let action = resolve_install_action(&request, &manifest).unwrap();
        assert!(matches!(action, InstallAction::Direct { .. }));
    }

    #[test]
    fn resolve_grapheneos_direct() {
        let manifest = make_full_manifest();
        let request = InstallRequest {
            app_id: "com.omnidea.throne".into(),
            version: "1.0.0".into(),
            platform: Platform::GrapheneOS,
        };

        let action = resolve_install_action(&request, &manifest).unwrap();
        assert!(matches!(action, InstallAction::Direct { .. }));
    }

    #[test]
    fn resolve_ios_store_redirect() {
        let manifest = make_full_manifest();
        let request = InstallRequest {
            app_id: "com.omnidea.throne".into(),
            version: "1.0.0".into(),
            platform: Platform::IOS,
        };

        let action = resolve_install_action(&request, &manifest).unwrap();
        match action {
            InstallAction::StoreRedirect { store_url } => {
                assert!(store_url.contains("apps.apple.com"));
            }
            _ => panic!("expected StoreRedirect for iOS"),
        }
    }

    #[test]
    fn resolve_android_store_redirect() {
        let manifest = make_full_manifest();
        let request = InstallRequest {
            app_id: "com.omnidea.throne".into(),
            version: "1.0.0".into(),
            platform: Platform::Android,
        };

        let action = resolve_install_action(&request, &manifest).unwrap();
        match action {
            InstallAction::StoreRedirect { store_url } => {
                assert!(store_url.contains("play.google.com"));
                assert!(store_url.contains("com.omnidea.throne"));
            }
            _ => panic!("expected StoreRedirect for Android"),
        }
    }

    #[test]
    fn resolve_web_install() {
        let manifest = make_full_manifest();
        let request = InstallRequest {
            app_id: "com.omnidea.throne".into(),
            version: "1.0.0".into(),
            platform: Platform::Web,
        };

        let action = resolve_install_action(&request, &manifest).unwrap();
        match action {
            InstallAction::WebInstall { url } => {
                assert_eq!(url, "https://example.com/throne-1.0.0");
            }
            _ => panic!("expected WebInstall for Web"),
        }
    }

    #[test]
    fn resolve_incompatible_platform() {
        let now = Utc::now();
        let manifest = AppManifest {
            id: "com.test.macos_only".into(),
            name: "MacOnly".into(),
            version: "1.0.0".into(),
            description: "macOS only app".into(),
            author_crown_id: "cpub1test".into(),
            platforms: vec![Platform::MacOS],
            permissions: vec![],
            versions: vec![AppVersion {
                version: "1.0.0".into(),
                release_notes: None,
                content_hash: "hash".into(),
                download_url: Some("https://example.com/dl".into()),
                min_os_version: None,
                released_at: now,
            }],
            created_at: now,
            updated_at: now,
            signature: None,
        };

        let request = InstallRequest {
            app_id: "com.test.macos_only".into(),
            version: "1.0.0".into(),
            platform: Platform::Linux,
        };

        let err = resolve_install_action(&request, &manifest).unwrap_err();
        assert_eq!(err, AppCatalogError::IncompatiblePlatform("Linux".into()));
    }

    #[test]
    fn resolve_version_not_found() {
        let manifest = make_full_manifest();
        let request = InstallRequest {
            app_id: "com.omnidea.throne".into(),
            version: "99.99.99".into(),
            platform: Platform::MacOS,
        };

        let err = resolve_install_action(&request, &manifest).unwrap_err();
        assert_eq!(
            err,
            AppCatalogError::VersionNotFound("99.99.99".into())
        );
    }

    #[test]
    fn resolve_no_download_url() {
        let now = Utc::now();
        let manifest = AppManifest {
            id: "com.test.nourl".into(),
            name: "NoUrl".into(),
            version: "1.0.0".into(),
            description: "Missing download URL".into(),
            author_crown_id: "cpub1test".into(),
            platforms: vec![Platform::MacOS],
            permissions: vec![],
            versions: vec![AppVersion {
                version: "1.0.0".into(),
                release_notes: None,
                content_hash: "hash".into(),
                download_url: None,
                min_os_version: None,
                released_at: now,
            }],
            created_at: now,
            updated_at: now,
            signature: None,
        };

        let request = InstallRequest {
            app_id: "com.test.nourl".into(),
            version: "1.0.0".into(),
            platform: Platform::MacOS,
        };

        let err = resolve_install_action(&request, &manifest).unwrap_err();
        assert!(matches!(err, AppCatalogError::ManifestInvalid(_)));
    }

    #[test]
    fn install_request_serde_round_trip() {
        let request = InstallRequest {
            app_id: "com.test.app".into(),
            version: "1.0.0".into(),
            platform: Platform::MacOS,
        };

        let json = serde_json::to_string(&request).expect("serialize");
        let loaded: InstallRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loaded.app_id, request.app_id);
        assert_eq!(loaded.version, request.version);
    }

    #[test]
    fn update_request_serde_round_trip() {
        let request = UpdateRequest {
            app_id: "com.test.app".into(),
            from_version: "1.0.0".into(),
            to_version: "2.0.0".into(),
        };

        let json = serde_json::to_string(&request).expect("serialize");
        let loaded: UpdateRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loaded.app_id, request.app_id);
        assert_eq!(loaded.from_version, request.from_version);
        assert_eq!(loaded.to_version, request.to_version);
    }

    #[test]
    fn uninstall_request_serde_round_trip() {
        let request = UninstallRequest {
            app_id: "com.test.app".into(),
        };

        let json = serde_json::to_string(&request).expect("serialize");
        let loaded: UninstallRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loaded.app_id, request.app_id);
    }

    #[test]
    fn install_action_serde_round_trip() {
        let actions = vec![
            InstallAction::Direct {
                download_url: "https://example.com/dl".into(),
                content_hash: "abc123".into(),
            },
            InstallAction::StoreRedirect {
                store_url: "https://apps.apple.com/app/test".into(),
            },
            InstallAction::WebInstall {
                url: "https://app.example.com".into(),
            },
        ];

        for action in &actions {
            let json = serde_json::to_string(action).expect("serialize");
            let loaded: InstallAction = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(&loaded, action);
        }
    }
}

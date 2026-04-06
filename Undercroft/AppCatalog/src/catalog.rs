//! The application catalog — a registry of known apps and their install states.
//!
//! [`AppCatalog`] is a pure in-memory registry. It does not perform network
//! requests, disk I/O, or any platform interaction. Discovery, downloading,
//! and actual installation are the caller's responsibility.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppCatalogError;
use crate::manifest::AppManifest;

/// The installation status of a catalog entry.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum InstallStatus {
    /// The app is known but not installed.
    Available,
    /// The app is installed at its latest known version.
    Installed,
    /// A newer version exists in the manifest.
    UpdateAvailable,
    /// The app does not support the current platform.
    Incompatible,
    /// The app binary is being downloaded.
    Downloading {
        /// Download progress as a fraction in `[0.0, 1.0]`.
        progress: f64,
    },
    /// The downloaded binary is being verified (hash check).
    Verifying,
    /// The app is being installed on the system.
    Installing,
    /// Installation or update failed.
    Failed {
        /// Human-readable failure reason.
        reason: String,
    },
}

/// A single entry in the catalog: manifest + current status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// The app's full manifest.
    pub manifest: AppManifest,
    /// Current installation status.
    pub status: InstallStatus,
    /// The installed version string, if any.
    pub installed_version: Option<String>,
    /// When this entry was first discovered.
    pub discovered_at: DateTime<Utc>,
}

/// The application catalog — an in-memory registry of apps.
///
/// Keyed by app ID (e.g., `"com.omnidea.throne"`). Provides search,
/// filtering, and lifecycle state management. Does not perform I/O.
#[derive(Clone, Debug, Default)]
pub struct AppCatalog {
    entries: HashMap<String, CatalogEntry>,
}

impl AppCatalog {
    /// Create an empty catalog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a manifest in the catalog.
    ///
    /// If an entry with the same ID already exists, its manifest is replaced
    /// and its status is preserved. New entries start as [`InstallStatus::Available`].
    pub fn add_manifest(&mut self, manifest: AppManifest) {
        let id = manifest.id.clone();
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.manifest = manifest;
        } else {
            self.entries.insert(
                id,
                CatalogEntry {
                    manifest,
                    status: InstallStatus::Available,
                    installed_version: None,
                    discovered_at: Utc::now(),
                },
            );
        }
    }

    /// Look up an entry by app ID.
    #[must_use]
    pub fn get(&self, app_id: &str) -> Option<&CatalogEntry> {
        self.entries.get(app_id)
    }

    /// Search for entries whose name or description contains `query`
    /// (case-insensitive substring match).
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&CatalogEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .values()
            .filter(|entry| {
                entry.manifest.name.to_lowercase().contains(&query_lower)
                    || entry
                        .manifest
                        .description
                        .to_lowercase()
                        .contains(&query_lower)
            })
            .collect()
    }

    /// All entries with [`InstallStatus::Installed`].
    #[must_use]
    pub fn installed(&self) -> Vec<&CatalogEntry> {
        self.entries
            .values()
            .filter(|e| e.status == InstallStatus::Installed)
            .collect()
    }

    /// All entries with [`InstallStatus::UpdateAvailable`].
    #[must_use]
    pub fn updates_available(&self) -> Vec<&CatalogEntry> {
        self.entries
            .values()
            .filter(|e| e.status == InstallStatus::UpdateAvailable)
            .collect()
    }

    /// All entries in the catalog.
    #[must_use]
    pub fn all(&self) -> Vec<&CatalogEntry> {
        self.entries.values().collect()
    }

    /// Mark an app as installed at the given version.
    ///
    /// # Errors
    ///
    /// - [`AppCatalogError::AppNotFound`] if no entry exists for `app_id`.
    /// - [`AppCatalogError::AlreadyInstalled`] if the app is already installed.
    pub fn mark_installed(
        &mut self,
        app_id: &str,
        version: &str,
    ) -> Result<(), AppCatalogError> {
        let entry = self
            .entries
            .get_mut(app_id)
            .ok_or_else(|| AppCatalogError::AppNotFound(app_id.into()))?;

        if entry.status == InstallStatus::Installed {
            return Err(AppCatalogError::AlreadyInstalled(app_id.into()));
        }

        entry.status = InstallStatus::Installed;
        entry.installed_version = Some(version.into());
        Ok(())
    }

    /// Mark an app as uninstalled (revert to [`InstallStatus::Available`]).
    ///
    /// # Errors
    ///
    /// - [`AppCatalogError::AppNotFound`] if no entry exists for `app_id`.
    /// - [`AppCatalogError::NotInstalled`] if the app is not currently installed.
    pub fn mark_uninstalled(&mut self, app_id: &str) -> Result<(), AppCatalogError> {
        let entry = self
            .entries
            .get_mut(app_id)
            .ok_or_else(|| AppCatalogError::AppNotFound(app_id.into()))?;

        if entry.status != InstallStatus::Installed
            && entry.status != InstallStatus::UpdateAvailable
        {
            return Err(AppCatalogError::NotInstalled(app_id.into()));
        }

        entry.status = InstallStatus::Available;
        entry.installed_version = None;
        Ok(())
    }

    /// Set an arbitrary status on an entry.
    ///
    /// # Errors
    ///
    /// - [`AppCatalogError::AppNotFound`] if no entry exists for `app_id`.
    pub fn mark_status(
        &mut self,
        app_id: &str,
        status: InstallStatus,
    ) -> Result<(), AppCatalogError> {
        let entry = self
            .entries
            .get_mut(app_id)
            .ok_or_else(|| AppCatalogError::AppNotFound(app_id.into()))?;

        entry.status = status;
        Ok(())
    }

    /// Scan all installed entries and mark those with a newer manifest
    /// version as [`InstallStatus::UpdateAvailable`].
    ///
    /// An entry is considered to have an update if it is currently
    /// [`InstallStatus::Installed`], has an `installed_version`, and the
    /// manifest's latest version string differs from the installed one.
    pub fn check_updates(&mut self) {
        for entry in self.entries.values_mut() {
            if entry.status != InstallStatus::Installed {
                continue;
            }

            let installed = match &entry.installed_version {
                Some(v) => v.clone(),
                None => continue,
            };

            if let Some(latest) = entry.manifest.latest_version() {
                if latest.version != installed {
                    entry.status = InstallStatus::UpdateAvailable;
                }
            }
        }
    }

    /// Remove an entry from the catalog entirely.
    ///
    /// Returns the removed entry, or `None` if no entry existed.
    pub fn remove(&mut self, app_id: &str) -> Option<CatalogEntry> {
        self.entries.remove(app_id)
    }

    /// The number of entries in the catalog.
    #[must_use]
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{AppVersion, Permission, Platform};

    fn make_manifest(id: &str, name: &str, version: &str) -> AppManifest {
        let now = Utc::now();
        AppManifest {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            description: format!("A test app called {name}"),
            author_crown_id: "cpub1testauthor".into(),
            platforms: vec![Platform::MacOS, Platform::IOS],
            permissions: vec![Permission::Network],
            versions: vec![AppVersion {
                version: version.into(),
                release_notes: None,
                content_hash: "hash123".into(),
                download_url: Some("https://example.com/download".into()),
                min_os_version: None,
                released_at: now,
            }],
            created_at: now,
            updated_at: now,
            signature: None,
        }
    }

    #[test]
    fn add_and_get() {
        let mut catalog = AppCatalog::new();
        let manifest = make_manifest("com.test.app", "TestApp", "1.0.0");
        catalog.add_manifest(manifest);

        let entry = catalog.get("com.test.app").expect("should find entry");
        assert_eq!(entry.manifest.name, "TestApp");
        assert_eq!(entry.status, InstallStatus::Available);
        assert_eq!(catalog.count(), 1);
    }

    #[test]
    fn add_updates_existing() {
        let mut catalog = AppCatalog::new();

        let m1 = make_manifest("com.test.app", "TestApp", "1.0.0");
        catalog.add_manifest(m1);

        // Mark as installed so we can verify status preservation.
        catalog.mark_installed("com.test.app", "1.0.0").unwrap();

        let m2 = make_manifest("com.test.app", "TestApp Updated", "2.0.0");
        catalog.add_manifest(m2);

        let entry = catalog.get("com.test.app").unwrap();
        assert_eq!(entry.manifest.name, "TestApp Updated");
        // Status should be preserved across manifest update.
        assert_eq!(entry.status, InstallStatus::Installed);
        assert_eq!(catalog.count(), 1);
    }

    #[test]
    fn get_missing_returns_none() {
        let catalog = AppCatalog::new();
        assert!(catalog.get("com.nonexistent").is_none());
    }

    #[test]
    fn search_case_insensitive() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.throne", "Throne", "1.0.0"));
        catalog.add_manifest(make_manifest("com.test.scry", "Scry", "1.0.0"));
        catalog.add_manifest(make_manifest("com.test.omny", "Omny", "1.0.0"));

        // Search by name (case insensitive).
        let results = catalog.search("throne");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].manifest.id, "com.test.throne");

        // Search by description (all contain "test app").
        let results = catalog.search("TEST APP");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn search_no_results() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));
        let results = catalog.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn installed_filter() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.a", "AppA", "1.0.0"));
        catalog.add_manifest(make_manifest("com.test.b", "AppB", "1.0.0"));
        catalog.add_manifest(make_manifest("com.test.c", "AppC", "1.0.0"));

        catalog.mark_installed("com.test.a", "1.0.0").unwrap();
        catalog.mark_installed("com.test.c", "1.0.0").unwrap();

        let installed = catalog.installed();
        assert_eq!(installed.len(), 2);
    }

    #[test]
    fn updates_available_filter() {
        let mut catalog = AppCatalog::new();

        let now = Utc::now();
        let mut manifest = make_manifest("com.test.app", "TestApp", "2.0.0");
        manifest.versions.push(AppVersion {
            version: "2.0.0".into(),
            release_notes: Some("New version".into()),
            content_hash: "newhash".into(),
            download_url: Some("https://example.com/v2".into()),
            min_os_version: None,
            released_at: now + chrono::Duration::days(1),
        });
        catalog.add_manifest(manifest);

        // Install at old version.
        catalog.mark_installed("com.test.app", "1.0.0").unwrap();

        // check_updates should detect the newer version.
        catalog.check_updates();

        let updates = catalog.updates_available();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].manifest.id, "com.test.app");
    }

    #[test]
    fn check_updates_no_change_when_current() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));
        catalog.mark_installed("com.test.app", "1.0.0").unwrap();

        catalog.check_updates();

        // Still installed, no update needed.
        let entry = catalog.get("com.test.app").unwrap();
        assert_eq!(entry.status, InstallStatus::Installed);
    }

    #[test]
    fn mark_installed_success() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));

        catalog.mark_installed("com.test.app", "1.0.0").unwrap();
        let entry = catalog.get("com.test.app").unwrap();
        assert_eq!(entry.status, InstallStatus::Installed);
        assert_eq!(entry.installed_version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn mark_installed_already_installed() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));
        catalog.mark_installed("com.test.app", "1.0.0").unwrap();

        let err = catalog
            .mark_installed("com.test.app", "1.0.0")
            .unwrap_err();
        assert_eq!(err, AppCatalogError::AlreadyInstalled("com.test.app".into()));
    }

    #[test]
    fn mark_installed_not_found() {
        let mut catalog = AppCatalog::new();
        let err = catalog
            .mark_installed("com.nonexistent", "1.0.0")
            .unwrap_err();
        assert_eq!(err, AppCatalogError::AppNotFound("com.nonexistent".into()));
    }

    #[test]
    fn mark_uninstalled_success() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));
        catalog.mark_installed("com.test.app", "1.0.0").unwrap();

        catalog.mark_uninstalled("com.test.app").unwrap();
        let entry = catalog.get("com.test.app").unwrap();
        assert_eq!(entry.status, InstallStatus::Available);
        assert!(entry.installed_version.is_none());
    }

    #[test]
    fn mark_uninstalled_not_installed() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));

        let err = catalog.mark_uninstalled("com.test.app").unwrap_err();
        assert_eq!(err, AppCatalogError::NotInstalled("com.test.app".into()));
    }

    #[test]
    fn mark_uninstalled_not_found() {
        let mut catalog = AppCatalog::new();
        let err = catalog.mark_uninstalled("com.nonexistent").unwrap_err();
        assert_eq!(err, AppCatalogError::AppNotFound("com.nonexistent".into()));
    }

    #[test]
    fn mark_status_arbitrary() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));

        catalog
            .mark_status("com.test.app", InstallStatus::Downloading { progress: 0.5 })
            .unwrap();
        let entry = catalog.get("com.test.app").unwrap();
        assert_eq!(
            entry.status,
            InstallStatus::Downloading { progress: 0.5 }
        );
    }

    #[test]
    fn mark_status_not_found() {
        let mut catalog = AppCatalog::new();
        let err = catalog
            .mark_status("com.nonexistent", InstallStatus::Verifying)
            .unwrap_err();
        assert_eq!(err, AppCatalogError::AppNotFound("com.nonexistent".into()));
    }

    #[test]
    fn remove_entry() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));
        assert_eq!(catalog.count(), 1);

        let removed = catalog.remove("com.test.app");
        assert!(removed.is_some());
        assert_eq!(catalog.count(), 0);
        assert!(catalog.get("com.test.app").is_none());
    }

    #[test]
    fn remove_nonexistent() {
        let mut catalog = AppCatalog::new();
        assert!(catalog.remove("com.nonexistent").is_none());
    }

    #[test]
    fn all_entries() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.a", "A", "1.0.0"));
        catalog.add_manifest(make_manifest("com.test.b", "B", "1.0.0"));

        let all = catalog.all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn mark_uninstalled_from_update_available() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));
        catalog.mark_installed("com.test.app", "0.9.0").unwrap();
        catalog.check_updates();

        // Should be UpdateAvailable now.
        let entry = catalog.get("com.test.app").unwrap();
        assert_eq!(entry.status, InstallStatus::UpdateAvailable);

        // Uninstalling from UpdateAvailable should work.
        catalog.mark_uninstalled("com.test.app").unwrap();
        let entry = catalog.get("com.test.app").unwrap();
        assert_eq!(entry.status, InstallStatus::Available);
    }

    #[test]
    fn catalog_serde_entry_round_trip() {
        let mut catalog = AppCatalog::new();
        catalog.add_manifest(make_manifest("com.test.app", "TestApp", "1.0.0"));
        catalog.mark_installed("com.test.app", "1.0.0").unwrap();

        let entry = catalog.get("com.test.app").unwrap();
        let json = serde_json::to_string(entry).expect("serialize entry");
        let loaded: CatalogEntry = serde_json::from_str(&json).expect("deserialize entry");
        assert_eq!(loaded.manifest.id, "com.test.app");
        assert_eq!(loaded.status, InstallStatus::Installed);
    }
}

# AppCatalog -- Application Registry

The app catalog for Omninet. A pure in-memory registry of application manifests, their install states, and lifecycle operations. AppCatalog does NOT perform discovery, downloading, or platform-specific installation -- those are the caller's responsibilities.

## Architecture

```
AppCatalog
    |-- manifest.rs   -- AppManifest, AppVersion, Platform, Permission
    |   |-- AppManifest: id, name, version, description, author_crown_id, platforms, permissions, versions, signature
    |   |-- supports_platform(platform) -> bool
    |   |-- latest_version() -> Option<&AppVersion>  (by most recent released_at)
    |   |-- verify_signature() -> Result  (BIP-340 via Crown::Signature)
    |   +-- signable_bytes() -> Vec<u8>  (id || name || version as UTF-8)
    |-- catalog.rs    -- AppCatalog, CatalogEntry, InstallStatus
    |   |-- AppCatalog: HashMap<String, CatalogEntry> keyed by app_id
    |   |-- add_manifest(manifest)  (adds or updates, preserves status)
    |   |-- get(app_id) / search(query) / installed() / updates_available() / all()
    |   |-- mark_installed(app_id, version) / mark_uninstalled(app_id) / mark_status(app_id, status)
    |   |-- check_updates()  (scans installed entries, marks UpdateAvailable)
    |   +-- remove(app_id) / count()
    |-- lifecycle.rs  -- InstallRequest, UpdateRequest, UninstallRequest, InstallAction
    |   |-- resolve_install_action(request, manifest) -> Result<InstallAction>
    |   |-- Platform routing: macOS/Linux/Windows/GrapheneOS -> Direct, iOS -> StoreRedirect, Android -> StoreRedirect, Web -> WebInstall
    |   +-- Validates platform support + version existence
    +-- error.rs      -- AppCatalogError (7 variants)
```

## Key Types

- **AppManifest** -- Canonical app metadata. Reverse-domain ID, author crown_id, platforms, permissions, version history. Optional BIP-340 signature by author.
- **AppVersion** -- A single release: version string, content_hash (SHA-256), download_url, release_notes, min_os_version, released_at.
- **Platform** -- Enum: MacOS, IOS, Android, Linux, Windows, GrapheneOS, Web. Serde-renamed to lowercase.
- **Permission** -- Enum: Network, Storage, Camera, Microphone, Location, Contacts, Notifications, BackgroundExecution, Custom(String).
- **AppCatalog** -- In-memory registry. HashMap<String, CatalogEntry>. No I/O, no networking.
- **CatalogEntry** -- Manifest + InstallStatus + installed_version + discovered_at.
- **InstallStatus** -- Available, Installed, UpdateAvailable, Incompatible, Downloading{progress}, Verifying, Installing, Failed{reason}.
- **InstallAction** -- Direct{download_url, content_hash}, StoreRedirect{store_url}, WebInstall{url}.
- **AppCatalogError** -- ManifestInvalid, SignatureInvalid, AppNotFound, VersionNotFound, IncompatiblePlatform, AlreadyInstalled, NotInstalled.

## Dependencies

```toml
crown = { path = "../../Crown" }  # Signature verification
serde, serde_json, chrono, thiserror
```

**No Globe dependency.** AppCatalog is a pure registry. Discovery is the caller's job.

## Design Decisions

- **Pure registry.** No I/O, no networking, no platform interaction. This is deliberate -- AppCatalog is a data structure, not an orchestrator.
- **Status preservation on manifest update.** When `add_manifest` updates an existing entry, the install status is preserved. This prevents re-discovery from resetting install state.
- **Uninstall from UpdateAvailable.** `mark_uninstalled` accepts both Installed and UpdateAvailable status, because an app with a pending update is still installed.
- **String-based version comparison.** `check_updates` compares version strings for inequality, not semver ordering. This means any difference triggers UpdateAvailable. Semver comparison can be added later.
- **Signature is optional.** Unsigned manifests are valid for the registry but `verify_signature()` will return ManifestInvalid if no signature is present.

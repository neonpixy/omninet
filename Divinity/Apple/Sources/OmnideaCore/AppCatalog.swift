import COmnideaFFI
import Foundation
import os

private let logger = Logger(subsystem: "co.omnidea", category: "AppCatalog")

// MARK: - AppCatalog

/// Swift wrapper around the Rust AppCatalog (in-memory app registry).
///
/// AppCatalog manages app manifests, tracks install status, and supports
/// searching, updates, and lifecycle operations for the extension system.
///
/// ```swift
/// let catalog = AppCatalog()
/// try catalog.addManifest(manifestJSON)
/// let results = try catalog.search(query: "studio")
/// try catalog.markInstalled(appId: "co.omnidea.studio", version: "1.0.0")
/// ```
public final class AppCatalog: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_appcatalog_new()!
    }

    deinit {
        divi_appcatalog_free(ptr)
    }

    // MARK: - Manifest Management

    /// Add or update a manifest in the catalog.
    ///
    /// - Parameter manifestJSON: A JSON-encoded `AppManifest`.
    public func addManifest(_ manifestJSON: String) throws {
        let result = divi_appcatalog_add_manifest(ptr, manifestJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add manifest to catalog")
        }
    }

    /// Look up a catalog entry by app ID.
    ///
    /// - Parameter appId: The app's unique identifier.
    /// - Returns: JSON-encoded `CatalogEntry`, or nil if not found.
    public func get(appId: String) throws -> String? {
        guard let json = divi_appcatalog_get(ptr, appId) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Search for entries by name or description (case-insensitive substring).
    ///
    /// - Parameter query: The search term.
    /// - Returns: JSON-encoded array of `CatalogEntry`.
    public func search(query: String) throws -> String {
        guard let json = divi_appcatalog_search(ptr, query) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to search catalog")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all installed entries.
    ///
    /// - Returns: JSON-encoded array of `CatalogEntry`.
    public func installed() throws -> String {
        guard let json = divi_appcatalog_installed(ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get installed entries")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all entries with updates available.
    ///
    /// - Returns: JSON-encoded array of `CatalogEntry`.
    public func updatesAvailable() throws -> String {
        guard let json = divi_appcatalog_updates_available(ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get entries with updates")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all entries in the catalog.
    ///
    /// - Returns: JSON-encoded array of `CatalogEntry`.
    public func all() throws -> String {
        guard let json = divi_appcatalog_all(ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get all catalog entries")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// The number of entries in the catalog.
    public var count: Int {
        Int(divi_appcatalog_count(ptr))
    }

    // MARK: - Install Lifecycle

    /// Mark an app as installed at the given version.
    ///
    /// - Parameters:
    ///   - appId: The app's unique identifier.
    ///   - version: The installed version string.
    public func markInstalled(appId: String, version: String) throws {
        let result = divi_appcatalog_mark_installed(ptr, appId, version)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to mark '\(appId)' as installed")
        }
    }

    /// Mark an app as uninstalled (revert to Available).
    ///
    /// - Parameter appId: The app's unique identifier.
    public func markUninstalled(appId: String) throws {
        let result = divi_appcatalog_mark_uninstalled(ptr, appId)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to mark '\(appId)' as uninstalled")
        }
    }

    /// Set an arbitrary install status on an entry.
    ///
    /// - Parameters:
    ///   - appId: The app's unique identifier.
    ///   - statusJSON: A JSON-encoded `InstallStatus`.
    public func markStatus(appId: String, statusJSON: String) throws {
        let result = divi_appcatalog_mark_status(ptr, appId, statusJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set status on '\(appId)'")
        }
    }

    /// Scan installed entries and flag those with newer manifest versions.
    public func checkUpdates() {
        divi_appcatalog_check_updates(ptr)
    }

    // MARK: - Removal

    /// Remove an entry from the catalog entirely.
    ///
    /// - Parameter appId: The app's unique identifier.
    /// - Returns: JSON-encoded `CatalogEntry` of the removed entry, or nil if not found.
    public func remove(appId: String) throws -> String? {
        guard let json = divi_appcatalog_remove(ptr, appId) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    // MARK: - Static Helpers

    /// Resolve the platform-appropriate install action for a request.
    ///
    /// - Parameters:
    ///   - requestJSON: A JSON-encoded `InstallRequest`.
    ///   - manifestJSON: A JSON-encoded `AppManifest`.
    /// - Returns: JSON-encoded `InstallAction`.
    public static func resolveInstall(requestJSON: String, manifestJSON: String) throws -> String {
        guard let json = divi_appcatalog_resolve_install(requestJSON, manifestJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to resolve install action")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Verify a manifest's BIP-340 signature.
    ///
    /// - Parameter manifestJSON: A JSON-encoded `AppManifest`.
    /// - Returns: `true` if the signature is valid.
    public static func manifestVerify(_ manifestJSON: String) throws -> Bool {
        let result = divi_appcatalog_manifest_verify(manifestJSON)
        if result != 0 {
            try OmnideaError.check()
            return false
        }
        return true
    }

    /// Get the latest version entry from a manifest.
    ///
    /// - Parameter manifestJSON: A JSON-encoded `AppManifest`.
    /// - Returns: JSON-encoded `AppVersion`, or nil if no versions exist.
    public static func manifestLatestVersion(_ manifestJSON: String) throws -> String? {
        guard let json = divi_appcatalog_manifest_latest_version(manifestJSON) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

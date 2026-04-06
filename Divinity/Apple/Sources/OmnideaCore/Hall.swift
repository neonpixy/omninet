import COmnideaFFI
import Foundation

// MARK: - Hall (File I/O)

/// Swift wrappers for the Hall crate — encrypted .idea file I/O.
///
/// All functions are stateless. Keys cross the boundary as raw `Data` bytes.
public enum Hall {

    // MARK: - Types

    /// Result of reading an .idea package, including non-fatal warnings.
    public struct ReadResult: Sendable {
        /// JSON-encoded IdeaPackage.
        public let packageJSON: String
        /// JSON array of HallWarning objects, if any.
        public let warningsJSON: String?
    }

    // MARK: - Scholar (Read Operations)

    /// Check whether a path is a .idea package (directory with Header.json).
    public static func isIdeaPackage(path: String) -> Bool {
        divi_hall_is_idea_package(path)
    }

    /// Read just the header from an .idea package (no key needed).
    ///
    /// Returns a JSON-encoded Header string.
    public static func readHeader(path: String) throws -> String {
        guard let json = divi_hall_read_header(path) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to read header at '\(path)'")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Read a full .idea package with decryption and graceful degradation.
    public static func read(path: String, contentKey: Data) throws -> ReadResult {
        var warningsPtr: UnsafeMutablePointer<CChar>?

        let packagePtr: UnsafeMutablePointer<CChar>? = contentKey.withUnsafeBytes { keyBuf in
            divi_hall_read(
                path,
                keyBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(keyBuf.count),
                &warningsPtr
            )
        }

        guard let packagePtr else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to read .idea at '\(path)'")
        }
        defer { divi_free_string(packagePtr) }

        let packageJSON = String(cString: packagePtr)

        var warningsJSON: String?
        if let warningsPtr {
            warningsJSON = String(cString: warningsPtr)
            divi_free_string(warningsPtr)
        }

        return ReadResult(packageJSON: packageJSON, warningsJSON: warningsJSON)
    }

    // MARK: - Scribe (Write Operations)

    /// Write an IdeaPackage to disk with encryption.
    ///
    /// - Parameters:
    ///   - packageJSON: JSON-encoded IdeaPackage.
    ///   - path: Destination directory path (overrides package's path field).
    ///   - contentKey: Encryption key bytes.
    /// - Returns: Number of bytes written.
    public static func write(packageJSON: String, path: String, contentKey: Data) throws -> Int {
        let bytesWritten: Int64 = contentKey.withUnsafeBytes { keyBuf in
            divi_hall_write(
                packageJSON,
                path,
                keyBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(keyBuf.count)
            )
        }

        if bytesWritten < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to write .idea to '\(path)'")
        }

        return Int(bytesWritten)
    }

    // MARK: - Archivist (Asset Operations)

    /// Import raw bytes as an encrypted asset.
    ///
    /// - Returns: The SHA-256 hex hash of the stored asset.
    public static func assetImport(
        data: Data,
        ideaPath: String,
        contentKey: Data,
        vocabSeed: Data
    ) throws -> String {
        let hashPtr: UnsafeMutablePointer<CChar>? = data.withUnsafeBytes { dataBuf in
            contentKey.withUnsafeBytes { keyBuf in
                vocabSeed.withUnsafeBytes { seedBuf in
                    divi_hall_asset_import(
                        dataBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                        UInt(dataBuf.count),
                        ideaPath,
                        keyBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                        UInt(keyBuf.count),
                        seedBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                        UInt(seedBuf.count)
                    )
                }
            }
        }

        guard let hashPtr else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to import asset")
        }
        defer { divi_free_string(hashPtr) }
        return String(cString: hashPtr)
    }

    /// Import a file as an encrypted asset.
    ///
    /// - Returns: The SHA-256 hex hash of the stored asset.
    public static func assetImportFile(
        sourcePath: String,
        ideaPath: String,
        contentKey: Data,
        vocabSeed: Data
    ) throws -> String {
        let hashPtr: UnsafeMutablePointer<CChar>? = contentKey.withUnsafeBytes { keyBuf in
            vocabSeed.withUnsafeBytes { seedBuf in
                divi_hall_asset_import_file(
                    sourcePath,
                    ideaPath,
                    keyBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(keyBuf.count),
                    seedBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(seedBuf.count)
                )
            }
        }

        guard let hashPtr else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to import asset file '\(sourcePath)'")
        }
        defer { divi_free_string(hashPtr) }
        return String(cString: hashPtr)
    }

    /// Read an asset by its hash. Decrypts, deobfuscates, and verifies integrity.
    public static func assetRead(
        hash: String,
        ideaPath: String,
        contentKey: Data,
        vocabSeed: Data
    ) throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result: Int32 = contentKey.withUnsafeBytes { keyBuf in
            vocabSeed.withUnsafeBytes { seedBuf in
                divi_hall_asset_read(
                    hash,
                    ideaPath,
                    keyBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(keyBuf.count),
                    seedBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(seedBuf.count),
                    &outData,
                    &outLen
                )
            }
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to read asset '\(hash)'")
        }

        guard let outData else { return Data() }
        let data = Data(bytes: outData, count: Int(outLen))
        divi_free_bytes(outData, outLen)
        return data
    }

    /// Export an asset to a destination file (decrypted).
    public static func assetExport(
        hash: String,
        ideaPath: String,
        destPath: String,
        contentKey: Data,
        vocabSeed: Data
    ) throws {
        let result: Int32 = contentKey.withUnsafeBytes { keyBuf in
            vocabSeed.withUnsafeBytes { seedBuf in
                divi_hall_asset_export(
                    hash,
                    ideaPath,
                    destPath,
                    keyBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(keyBuf.count),
                    seedBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(seedBuf.count)
                )
            }
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to export asset '\(hash)' to '\(destPath)'")
        }
    }

    /// List all asset hashes in the Assets/ directory.
    ///
    /// Returns a JSON array of hex hash strings.
    public static func assetList(ideaPath: String) throws -> String {
        guard let json = divi_hall_asset_list(ideaPath) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to list assets at '\(ideaPath)'")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Check if an asset exists by its hash.
    public static func assetExists(hash: String, ideaPath: String) -> Bool {
        divi_hall_asset_exists(hash, ideaPath)
    }

    /// Delete an asset by its hash.
    public static func assetDelete(hash: String, ideaPath: String) throws {
        let result = divi_hall_asset_delete(hash, ideaPath)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to delete asset '\(hash)'")
        }
    }
}

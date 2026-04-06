import COmnideaFFI
import Foundation

/// Swift wrapper around the Rust Contacts (module registry).
public final class Contacts: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_contacts_new()!
    }

    deinit {
        divi_contacts_free(ptr)
    }

    /// Register a module from a JSON-encodable value.
    public func register(_ info: some Encodable) throws {
        let json = try JSONEncoder().encode(info)
        let jsonString = String(data: json, encoding: .utf8)!

        let result = divi_contacts_register(ptr, jsonString)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Contacts register failed")
        }
    }

    /// Unregister a module by ID. Dependents are shut down first.
    public func unregister(_ moduleId: String) throws {
        let result = divi_contacts_unregister(ptr, moduleId)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Contacts unregister failed")
        }
    }

    /// Shut down all modules in dependency order.
    public func shutdownAll() {
        divi_contacts_shutdown_all(ptr)
    }

    /// Look up a module by ID. Returns JSON string or nil.
    public func lookup(_ moduleId: String) -> String? {
        guard let json = divi_contacts_lookup(ptr, moduleId) else { return nil }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all registered modules as a JSON array string.
    public func allModulesJSON() -> String {
        guard let json = divi_contacts_all_modules(ptr) else { return "[]" }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all registered module IDs.
    public func registeredModuleIds() -> [String] {
        guard let json = divi_contacts_registered_module_ids(ptr) else { return [] }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return (try? JSONDecoder().decode([String].self, from: data)) ?? []
    }

    /// Get modules that depend on the given module, as JSON.
    public func dependentsOf(_ moduleId: String) -> String {
        guard let json = divi_contacts_dependents_of(ptr, moduleId) else { return "[]" }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

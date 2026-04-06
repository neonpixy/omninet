#pragma once

#include "error.hpp"
#include <functional>
#include <string>
#include <vector>
#include <optional>

namespace divinity {

/// Boxed shutdown callback — heap-allocated for void* context passing.
struct ShutdownBox {
    std::function<void()> fn;
};

/// C trampoline for shutdown callbacks.
inline void shutdown_trampoline(void* context) {
    if (!context) return;
    auto* box = static_cast<ShutdownBox*>(context);
    try {
        box->fn();
    } catch (...) {}
    delete box;  // One-shot: consumed after firing.
}

/// Module registry with dependency tracking.
class Contacts {
    ::Contacts* ptr_;

public:
    Contacts() : ptr_(divi_contacts_new()) {
        if (!ptr_) throw OmnideaError("Failed to create Contacts");
    }

    ~Contacts() {
        if (ptr_) divi_contacts_free(ptr_);
    }

    Contacts(const Contacts&) = delete;
    Contacts& operator=(const Contacts&) = delete;

    /// Register a module from a JSON string. Throws on error.
    void register_module(const std::string& json) {
        OmnideaError::check(divi_contacts_register(ptr_, json.c_str()));
    }

    /// Register a module with a shutdown callback.
    void register_with_shutdown(const std::string& json,
                                 std::function<void()> on_shutdown) {
        auto* box = new ShutdownBox{std::move(on_shutdown)};
        int32_t result = divi_contacts_register_with_shutdown(
            ptr_, json.c_str(), shutdown_trampoline, box
        );
        if (result != 0) {
            delete box;
            throw OmnideaError::last();
        }
    }

    /// Unregister a module by ID. Dependents shut down first. Throws on error.
    void unregister(const std::string& module_id) {
        OmnideaError::check(divi_contacts_unregister(ptr_, module_id.c_str()));
    }

    /// Shut down all modules in dependency order.
    void shutdown_all() {
        divi_contacts_shutdown_all(ptr_);
    }

    /// Look up a module by ID. Returns JSON string or nullopt.
    std::optional<std::string> lookup(const std::string& module_id) const {
        RustString json(divi_contacts_lookup(ptr_, module_id.c_str()));
        if (!json) return std::nullopt;
        return json.to_string();
    }

    /// Get all registered modules as a JSON array string.
    std::string all_modules_json() const {
        RustString json(divi_contacts_all_modules(ptr_));
        return json.to_string();
    }

    /// Get all registered module IDs.
    std::vector<std::string> registered_module_ids() const {
        RustString json(divi_contacts_registered_module_ids(ptr_));
        if (!json) return {};
        return parse_json_string_array(json.to_string());
    }

    /// Get all modules that depend on the given module, as a JSON array.
    std::string dependents_of(const std::string& module_id) const {
        RustString json(divi_contacts_dependents_of(ptr_, module_id.c_str()));
        return json.to_string();
    }
};

} // namespace divinity

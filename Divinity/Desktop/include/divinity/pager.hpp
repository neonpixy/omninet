#pragma once

#include "error.hpp"
#include <string>
#include <optional>

namespace divinity {

/// Notification queue — no callbacks, pure query/mutation.
class Pager {
    ::Pager* ptr_;

public:
    Pager() : ptr_(divi_pager_new()) {
        if (!ptr_) throw OmnideaError("Failed to create Pager");
    }

    ~Pager() {
        if (ptr_) divi_pager_free(ptr_);
    }

    Pager(const Pager&) = delete;
    Pager& operator=(const Pager&) = delete;

    /// Push a notification from a JSON string. Returns UUID string or nullopt.
    std::optional<std::string> notify(const std::string& json) {
        RustString uuid(divi_pager_notify(ptr_, json.c_str()));
        if (!uuid) return std::nullopt;
        return uuid.to_string();
    }

    /// Get pending notifications as a JSON array.
    std::string get_pending_json(const char* priority_json = nullptr) const {
        RustString json(divi_pager_get_pending(ptr_, priority_json));
        return json.to_string();
    }

    /// Get unread notifications as a JSON array.
    std::string get_unread_json() const {
        RustString json(divi_pager_get_unread(ptr_));
        return json.to_string();
    }

    /// Mark a notification as read. Returns true if found.
    bool mark_read(const std::string& uuid_str) {
        return divi_pager_mark_read(ptr_, uuid_str.c_str());
    }

    /// Dismiss a notification. Returns true if found.
    bool dismiss(const std::string& uuid_str) {
        return divi_pager_dismiss(ptr_, uuid_str.c_str());
    }

    /// Get the count of unread notifications.
    size_t badge_count() const {
        return static_cast<size_t>(divi_pager_badge_count(ptr_));
    }

    /// Prune expired notifications.
    void prune_expired() {
        divi_pager_prune_expired(ptr_);
    }

    /// Export pager state as a JSON array.
    std::string export_state_json() const {
        RustString json(divi_pager_export_state(ptr_));
        return json.to_string();
    }

    /// Restore pager state from a JSON array.
    void restore_state(const std::string& json) {
        divi_pager_restore_state(ptr_, json.c_str());
    }
};

} // namespace divinity

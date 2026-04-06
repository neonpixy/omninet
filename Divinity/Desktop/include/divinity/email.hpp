#pragma once

#include "error.hpp"
#include <functional>
#include <string>
#include <vector>
#include <array>
#include <unordered_map>
#include <cstdio>

namespace divinity {

/// Boxed email handler — heap-allocated for void* context passing.
struct EmailHandlerBox {
    std::function<void(const uint8_t*, size_t)> fn;
};

/// C trampoline: called by Rust, forwards to the boxed std::function.
inline void email_trampoline(const uint8_t* data, uintptr_t len, void* context) {
    if (!context) return;
    auto* box = static_cast<EmailHandlerBox*>(context);
    try {
        box->fn(data, static_cast<size_t>(len));
    } catch (...) {
        // Fire-and-forget — swallow errors like Rust does.
    }
}

/// 16-byte UUID as a string (e.g., "550e8400-e29b-41d4-a716-446655440000").
inline std::string uuid_bytes_to_string(const uint8_t* bytes) {
    char buf[37];
    std::snprintf(buf, sizeof(buf),
        "%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11],
        bytes[12], bytes[13], bytes[14], bytes[15]);
    return std::string(buf);
}

/// Pub/sub event hub.
class Email {
    ::Email* ptr_;
    // Track subscriber boxes by UUID string for cleanup.
    std::unordered_map<std::string, EmailHandlerBox*> boxes_;

public:
    Email() : ptr_(divi_email_new()) {
        if (!ptr_) throw OmnideaError("Failed to create Email");
    }

    ~Email() {
        if (ptr_) divi_email_free(ptr_);
        for (auto& [_, box] : boxes_) delete box;
    }

    Email(const Email&) = delete;
    Email& operator=(const Email&) = delete;

    /// Subscribe to an email ID. Returns the subscriber UUID as a string.
    std::string subscribe_raw(const std::string& email_id,
                               std::function<void(const uint8_t*, size_t)> handler) {
        auto* box = new EmailHandlerBox{std::move(handler)};
        std::array<uint8_t, 16> uuid_bytes{};

        int32_t result = divi_email_subscribe_raw(
            ptr_, email_id.c_str(), email_trampoline, box, uuid_bytes.data()
        );

        if (result != 0) {
            delete box;
            throw OmnideaError::last();
        }

        auto uuid_str = uuid_bytes_to_string(uuid_bytes.data());
        boxes_[uuid_str] = box;
        return uuid_str;
    }

    /// Send raw bytes to all subscribers of an email ID.
    void send_raw(const std::string& email_id,
                  const std::vector<uint8_t>& data = {}) {
        divi_email_send_raw(
            ptr_, email_id.c_str(),
            data.empty() ? nullptr : data.data(),
            static_cast<uintptr_t>(data.size())
        );
    }

    /// Unsubscribe by UUID bytes (16-byte array).
    void unsubscribe(const std::array<uint8_t, 16>& uuid_bytes) {
        auto uuid_str = uuid_bytes_to_string(uuid_bytes.data());
        divi_email_unsubscribe(ptr_, uuid_bytes.data());
        auto it = boxes_.find(uuid_str);
        if (it != boxes_.end()) {
            delete it->second;
            boxes_.erase(it);
        }
    }

    /// Unsubscribe by UUID string. Parses the string back to bytes.
    void unsubscribe(const std::string& uuid_str) {
        auto bytes = parse_uuid_string(uuid_str);
        divi_email_unsubscribe(ptr_, bytes.data());
        auto it = boxes_.find(uuid_str);
        if (it != boxes_.end()) {
            delete it->second;
            boxes_.erase(it);
        }
    }

    /// Unsubscribe all subscribers for an email ID.
    void unsubscribe_all(const std::string& email_id) {
        divi_email_unsubscribe_all(ptr_, email_id.c_str());
        // Can't easily track which boxes belong to which email_id.
        // Leaked boxes are cleaned up on Email destruction.
    }

    /// Check if any subscribers exist for an email ID.
    bool has_subscribers(const std::string& email_id) const {
        return divi_email_has_subscribers(ptr_, email_id.c_str());
    }

    /// Get all active email IDs.
    std::vector<std::string> active_email_ids() const {
        RustString json(divi_email_active_email_ids(ptr_));
        if (!json) return {};
        return parse_json_string_array(json.to_string());
    }

private:
    /// Parse "550e8400-e29b-41d4-a716-446655440000" to 16 bytes.
    static std::array<uint8_t, 16> parse_uuid_string(const std::string& s) {
        std::array<uint8_t, 16> bytes{};
        size_t bi = 0;
        for (size_t i = 0; i < s.size() && bi < 16; ) {
            if (s[i] == '-') { ++i; continue; }
            unsigned int val = 0;
            std::sscanf(s.c_str() + i, "%2x", &val);
            bytes[bi++] = static_cast<uint8_t>(val);
            i += 2;
        }
        return bytes;
    }
};

} // namespace divinity

#pragma once

#include "error.hpp"
#include <functional>
#include <string>
#include <vector>
#include <unordered_map>
#include <cstdlib>
#include <cstring>

namespace divinity {

/// Boxed phone handler — heap-allocated so we can pass it as void* context.
struct PhoneHandlerBox {
    std::function<std::vector<uint8_t>(const uint8_t*, size_t)> fn;
};

/// C trampoline: called by Rust, forwards to the boxed std::function.
inline int32_t phone_trampoline(
    const uint8_t* request_data, uintptr_t request_len,
    uint8_t** response_data, uintptr_t* response_len,
    void* context
) {
    if (!context) return -1;
    auto* box = static_cast<PhoneHandlerBox*>(context);
    try {
        auto response = box->fn(request_data, static_cast<size_t>(request_len));
        if (response.empty()) {
            *response_data = nullptr;
            *response_len = 0;
        } else {
            auto* buf = static_cast<uint8_t*>(std::malloc(response.size()));
            if (!buf) return -1;
            std::memcpy(buf, response.data(), response.size());
            *response_data = buf;
            *response_len = static_cast<uintptr_t>(response.size());
        }
        return 0;
    } catch (...) {
        return -1;
    }
}

/// RPC switchboard — register handlers, make calls.
class Phone {
    ::Phone* ptr_;
    std::unordered_map<std::string, PhoneHandlerBox*> boxes_;

public:
    Phone() : ptr_(divi_phone_new()) {
        if (!ptr_) throw OmnideaError("Failed to create Phone");
    }

    ~Phone() {
        if (ptr_) divi_phone_free(ptr_);
        for (auto& [_, box] : boxes_) delete box;
    }

    Phone(const Phone&) = delete;
    Phone& operator=(const Phone&) = delete;

    /// Register a raw handler for a call ID.
    /// The handler receives request bytes and returns response bytes.
    void register_raw(const std::string& call_id,
                      std::function<std::vector<uint8_t>(const uint8_t*, size_t)> handler) {
        // Clean up any existing box for this call_id.
        auto it = boxes_.find(call_id);
        if (it != boxes_.end()) {
            delete it->second;
            boxes_.erase(it);
        }

        auto* box = new PhoneHandlerBox{std::move(handler)};
        boxes_[call_id] = box;
        divi_phone_register_raw(ptr_, call_id.c_str(), phone_trampoline, box);
    }

    /// Make a raw call. Returns response bytes. Throws on error.
    std::vector<uint8_t> call_raw(const std::string& call_id,
                                   const std::vector<uint8_t>& data = {}) {
        uint8_t* out_data = nullptr;
        uintptr_t out_len = 0;

        int32_t result = divi_phone_call_raw(
            ptr_, call_id.c_str(),
            data.empty() ? nullptr : data.data(),
            static_cast<uintptr_t>(data.size()),
            &out_data, &out_len
        );

        if (result != 0) throw OmnideaError::last();

        if (!out_data || out_len == 0) return {};

        std::vector<uint8_t> response(out_data, out_data + out_len);
        divi_free_bytes(out_data, out_len);
        return response;
    }

    /// Check if a handler is registered for a call ID.
    bool has_handler(const std::string& call_id) const {
        return divi_phone_has_handler(ptr_, call_id.c_str());
    }

    /// Get all registered call IDs.
    std::vector<std::string> registered_call_ids() const {
        RustString json(divi_phone_registered_call_ids(ptr_));
        if (!json) return {};
        return parse_json_string_array(json.to_string());
    }

    /// Unregister a handler by call ID.
    void unregister(const std::string& call_id) {
        divi_phone_unregister(ptr_, call_id.c_str());
        auto it = boxes_.find(call_id);
        if (it != boxes_.end()) {
            delete it->second;
            boxes_.erase(it);
        }
    }
};

} // namespace divinity

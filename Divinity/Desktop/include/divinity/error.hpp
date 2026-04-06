#pragma once

#include <stdexcept>
#include <string>
#include <vector>

extern "C" {
#include "divinity_ffi.h"
}

namespace divinity {

/// Exception type for Omnidea FFI errors.
class OmnideaError : public std::runtime_error {
public:
    explicit OmnideaError(const std::string& msg)
        : std::runtime_error(msg) {}

    /// Construct from the thread-local FFI error message.
    static OmnideaError last() {
        const char* msg = divi_last_error();
        return OmnideaError(msg ? std::string(msg) : "unknown error");
    }

    /// Throw if result indicates failure (non-zero).
    static void check(int32_t result) {
        if (result != 0) throw last();
    }
};

/// RAII wrapper for Rust-allocated C strings.
/// Automatically calls divi_free_string on destruction.
class RustString {
    char* ptr_;

public:
    explicit RustString(char* p) noexcept : ptr_(p) {}
    ~RustString() { if (ptr_) divi_free_string(ptr_); }

    RustString(const RustString&) = delete;
    RustString& operator=(const RustString&) = delete;
    RustString(RustString&& o) noexcept : ptr_(o.ptr_) { o.ptr_ = nullptr; }
    RustString& operator=(RustString&& o) noexcept {
        if (this != &o) {
            if (ptr_) divi_free_string(ptr_);
            ptr_ = o.ptr_;
            o.ptr_ = nullptr;
        }
        return *this;
    }

    const char* c_str() const noexcept { return ptr_; }
    std::string to_string() const { return ptr_ ? std::string(ptr_) : ""; }
    explicit operator bool() const noexcept { return ptr_ != nullptr; }
};

/// Minimal inline parser for JSON arrays of strings: ["a","b","c"].
/// Only handles simple quoted strings with no escaped quotes inside.
inline std::vector<std::string> parse_json_string_array(const std::string& json) {
    std::vector<std::string> result;
    bool in_string = false;
    std::string current;

    for (size_t i = 0; i < json.size(); ++i) {
        char c = json[i];
        if (!in_string) {
            if (c == '"') {
                in_string = true;
                current.clear();
            }
        } else {
            if (c == '"') {
                in_string = false;
                result.push_back(std::move(current));
            } else {
                current += c;
            }
        }
    }
    return result;
}

} // namespace divinity

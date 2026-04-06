cmake_minimum_required(VERSION 3.18)
project(omnidea_jni LANGUAGES C)

# Build the JNI shared library from the bridge file.
add_library(omnidea_jni SHARED jni/jni_bridge.c)

# Include the FFI C header.
target_include_directories(omnidea_jni PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../Apple/Sources/COmnideaFFI/include
)

# Link the prebuilt Rust static library.
# For Android NDK builds, cross-compile with:
#   cargo ndk -t arm64-v8a build --release
# Then set RUST_LIB_DIR to the target directory.
if(DEFINED RUST_LIB_DIR)
    set(RUST_SEARCH_PATH ${RUST_LIB_DIR})
elseif(DEFINED ANDROID_ABI)
    set(RUST_SEARCH_PATH
        ${CMAKE_CURRENT_SOURCE_DIR}/../../target/${ANDROID_ABI}/release
        ${CMAKE_CURRENT_SOURCE_DIR}/../../target/${ANDROID_ABI}/debug
    )
else()
    # Host build (for testing on macOS/Linux).
    set(RUST_SEARCH_PATH
        ${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug
        ${CMAKE_CURRENT_SOURCE_DIR}/../../target/release
    )
endif()

find_library(DIVINITY_FFI divinity_ffi PATHS ${RUST_SEARCH_PATH})

if(NOT DIVINITY_FFI)
    message(FATAL_ERROR
        "libdivinity_ffi not found. Build the Rust crate first.\n"
        "Searched: ${RUST_SEARCH_PATH}")
endif()

target_link_libraries(omnidea_jni PRIVATE ${DIVINITY_FFI})

# Android-specific: link the log library.
if(ANDROID)
    find_library(LOG_LIB log)
    target_link_libraries(omnidea_jni PRIVATE ${LOG_LIB})
endif()

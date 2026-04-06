cmake_minimum_required(VERSION 3.16)
project(OmnideaDesktop LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Header-only library.
add_library(omnidea_core INTERFACE)
target_include_directories(omnidea_core INTERFACE
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${CMAKE_CURRENT_SOURCE_DIR}/../Apple/Sources/COmnideaFFI/include
)

# Find the Rust static library (cargo build output).
find_library(DIVINITY_FFI divinity_ffi
    PATHS
        ${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug
        ${CMAKE_CURRENT_SOURCE_DIR}/../../target/release
)

if(NOT DIVINITY_FFI)
    message(FATAL_ERROR
        "libdivinity_ffi not found. Run 'cargo build' from the Omnidea root first.")
endif()

# Platform-specific link dependencies for the Rust static library.
if(APPLE)
    set(PLATFORM_LIBS "-framework Security" "-framework CoreFoundation")
elseif(UNIX)
    set(PLATFORM_LIBS pthread dl m)
elseif(WIN32)
    set(PLATFORM_LIBS ws2_32 userenv bcrypt ntdll)
endif()

# Test executable.
add_executable(equipment_tests tests/equipment_tests.cpp)
target_link_libraries(equipment_tests PRIVATE
    omnidea_core
    ${DIVINITY_FFI}
    ${PLATFORM_LIBS}
)

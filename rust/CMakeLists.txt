cmake_minimum_required(VERSION 3.22.1)

set(LOG_SURGEON_CARGO_TARGET_DIR
    "${CMAKE_CURRENT_BINARY_DIR}/cargo"
    CACHE PATH
    "Directory in which Cargo places its build artifacts."
)

find_program(LOG_SURGEON_CARGO_EXECUTABLE cargo REQUIRED)

# Cargo names the output directory for the `dev` profile "debug".
string(TOLOWER "${CMAKE_BUILD_TYPE}" LOG_SURGEON_BUILD_TYPE_LOWER)
if(LOG_SURGEON_BUILD_TYPE_LOWER STREQUAL "debug")
    set(LOG_SURGEON_CARGO_PROFILE "dev")
    set(LOG_SURGEON_CARGO_PROFILE_DIR "debug")
else()
    set(LOG_SURGEON_CARGO_PROFILE "release")
    set(LOG_SURGEON_CARGO_PROFILE_DIR "release")
endif()

set(LOG_SURGEON_RUST_STATIC_LIBRARY_NAME "liblog_surgeon.a")

set(LOG_SURGEON_RUST_STATIC_LIBRARY
    "${LOG_SURGEON_CARGO_TARGET_DIR}/${LOG_SURGEON_CARGO_PROFILE_DIR}/${LOG_SURGEON_RUST_STATIC_LIBRARY_NAME}"
)

file(GLOB_RECURSE LOG_SURGEON_RUST_SOURCES CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")
list(
    APPEND LOG_SURGEON_RUST_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/build.rs"
    "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
    "${CMAKE_CURRENT_SOURCE_DIR}/cbindgen.toml"
    "${CMAKE_CURRENT_SOURCE_DIR}/rust-toolchain.toml"
)
if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.lock")
    list(APPEND LOG_SURGEON_RUST_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.lock")
endif()

add_custom_command(
    OUTPUT
        "${LOG_SURGEON_RUST_STATIC_LIBRARY}"
    COMMAND
        "${LOG_SURGEON_CARGO_EXECUTABLE}" build --profile "${LOG_SURGEON_CARGO_PROFILE}"
        --target-dir "${LOG_SURGEON_CARGO_TARGET_DIR}"
    DEPENDS
        ${LOG_SURGEON_RUST_SOURCES}
    COMMENT "Building log-surgeon Rust static library (${LOG_SURGEON_CARGO_PROFILE})."
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    VERBATIM
)

add_custom_target(log_surgeon_rust_build DEPENDS "${LOG_SURGEON_RUST_STATIC_LIBRARY}")

add_library(log_surgeon_rust STATIC IMPORTED GLOBAL)
add_library(log_surgeon::rust ALIAS log_surgeon_rust)
set_target_properties(
    log_surgeon_rust
    PROPERTIES
        IMPORTED_LOCATION
            "${LOG_SURGEON_RUST_STATIC_LIBRARY}"
)
add_dependencies(log_surgeon_rust log_surgeon_rust_build)

find_package(Threads REQUIRED)

add_subdirectory(cxx)

# `log_surgeon` mirrors the target name of the C++ implementation,
# so consumers can switch to the Rust implementation without changing their link line.
add_library(log_surgeon INTERFACE)
add_library(log_surgeon::log_surgeon ALIAS log_surgeon)
target_link_libraries(
    log_surgeon
    INTERFACE
        log_surgeon_cxx
        log_surgeon_rust
        Threads::Threads
        ${CMAKE_DL_LIBS}
)

# Rust's static library depends on the C math library on these platforms.
if(UNIX AND NOT APPLE)
    target_link_libraries(log_surgeon INTERFACE m)
endif()

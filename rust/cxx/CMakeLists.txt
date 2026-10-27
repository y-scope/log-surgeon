# Header-only C++ wrapper around the Rust C ABI.
add_library(log_surgeon_cxx INTERFACE)
add_library(log_surgeon::cxx ALIAS log_surgeon_cxx)

target_include_directories(log_surgeon_cxx INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")

target_compile_features(log_surgeon_cxx INTERFACE cxx_std_20)

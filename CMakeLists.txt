cmake_minimum_required(VERSION 3.22.1)

project(
    log-surgeon
    VERSION 0.1.0
    DESCRIPTION "log-surgeon: A performant log parsing library"
    HOMEPAGE_URL https://github.com/y-scope/log-surgeon
    LANGUAGES
        CXX
)

if(POLICY CMP0077)
    cmake_policy(SET CMP0077 NEW)
endif()

include(CMakePackageConfigHelpers)
include(GNUInstallDirs)

option(BUILD_SHARED_LIBS "Build using shared libraries" OFF)
option(log_surgeon_BUILD_TESTING "Build the testing tree." ON)

set(CMAKE_EXPORT_COMPILE_COMMANDS
    ON
    CACHE BOOL
    "Enable/Disable output of compile commands during generation."
    FORCE
)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(default_build_type "Release")
    message(STATUS "No build type specified. Setting to '${default_build_type}'.")
    set(CMAKE_BUILD_TYPE "${default_build_type}" CACHE STRING "Choose the type of build." FORCE)
endif()

add_subdirectory(rust)

# The Rust CMake project uses underscore-separated target names.
# CLP and the previous C++ implementation use `log-surgeon::log-surgeon`.
add_library(log-surgeon::log-surgeon ALIAS log_surgeon)

get_target_property(LOG_SURGEON_RUST_STATIC_LIBRARY log_surgeon_rust IMPORTED_LOCATION)
get_filename_component(
    LOG_SURGEON_RUST_STATIC_LIBRARY_NAME
    "${LOG_SURGEON_RUST_STATIC_LIBRARY}"
    NAME
)

set(LOG_SURGEON_INSTALL_CONFIG_DIR "${CMAKE_INSTALL_LIBDIR}/cmake/log-surgeon")
set(LOG_SURGEON_INSTALL_INCLUDE_DIR "${CMAKE_INSTALL_INCLUDEDIR}")
set(LOG_SURGEON_INSTALL_LIBRARY_DIR "${CMAKE_INSTALL_LIBDIR}")

set(LOG_SURGEON_RUST_EXTRA_SYSTEM_LIBS "")
if(UNIX AND NOT APPLE)
    set(LOG_SURGEON_RUST_EXTRA_SYSTEM_LIBS ";m")
endif()

install(FILES "${LOG_SURGEON_RUST_STATIC_LIBRARY}" DESTINATION "${LOG_SURGEON_INSTALL_LIBRARY_DIR}")

install(
    DIRECTORY
        "${CMAKE_CURRENT_SOURCE_DIR}/rust/cxx/log_surgeon"
    DESTINATION "${LOG_SURGEON_INSTALL_INCLUDE_DIR}"
    FILES_MATCHING
    PATTERN "*.h"
    PATTERN "*.hpp"
    PATTERN "*.tpp"
)

configure_package_config_file(
    "${CMAKE_CURRENT_LIST_DIR}/cmake/log-surgeon-config.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/log-surgeon-config.cmake"
    INSTALL_DESTINATION "${LOG_SURGEON_INSTALL_CONFIG_DIR}"
    PATH_VARS
        LOG_SURGEON_INSTALL_INCLUDE_DIR
        LOG_SURGEON_INSTALL_LIBRARY_DIR
)

write_basic_package_version_file(
    "${CMAKE_CURRENT_BINARY_DIR}/log-surgeon-config-version.cmake"
    VERSION "${PROJECT_VERSION}"
    COMPATIBILITY SameMajorVersion
)

install(
    FILES
        "${CMAKE_CURRENT_BINARY_DIR}/log-surgeon-config.cmake"
        "${CMAKE_CURRENT_BINARY_DIR}/log-surgeon-config-version.cmake"
    DESTINATION "${LOG_SURGEON_INSTALL_CONFIG_DIR}"
)

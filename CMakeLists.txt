cmake_minimum_required(VERSION 3.5.1)

project(log_surgeon
    VERSION 0.0.1
    DESCRIPTION "log-surgeon: A performant log parsing library"
    HOMEPAGE_URL https://github.com/y-scope/log-surgeon
    LANGUAGES CXX
    )

if (POLICY CMP0077)
    cmake_policy(SET CMP0077 NEW)
endif()

if (CMAKE_PROJECT_NAME STREQUAL PROJECT_NAME AND BUILD_TESTING)
    find_package(Catch2 3 REQUIRED)
    include(Catch)
    include(CTest)
endif()

include(CMakePackageConfigHelpers)
include(GNUInstallDirs)
include(FetchContent)

find_package(Microsoft.GSL QUIET CONFIG)
if (NOT Microsoft.GSL_FOUND)
    FetchContent_Declare(GSL
                         GIT_REPOSITORY "https://github.com/microsoft/GSL"
                         GIT_TAG "v4.0.0"
                         GIT_SHALLOW ON
    )
endif()

# Use <fmt/*.h> as <format> is unsupported in gcc-10.
find_package(fmt 8.0.1 QUIET)
if (NOT fmt_FOUND)
    FetchContent_Declare(fmt
                         GIT_REPOSITORY https://github.com/fmtlib/fmt
                         GIT_TAG "8.0.1"
                         GIT_SHALLOW ON
    )
endif()

# Declare the details of all fetched content before making them available.
if (NOT Microsoft.GSL_FOUND)
    FetchContent_MakeAvailable(GSL)
endif()

if (NOT fmt_FOUND)
    # Force fmt to generate install rules
    set(FMT_INSTALL ON CACHE BOOL "Enable installation for fmt." FORCE)
    FetchContent_MakeAvailable(fmt)
endif()

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

if (NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(default_build_type "Release")
    message(STATUS "No build type specified. Setting to '${default_build_type}'.")
    set(CMAKE_BUILD_TYPE "${default_build_type}" CACHE STRING "Choose the type of build." FORCE)
endif()

option(BUILD_SHARED_LIBS "Build using shared libraries" OFF)

set(SOURCE_FILES
    src/log_surgeon/Buffer.hpp
    src/log_surgeon/BufferParser.cpp
    src/log_surgeon/BufferParser.hpp
    src/log_surgeon/Constants.hpp
    src/log_surgeon/FileReader.cpp
    src/log_surgeon/FileReader.hpp
    src/log_surgeon/finite_automata/Capture.hpp
    src/log_surgeon/finite_automata/Dfa.hpp
    src/log_surgeon/finite_automata/DfaState.hpp
    src/log_surgeon/finite_automata/DfaStatePair.hpp
    src/log_surgeon/finite_automata/Nfa.hpp
    src/log_surgeon/finite_automata/NfaSpontaneousTransition.hpp
    src/log_surgeon/finite_automata/NfaState.hpp
    src/log_surgeon/finite_automata/PrefixTree.cpp
    src/log_surgeon/finite_automata/PrefixTree.hpp
    src/log_surgeon/finite_automata/RegexAST.hpp
    src/log_surgeon/finite_automata/RegisterHandler.hpp
    src/log_surgeon/finite_automata/StateType.hpp
    src/log_surgeon/finite_automata/TagOperation.hpp
    src/log_surgeon/finite_automata/UnicodeIntervalTree.hpp
    src/log_surgeon/finite_automata/UnicodeIntervalTree.tpp
    src/log_surgeon/ItemSet.hpp
    src/log_surgeon/Lalr1Parser.hpp
    src/log_surgeon/Lalr1Parser.tpp
    src/log_surgeon/Lexer.hpp
    src/log_surgeon/Lexer.tpp
    src/log_surgeon/LexicalRule.hpp
    src/log_surgeon/LogEvent.cpp
    src/log_surgeon/LogEvent.hpp
    src/log_surgeon/LogParser.cpp
    src/log_surgeon/LogParser.hpp
    src/log_surgeon/LogParserOutputBuffer.cpp
    src/log_surgeon/LogParserOutputBuffer.hpp
    src/log_surgeon/NonTerminal.cpp
    src/log_surgeon/NonTerminal.hpp
    src/log_surgeon/Parser.tpp
    src/log_surgeon/Parser.hpp
    src/log_surgeon/ParserAst.cpp
    src/log_surgeon/ParserAst.hpp
    src/log_surgeon/ParserInputBuffer.cpp
    src/log_surgeon/ParserInputBuffer.hpp
    src/log_surgeon/Production.hpp
    src/log_surgeon/Reader.hpp
    src/log_surgeon/ReaderParser.cpp
    src/log_surgeon/ReaderParser.hpp
    src/log_surgeon/Schema.cpp
    src/log_surgeon/Schema.hpp
    src/log_surgeon/SchemaParser.cpp
    src/log_surgeon/SchemaParser.hpp
    src/log_surgeon/Token.cpp
    src/log_surgeon/Token.hpp
    src/log_surgeon/types.hpp
    src/log_surgeon/UniqueIdGenerator.hpp
    )

set(LCHIP_INSTALL_CONFIG_DIR ${CMAKE_INSTALL_LIBDIR}/cmake/log_surgeon)
set(LCHIP_INSTALL_INCLUDE_DIR ${CMAKE_INSTALL_INCLUDEDIR})
# Directory for installing third-party includes that the user doesn't have installed.
set(LCHIP_THIRD_PARTY_INCLUDE_DIR "${LCHIP_INSTALL_INCLUDE_DIR}/log_surgeon/third_party_include")

add_library(log_surgeon ${SOURCE_FILES})
add_library(log_surgeon::log_surgeon ALIAS log_surgeon)

if (Microsoft.GSL_FOUND)
    target_link_libraries(log_surgeon
            PUBLIC
            Microsoft.GSL::GSL
            )
else()
    # Since the user doesn't have GSL installed, use the GSL headers directly.
    # NOTE:
    # - We can't link against the `Microsoft.GSL::GSL` target since that would require adding `GSL`
    #   to the `install` command and force the user to have GSL installed when using log-surgeon.
    # - At install time, we'll copy GSL into log-surgeon's third-party includes directory.
    target_include_directories(log_surgeon
            PUBLIC
            $<BUILD_INTERFACE:${GSL_SOURCE_DIR}/include>
            $<INSTALL_INTERFACE:${LCHIP_THIRD_PARTY_INCLUDE_DIR}>
            )
endif()

target_link_libraries(log_surgeon PUBLIC fmt::fmt)

target_include_directories(log_surgeon
        PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>
        $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>
        PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/src
        )

target_compile_features(log_surgeon
    PRIVATE cxx_std_20
    )

target_compile_options(log_surgeon PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX>
    $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-Wall -Wextra -Wpedantic -Werror>
    )

# Macro providing the length of the absolute source directory path so we can
# create a relative (rather than absolute) __FILE__ macro
string(LENGTH "${CMAKE_SOURCE_DIR}/" SOURCE_PATH_SIZE)
target_compile_definitions(log_surgeon
    PUBLIC
    SOURCE_PATH_SIZE=${SOURCE_PATH_SIZE}
    )

# Make off_t 64-bit
target_compile_definitions(log_surgeon
    PRIVATE
    _FILE_OFFSET_BITS=64
    )

install(
    TARGETS
    log_surgeon
    EXPORT
    log_surgeon-targets
    )

install(
    EXPORT
    log_surgeon-targets
    NAMESPACE
    log_surgeon::
    DESTINATION
    ${LCHIP_INSTALL_CONFIG_DIR}
    )

install(
    DIRECTORY
    "${PROJECT_SOURCE_DIR}/src/log_surgeon"
    DESTINATION
    "${LCHIP_INSTALL_INCLUDE_DIR}"
    FILES_MATCHING
    PATTERN "*.h"
    PATTERN "*.hpp"
    PATTERN "*.tpp"
    )
if (NOT Microsoft.GSL_FOUND)
    install(
        DIRECTORY
        # NOTE: We don't include a trailing slash so that the gsl directory is copied rather than
        # its contents.
        "${GSL_SOURCE_DIR}/include/gsl"
        DESTINATION
        "${LCHIP_THIRD_PARTY_INCLUDE_DIR}"
        )
endif ()

configure_package_config_file(
    ${CMAKE_CURRENT_LIST_DIR}/cmake/log_surgeon-config.cmake.in
    ${CMAKE_CURRENT_BINARY_DIR}/log_surgeon-config.cmake
    INSTALL_DESTINATION
    ${LCHIP_INSTALL_CONFIG_DIR}
    PATH_VARS
    LCHIP_INSTALL_INCLUDE_DIR
    )

write_basic_package_version_file(
    ${CMAKE_CURRENT_BINARY_DIR}/log_surgeon-config-version.cmake
    COMPATIBILITY
    SameMajorVersion
    )

install(
    FILES
    ${CMAKE_CURRENT_BINARY_DIR}/log_surgeon-config.cmake
    ${CMAKE_CURRENT_BINARY_DIR}/log_surgeon-config-version.cmake
    DESTINATION
    ${LCHIP_INSTALL_CONFIG_DIR}
    )

if (CMAKE_PROJECT_NAME STREQUAL PROJECT_NAME AND BUILD_TESTING)
    add_subdirectory(tests)
endif()

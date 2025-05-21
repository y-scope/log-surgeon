cmake_minimum_required(VERSION 3.22.1)

project(log_surgeon
    VERSION 0.0.1
    DESCRIPTION "log-surgeon: A performant log parsing library"
    HOMEPAGE_URL https://github.com/y-scope/log-surgeon
    LANGUAGES CXX
    )

if (POLICY CMP0077)
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

if (NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(default_build_type "Release")
    message(STATUS "No build type specified. Setting to '${default_build_type}'.")
    set(CMAKE_BUILD_TYPE "${default_build_type}" CACHE STRING "Choose the type of build." FORCE)
endif()

if(log_surgeon_IS_TOP_LEVEL)
    # Include dependency settings if the project isn't being included as a subproject.
    # NOTE: We mark the file optional because if the user happens to have the dependencies
    # installed, this file is not necessary.
    include("${CMAKE_SOURCE_DIR}/build/deps/cmake-settings/all.cmake" OPTIONAL)

    # If previously undefined, `BUILD_TESTING` will be set to ON.
    include(CTest)
endif()

if(BUILD_TESTING AND log_surgeon_BUILD_TESTING)
    set(log_surgeon_ENABLE_TESTS ON)
endif()

# Use <fmt/*.h> as <format> is unsupported in gcc-10.
find_package(fmt 8.0.1 REQUIRED)
message(STATUS "Found fmt ${fmt_VERSION}.")

find_package(Microsoft.GSL 4.0.0 REQUIRED)
message(STATUS "Found Microsoft.GSL ${Microsoft.GSL_VERSION}.")

if(log_surgeon_ENABLE_TESTS)
    find_package(Catch2 3.8.1 REQUIRED)
    message(STATUS "Found Catch2 ${Catch2_VERSION}.")
    include(Catch)
endif()

set(SOURCE_FILES
    src/log_surgeon/Buffer.hpp
    src/log_surgeon/BufferParser.cpp
    src/log_surgeon/BufferParser.hpp
    src/log_surgeon/Constants.hpp
    src/log_surgeon/FileReader.cpp
    src/log_surgeon/FileReader.hpp
    src/log_surgeon/finite_automata/Capture.hpp
    src/log_surgeon/finite_automata/DeterminizationConfiguration.hpp
    src/log_surgeon/finite_automata/Dfa.hpp
    src/log_surgeon/finite_automata/DfaState.hpp
    src/log_surgeon/finite_automata/DfaStatePair.hpp
    src/log_surgeon/finite_automata/DfaTransition.hpp
    src/log_surgeon/finite_automata/Nfa.hpp
    src/log_surgeon/finite_automata/NfaSpontaneousTransition.hpp
    src/log_surgeon/finite_automata/NfaState.hpp
    src/log_surgeon/finite_automata/PrefixTree.cpp
    src/log_surgeon/finite_automata/PrefixTree.hpp
    src/log_surgeon/finite_automata/RegexAST.hpp
    src/log_surgeon/finite_automata/RegisterHandler.hpp
    src/log_surgeon/finite_automata/RegisterOperation.hpp
    src/log_surgeon/finite_automata/StateType.hpp
    src/log_surgeon/finite_automata/TagOperation.hpp
    src/log_surgeon/finite_automata/UnicodeIntervalTree.hpp
    src/log_surgeon/finite_automata/UnicodeIntervalTree.tpp
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
    src/log_surgeon/Parser.tpp
    src/log_surgeon/Parser.hpp
    src/log_surgeon/parser_types.cpp
    src/log_surgeon/parser_types.hpp
    src/log_surgeon/ParserAst.cpp
    src/log_surgeon/ParserAst.hpp
    src/log_surgeon/ParserInputBuffer.cpp
    src/log_surgeon/ParserInputBuffer.hpp
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

set(LOG_SURGEON_INSTALL_CONFIG_DIR ${CMAKE_INSTALL_LIBDIR}/cmake/log_surgeon)
set(LOG_SURGEON_INSTALL_INCLUDE_DIR ${CMAKE_INSTALL_INCLUDEDIR})

add_library(log_surgeon ${SOURCE_FILES})
add_library(log_surgeon::log_surgeon ALIAS log_surgeon)

target_link_libraries(log_surgeon
    PUBLIC
    fmt::fmt
    Microsoft.GSL::GSL
    )

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
    ${LOG_SURGEON_INSTALL_CONFIG_DIR}
    )

install(
    DIRECTORY
    "${PROJECT_SOURCE_DIR}/src/log_surgeon"
    DESTINATION
    "${LOG_SURGEON_INSTALL_INCLUDE_DIR}"
    FILES_MATCHING
    PATTERN "*.h"
    PATTERN "*.hpp"
    PATTERN "*.tpp"
    )

configure_package_config_file(
    ${CMAKE_CURRENT_LIST_DIR}/cmake/log_surgeon-config.cmake.in
    ${CMAKE_CURRENT_BINARY_DIR}/log_surgeon-config.cmake
    INSTALL_DESTINATION
    ${LOG_SURGEON_INSTALL_CONFIG_DIR}
    PATH_VARS
    LOG_SURGEON_INSTALL_INCLUDE_DIR
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
    ${LOG_SURGEON_INSTALL_CONFIG_DIR}
    )

if (log_surgeon_ENABLE_TESTS)
    add_subdirectory(tests)
endif()

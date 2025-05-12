cmake_minimum_required(VERSION 3.16)
project(log-surgeon-examples)

if (NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(default_build_type "Release")
    message(STATUS "No build type specified. Setting to '${default_build_type}'.")
    set(CMAKE_BUILD_TYPE "${default_build_type}" CACHE STRING "Choose the type of build." FORCE)
endif()

if(log-surgeon-examples_IS_TOP_LEVEL)
    # Include dependency settings if the project isn't being included as a subproject.
    # NOTE: We mark the file optional because if the user happens to have the dependencies
    # installed, this file is not necessary.
    include("../build/deps/cmake-settings/all.cmake" OPTIONAL)

    # If previously undefined, `BUILD_TESTING` will be set to ON.
    include(CTest)
endif()

find_package(log_surgeon REQUIRED)
if(log_surgeon_FOUND)
    message(STATUS "Found log_surgeon ${log_surgeon_VERSION}.")
endif()

function(add_to_target target libraries)
    target_link_libraries(${target} ${libraries})
    target_compile_features(${target} PRIVATE cxx_std_20)
    target_compile_options(${target} PRIVATE
        $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX>
        $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-Wall -Wextra -Wpedantic -Werror>
        )
endfunction()

add_library(common OBJECT common.cpp common.hpp)
add_to_target(common log_surgeon::log_surgeon)

list(APPEND libraries log_surgeon::log_surgeon common)

add_executable(buffer-parser buffer-parser.cpp)
add_to_target(buffer-parser "${libraries}")

add_executable(reader-parser reader-parser.cpp)
add_to_target(reader-parser "${libraries}")

add_executable(intersect-test intersect-test.cpp)
add_to_target(intersect-test "${libraries}")

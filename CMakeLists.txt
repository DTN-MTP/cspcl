cmake_minimum_required(VERSION 3.10)
project(cspcl VERSION 1.0.0 LANGUAGES C)

# C standard
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# Options
option(CSPCL_BUILD_TESTS "Build unit tests" ON)
option(CSPCL_USE_FREERTOS "Build for FreeRTOS (default: POSIX)" OFF)
option(CSPCL_DEBUG "Enable debug output" OFF)

# CSP library handling
# CSPCL requires CSP v1.6 (not v2.0 which has a different API)
# By default, use the included stubs for CSP v1.6 API
option(CSPCL_USE_SYSTEM_CSP "Use system-installed libcsp (must be v1.6)" ON)

set(CSP_FOUND FALSE)
set(USING_CSP_STUBS TRUE)

if(CSPCL_USE_SYSTEM_CSP)
    # Try pkg-config first
    find_package(PkgConfig QUIET)
    if(PKG_CONFIG_FOUND)
        pkg_check_modules(CSP libcsp)
    endif()

    # If not found via pkg-config, try manual search
    if(NOT CSP_FOUND)
        find_path(CSP_INCLUDE_DIRS csp/csp.h
            PATHS
                /usr/include
                /usr/local/include
                ${CMAKE_SOURCE_DIR}/../libcsp/include
                $ENV{CSP_PATH}/include
        )

        find_library(CSP_LIBRARIES csp
            PATHS
                /usr/lib
                /usr/local/lib
                ${CMAKE_SOURCE_DIR}/../libcsp/build
                $ENV{CSP_PATH}/lib
        )

        if(CSP_INCLUDE_DIRS AND CSP_LIBRARIES)
            set(CSP_FOUND TRUE)
            set(USING_CSP_STUBS FALSE)
        endif()
    else()
        set(USING_CSP_STUBS FALSE)
    endif()

    if(NOT CSP_FOUND)
        message(WARNING "System libcsp not found. Falling back to stubs.")
        set(CSP_INCLUDE_DIRS ${CMAKE_SOURCE_DIR}/stubs)
        set(CSP_LIBRARIES "")
        set(USING_CSP_STUBS TRUE)
    endif()
else()
    message(STATUS "Using CSP v1.6 stubs (set -DCSPCL_USE_SYSTEM_CSP=ON for system libcsp)")
    set(CSP_INCLUDE_DIRS ${CMAKE_SOURCE_DIR}/stubs)
    set(CSP_LIBRARIES "")
    set(USING_CSP_STUBS TRUE)
endif()

# CSPCL library
set(CSPCL_SOURCES src/cspcl.c)

# Add stub implementation when not using system CSP
if(USING_CSP_STUBS)
    list(APPEND CSPCL_SOURCES stubs/csp_stub.c)
endif()

add_library(cspcl ${CSPCL_SOURCES})

target_include_directories(cspcl
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_SOURCE_DIR}/src>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CSP_INCLUDE_DIRS}
)

target_link_libraries(cspcl
    PRIVATE
        ${CSP_LIBRARIES}
)

# Platform-specific settings
if(CSPCL_USE_FREERTOS)
    target_compile_definitions(cspcl PRIVATE FREERTOS)
else()
    target_compile_definitions(cspcl PRIVATE __linux__)
    target_link_libraries(cspcl PRIVATE rt)
endif()

# Debug settings
if(CSPCL_DEBUG)
    target_compile_definitions(cspcl PRIVATE CSPCL_DEBUG CSPCL_DEBUG_VERBOSE)
endif()

# Compiler warnings
target_compile_options(cspcl PRIVATE
    -Wall -Wextra -Wpedantic
    $<$<CONFIG:Debug>:-g -O0>
    $<$<CONFIG:Release>:-O2>
)

# Tests
if(CSPCL_BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()

# Installation
install(TARGETS cspcl
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
)

install(FILES
    src/cspcl.h
    src/cspcl_config.h
    DESTINATION include
)

# Package configuration
include(CMakePackageConfigHelpers)
write_basic_package_version_file(
    "${CMAKE_BINARY_DIR}/cspcl-config-version.cmake"
    VERSION ${PROJECT_VERSION}
    COMPATIBILITY SameMajorVersion
)

# Summary
message(STATUS "")
message(STATUS "CSPCL Configuration:")
message(STATUS "  Version:        ${PROJECT_VERSION}")
message(STATUS "  Using CSP stubs:${USING_CSP_STUBS}")
message(STATUS "  CSP includes:   ${CSP_INCLUDE_DIRS}")
message(STATUS "  CSP libraries:  ${CSP_LIBRARIES}")
message(STATUS "  Build tests:    ${CSPCL_BUILD_TESTS}")
message(STATUS "  Build examples: ${CSPCL_BUILD_EXAMPLES}")
message(STATUS "  FreeRTOS:       ${CSPCL_USE_FREERTOS}")
message(STATUS "  Debug:          ${CSPCL_DEBUG}")
message(STATUS "")


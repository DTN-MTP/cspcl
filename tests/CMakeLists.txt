cmake_minimum_required(VERSION 3.10)

# Test executable
add_executable(test_cspcl
    test_cspcl.c
)

target_include_directories(test_cspcl PRIVATE
    ${CMAKE_SOURCE_DIR}/src
    ${CSP_ALL_INCLUDE_DIRS}
)

target_link_libraries(test_cspcl PRIVATE
    cspcl
)

if(USING_CSP_STUBS)
    target_compile_definitions(test_cspcl PRIVATE USING_CSP_STUBS)
endif()

# Add test
add_test(NAME test_cspcl COMMAND test_cspcl)

# Integration test binary (LRU eviction, cache behavior)
add_executable(test_cspcl_pool_integration
    test_cspcl_pool_integration.c
)
target_include_directories(test_cspcl_pool_integration PRIVATE
    ${CMAKE_SOURCE_DIR}/src
    ${CSP_ALL_INCLUDE_DIRS}
)
target_link_libraries(test_cspcl_pool_integration PRIVATE cspcl)
add_test(NAME test_cspcl_pool_integration COMMAND test_cspcl_pool_integration)


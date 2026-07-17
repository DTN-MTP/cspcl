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

# Consolidation test binary (Phase 2-4 APIs)
add_executable(test_consolidation
    test_consolidation.c
)
target_include_directories(test_consolidation PRIVATE
    ${CMAKE_SOURCE_DIR}/src
    ${CSP_ALL_INCLUDE_DIRS}
)
target_link_libraries(test_consolidation PRIVATE cspcl)
add_test(NAME test_consolidation COMMAND test_consolidation)

# ACK failure detection demo
add_executable(test_ack_demo
    test_ack_demo.c
)
target_include_directories(test_ack_demo PRIVATE
    ${CMAKE_SOURCE_DIR}/src
    ${CSP_ALL_INCLUDE_DIRS}
)
target_link_libraries(test_ack_demo PRIVATE cspcl pthread)
add_test(NAME test_ack_demo COMMAND test_ack_demo)

# ACK timeout demonstration (shows real timeout logs)
add_executable(test_ack_timeout_demo
    test_ack_timeout_demo.c
)
target_include_directories(test_ack_timeout_demo PRIVATE
    ${CMAKE_SOURCE_DIR}/src
    ${CSP_ALL_INCLUDE_DIRS}
)
target_link_libraries(test_ack_timeout_demo PRIVATE cspcl)
add_test(NAME test_ack_timeout_demo COMMAND test_ack_timeout_demo)


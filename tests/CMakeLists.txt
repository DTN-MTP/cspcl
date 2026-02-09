cmake_minimum_required(VERSION 3.10)

# Test executable
add_executable(test_cspcl
    test_cspcl.c
)

target_include_directories(test_cspcl PRIVATE
    ${CMAKE_SOURCE_DIR}/src
    ${CMAKE_SOURCE_DIR}/stubs
)

target_link_libraries(test_cspcl PRIVATE
    cspcl
)

# Add test
add_test(NAME test_cspcl COMMAND test_cspcl)


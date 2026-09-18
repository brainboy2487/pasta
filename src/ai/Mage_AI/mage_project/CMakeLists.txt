# MAGE CMake Build Configuration
# Generated: 2026-01-23 15:32:35

cmake_minimum_required(VERSION 3.10)
project(MAGE VERSION 1.0.0 LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# Compiler options
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

set(CMAKE_C_FLAGS_RELEASE "-O3 -march=native -DNDEBUG")
set(CMAKE_C_FLAGS_DEBUG "-g -O0 -DDEBUG")
set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIC")

# Include directories
include_directories(${PROJECT_SOURCE_DIR}/include)

# Source files
file(GLOB_RECURSE CORE_SOURCES src/core/*.c)
file(GLOB_RECURSE GEN_SOURCES src/generators/*.c)
file(GLOB_RECURSE IO_SOURCES src/io/*.c)
file(GLOB_RECURSE UTIL_SOURCES src/utils/*.c)

set(ALL_SOURCES ${CORE_SOURCES} ${GEN_SOURCES} ${IO_SOURCES} ${UTIL_SOURCES})

# Shared library
add_library(mage SHARED ${ALL_SOURCES})
target_include_directories(mage PUBLIC ${PROJECT_SOURCE_DIR}/include)
target_link_libraries(mage m pthread)

# CLI executable
add_executable(mage_cli src/mage_cli.c)
target_link_libraries(mage_cli mage)

# Tests
enable_testing()
add_executable(test_runner tests/test_runner.c)
target_link_libraries(test_runner mage)
add_test(NAME UnitTests COMMAND test_runner)

add_executable(test_mphf tests/unit/test_mphf.c)
target_link_libraries(test_mphf PRIVATE mage)
add_test(NAME MphfTest COMMAND test_mphf)

add_executable(tap_test tests/tap_test.c src/core/tap.c)
target_link_libraries(tap_test mage)
add_test(NAME TapTest COMMAND tap_test)


# Benchmarks
add_executable(benchmark_suite tests/benchmarks/benchmark_suite.c)
target_link_libraries(benchmark_suite mage)

# Installation
install(TARGETS mage mage_cli
        LIBRARY DESTINATION lib
        ARCHIVE DESTINATION lib
        RUNTIME DESTINATION bin)

install(DIRECTORY include/mage DESTINATION include)

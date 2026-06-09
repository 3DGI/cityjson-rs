cmake_minimum_required(VERSION 3.20)

project(cityjson_lib_cpp VERSION 0.10.0 LANGUAGES CXX)

include(CMakePackageConfigHelpers)
include(CTest)
include(GNUInstallDirs)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(CITYJSON_LIB_ROOT_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../.." CACHE PATH "Repository root")
set(CITYJSON_LIB_FFI_CORE_INCLUDE_DIR "${CITYJSON_LIB_ROOT_DIR}/ffi/core/include" CACHE PATH "Generated cityjson_lib C ABI headers")
set(CITYJSON_LIB_FFI_CORE_SHARED_LIB "${CITYJSON_LIB_ROOT_DIR}/target/debug/libcityjson_lib_ffi_core.so" CACHE FILEPATH "Built cityjson_lib ffi shared library")
get_filename_component(CITYJSON_LIB_FFI_CORE_SHARED_LIB_NAME "${CITYJSON_LIB_FFI_CORE_SHARED_LIB}" NAME)

if(NOT EXISTS "${CITYJSON_LIB_FFI_CORE_INCLUDE_DIR}/cityjson_lib/cityjson_lib.h")
  message(FATAL_ERROR "Missing generated C ABI header at ${CITYJSON_LIB_FFI_CORE_INCLUDE_DIR}/cityjson_lib/cityjson_lib.h. Run `just ffi build header` first.")
endif()

if(NOT EXISTS "${CITYJSON_LIB_FFI_CORE_SHARED_LIB}")
  message(FATAL_ERROR "Missing built cityjson_lib ffi shared library at ${CITYJSON_LIB_FFI_CORE_SHARED_LIB}. Build `cityjson-lib-ffi-core` first.")
endif()

add_library(cityjson_lib_ffi_core SHARED IMPORTED GLOBAL)
set_target_properties(
  cityjson_lib_ffi_core
  PROPERTIES
    IMPORTED_LOCATION "${CITYJSON_LIB_FFI_CORE_SHARED_LIB}"
)

add_library(cityjson_lib_cpp INTERFACE)
target_compile_features(cityjson_lib_cpp INTERFACE cxx_std_20)
target_compile_definitions(cityjson_lib_cpp INTERFACE CITYJSON_LIB_NATIVE_FORMATS)
target_include_directories(
  cityjson_lib_cpp
  INTERFACE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
    "${CITYJSON_LIB_FFI_CORE_INCLUDE_DIR}"
)
target_link_libraries(cityjson_lib_cpp INTERFACE cityjson_lib_ffi_core)

install(TARGETS cityjson_lib_cpp)
install(FILES
  "${CITYJSON_LIB_FFI_CORE_SHARED_LIB}"
  DESTINATION "${CMAKE_INSTALL_LIBDIR}"
)
install(DIRECTORY include/ DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")
install(DIRECTORY "${CITYJSON_LIB_FFI_CORE_INCLUDE_DIR}/cityjson_lib"
  DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}"
)

configure_package_config_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/cmake/cityjson_lib_cppConfig.cmake.in"
  "${CMAKE_CURRENT_BINARY_DIR}/cityjson_lib_cppConfig.cmake"
  INSTALL_DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/cityjson_lib_cpp"
)
write_basic_package_version_file(
  "${CMAKE_CURRENT_BINARY_DIR}/cityjson_lib_cppConfigVersion.cmake"
  VERSION "${PROJECT_VERSION}"
  COMPATIBILITY SameMajorVersion
)
install(FILES
  "${CMAKE_CURRENT_BINARY_DIR}/cityjson_lib_cppConfig.cmake"
  "${CMAKE_CURRENT_BINARY_DIR}/cityjson_lib_cppConfigVersion.cmake"
  DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/cityjson_lib_cpp"
)

if(BUILD_TESTING)
  add_executable(cityjson_lib_cpp_smoke tests/smoke.cpp)
  target_link_libraries(cityjson_lib_cpp_smoke PRIVATE cityjson_lib_cpp)
  target_compile_definitions(
    cityjson_lib_cpp_smoke
    PRIVATE
      CITYJSON_LIB_FIXTURE_PATH="${CITYJSON_LIB_ROOT_DIR}/tests/data/v2_0/minimal.city.json"
  )
  get_filename_component(CITYJSON_LIB_FFI_CORE_LIB_DIR "${CITYJSON_LIB_FFI_CORE_SHARED_LIB}" DIRECTORY)
  set_target_properties(cityjson_lib_cpp_smoke PROPERTIES BUILD_RPATH "${CITYJSON_LIB_FFI_CORE_LIB_DIR}")
  add_test(NAME cityjson_lib_cpp_smoke COMMAND cityjson_lib_cpp_smoke)

  add_executable(cityjson_lib_cpp_ffi_visibility benchmarks/ffi_visibility.cpp)
  target_link_libraries(cityjson_lib_cpp_ffi_visibility PRIVATE cityjson_lib_cpp)
  target_compile_definitions(
    cityjson_lib_cpp_ffi_visibility
    PRIVATE
      CITYJSON_LIB_SMALL_FIXTURE_PATH="${CITYJSON_LIB_ROOT_DIR}/tests/data/v2_0/minimal.city.json"
      CITYJSON_LIB_LARGE_FIXTURE_PATH="${CITYJSON_LIB_ROOT_DIR}/tests/data/v2_0/cityjson_fake_complete.city.json"
  )
  set_target_properties(
    cityjson_lib_cpp_ffi_visibility
    PROPERTIES
      BUILD_RPATH "${CITYJSON_LIB_FFI_CORE_LIB_DIR}"
  )

  add_executable(cityjson_lib_cpp_fake_complete examples/fake_complete.cpp)
  target_link_libraries(cityjson_lib_cpp_fake_complete PRIVATE cityjson_lib_cpp)
  set_target_properties(
    cityjson_lib_cpp_fake_complete
    PROPERTIES
      BUILD_RPATH "${CITYJSON_LIB_FFI_CORE_LIB_DIR}"
  )
endif()

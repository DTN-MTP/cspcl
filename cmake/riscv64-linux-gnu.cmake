# CMake toolchain for cross-compiling to 64-bit RISC-V Linux (e.g. the
# PolarFire SoC U54 application cores, RV64GC / lp64d ABI).
#
# Binaries are linked statically so they run on any riscv64 Linux image
# regardless of which shared libraries (or glibc version) it ships.
#
# Usage:
#   cmake -B build-riscv64 -DCMAKE_TOOLCHAIN_FILE=cmake/riscv64-linux-gnu.cmake ...
#
# Requires: gcc-riscv64-linux-gnu, g++-riscv64-linux-gnu
# Optional: qemu-user-static (lets ctest run the test binaries on the host)

set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR riscv64)

set(CMAKE_C_COMPILER riscv64-linux-gnu-gcc)
set(CMAKE_CXX_COMPILER riscv64-linux-gnu-g++)

set(CMAKE_EXE_LINKER_FLAGS_INIT "-static")

find_program(QEMU_RISCV64 NAMES qemu-riscv64-static qemu-riscv64)
if(QEMU_RISCV64)
    set(CMAKE_CROSSCOMPILING_EMULATOR ${QEMU_RISCV64})
endif()

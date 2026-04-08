#!/bin/bash
#
# format-code.sh - Format all C code according to project style
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=============================================="
echo "  CSPCL Code Formatter"
echo "=============================================="
echo ""

# Check if clang-format is installed
if ! command -v clang-format &> /dev/null; then
    echo "ERROR: clang-format not found"
    echo "Install with: sudo apt-get install clang-format"
    exit 1
fi

# Check if black is installed for Python
if ! command -v black &> /dev/null; then
    echo "WARNING: black not found (Python formatter)"
    echo "Install with: pip install black"
    PYTHON_FORMAT=0
else
    PYTHON_FORMAT=1
fi

# Format C code
echo "[1/3] Formatting C code..."
find src/ tests/ ud3tn-integration/ unibo-integration/ \
    -type f \( -name "*.c" -o -name "*.h" \) \
    -not -path "*/build/*" \
    -exec clang-format -i {} \;
echo "✓ C code formatted"
echo ""

# Format Python code
if [ $PYTHON_FORMAT -eq 1 ]; then
    echo "[2/3] Formatting Python code..."
    black tools/ --line-length 100 --quiet
    echo "✓ Python code formatted"
else
    echo "[2/3] Skipping Python formatting (black not installed)"
fi
echo ""

# Format shell scripts (basic cleanup)
echo "[3/3] Cleaning up shell scripts..."
find . -type f -name "*.sh" -not -path "*/build/*" | while read -r script; do
    # Remove trailing whitespace
    sed -i 's/[[:space:]]*$//' "$script"
done
echo "✓ Shell scripts cleaned"
echo ""

echo "=============================================="
echo "  Formatting Complete"
echo "=============================================="
echo ""
echo "Next steps:"
echo "  1. Review changes: git diff"
echo "  2. Test build: mkdir -p build && cd build && cmake .. && make"
echo "  3. Run tests: cd build && ctest"
echo "  4. Commit: git add -u && git commit -m 'Apply code formatting'"
echo ""

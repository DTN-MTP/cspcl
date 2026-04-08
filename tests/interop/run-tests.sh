#!/bin/bash
#
# run-tests.sh - Master test runner for CSPCL Docker integration tests
# Orchestrates Docker Compose startup and runs all interoperability tests
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DOCKER_DIR="${PROJECT_ROOT}/docker"
TESTS_DIR="${SCRIPT_DIR}"

# Default configuration
TRANSPORT="zmqhub"
TEST_SUITE="all"
INTERACTIVE=0
KEEP_RUNNING=0
BUILD_IMAGES=1

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_usage() {
    cat <<EOF
CSPCL Docker Integration Test Runner

Usage: $(basename "$0") [OPTIONS]

Options:
  --transport TYPE     Transport type: zmqhub (default) or can
  --test SUITE         Test suite: all (default), ud3tn, unibo, cross
  --interactive        Start services and enter interactive mode (no tests)
  --keep-running       Keep services running after tests complete
  --no-build           Skip building Docker images
  -h, --help           Show this help message

Examples:
  $(basename "$0")                          # Run all tests with ZMQHUB
  $(basename "$0") --transport can          # Run all tests with CAN
  $(basename "$0") --test ud3tn             # Run only uD3TN tests
  $(basename "$0") --interactive            # Start services for manual testing
  $(basename "$0") --keep-running           # Run tests but keep services up

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --transport)
            TRANSPORT="$2"
            shift 2
            ;;
        --test)
            TEST_SUITE="$2"
            shift 2
            ;;
        --interactive)
            INTERACTIVE=1
            shift
            ;;
        --keep-running)
            KEEP_RUNNING=1
            shift
            ;;
        --no-build)
            BUILD_IMAGES=0
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

# Validate transport
if [ "$TRANSPORT" != "zmqhub" ] && [ "$TRANSPORT" != "can" ]; then
    echo -e "${RED}Error: Invalid transport '$TRANSPORT'. Must be 'zmqhub' or 'can'${NC}"
    exit 1
fi

# Validate test suite
if [ "$TEST_SUITE" != "all" ] && [ "$TEST_SUITE" != "ud3tn" ] && \
   [ "$TEST_SUITE" != "unibo" ] && [ "$TEST_SUITE" != "cross" ]; then
    echo -e "${RED}Error: Invalid test suite '$TEST_SUITE'${NC}"
    exit 1
fi

COMPOSE_FILE="${DOCKER_DIR}/docker-compose.${TRANSPORT}.yml"

# Prefer docker compose v2, but keep compatibility with docker-compose v1
if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
elif docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
else
    echo -e "${RED}Error: Docker Compose not found (need docker compose v2 or docker-compose v1)${NC}"
    exit 1
fi

compose() {
    "${COMPOSE_CMD[@]}" "$@"
}

echo -e "${BLUE}=================================================="
echo "  CSPCL Docker Integration Test Runner"
echo "=================================================="
echo -e "Transport:     ${YELLOW}${TRANSPORT}${NC}"
echo -e "Test Suite:    ${YELLOW}${TEST_SUITE}${NC}"
echo -e "Compose File:  ${COMPOSE_FILE}"
echo -e "${BLUE}==================================================${NC}"
echo ""

# Change to project root for Docker context
cd "${PROJECT_ROOT}"

# Build base image first if needed
if [ $BUILD_IMAGES -eq 1 ]; then
    echo -e "${BLUE}[1/4] Building Docker images...${NC}"
    docker build -t cspcl-base:latest -f docker/base/Dockerfile .
    compose -f "${COMPOSE_FILE}" build
    echo -e "${GREEN}✓ Images built${NC}"
    echo ""
else
    echo -e "${YELLOW}[1/4] Skipping image build${NC}"
    echo ""
fi

# Start services
echo -e "${BLUE}[2/4] Starting Docker Compose services...${NC}"
compose -f "${COMPOSE_FILE}" up -d

# Wait for services to be healthy
echo -e "${BLUE}[3/4] Waiting for services to be healthy...${NC}"
sleep 10

# Check health
HEALTHY=1
for service in $(compose -f "${COMPOSE_FILE}" config --services); do
    if ! compose -f "${COMPOSE_FILE}" ps "$service" | grep -q "Up"; then
        echo -e "${RED}✗ Service $service is not running${NC}"
        HEALTHY=0
    fi
done

if [ $HEALTHY -eq 0 ]; then
    echo -e "${RED}Some services failed to start. Check logs:${NC}"
    compose -f "${COMPOSE_FILE}" logs
    compose -f "${COMPOSE_FILE}" down
    exit 1
fi

echo -e "${GREEN}✓ All services are running${NC}"
echo ""

# Interactive mode
if [ $INTERACTIVE -eq 1 ]; then
    echo -e "${BLUE}=================================================="
    echo "  Interactive Mode"
    echo "=================================================="
    echo "All services are running. Use these commands:"
    echo ""
    echo "View logs:"
    echo "  ${COMPOSE_CMD[*]} -f ${COMPOSE_FILE} logs -f [service]"
    echo ""
    echo "Execute commands:"
    echo "  docker exec -it cspcl-ud3tn-node-a /bin/bash"
    echo "  docker exec -it cspcl-unibo-node-1 /bin/bash"
    echo ""
    echo "Run tests manually:"
    echo "  ${TESTS_DIR}/test-ud3tn-basic.sh"
    echo "  ${TESTS_DIR}/test-unibo-basic.sh"
    echo "  ${TESTS_DIR}/test-cross-integration.sh"
    echo ""
    echo "Stop services:"
    echo "  ${COMPOSE_CMD[*]} -f ${COMPOSE_FILE} down"
    echo -e "${BLUE}==================================================${NC}"
    echo ""
    echo "Press Ctrl+C to stop services and exit"

    trap 'compose -f "$COMPOSE_FILE" down; exit 0' INT TERM
    tail -f /dev/null
fi

# Run tests
echo -e "${BLUE}[4/4] Running tests...${NC}"
echo ""

TEST_RESULTS=()
FAILED_TESTS=()

run_test() {
    local test_name=$1
    local test_script=$2

    echo -e "${BLUE}Running: ${test_name}${NC}"
    if bash "${test_script}"; then
        echo -e "${GREEN}✓ PASSED: ${test_name}${NC}"
        TEST_RESULTS+=("PASS")
        return 0
    else
        echo -e "${RED}✗ FAILED: ${test_name}${NC}"
        TEST_RESULTS+=("FAIL")
        FAILED_TESTS+=("${test_name}")
        return 1
    fi
}

# Execute test suite
case "$TEST_SUITE" in
    all)
        run_test "uD3TN Basic" "${TESTS_DIR}/test-ud3tn-basic.sh" || true
        run_test "Unibo-BP Basic" "${TESTS_DIR}/test-unibo-basic.sh" || true
        run_test "Cross-Integration" "${TESTS_DIR}/test-cross-integration.sh" || true
        ;;
    ud3tn)
        run_test "uD3TN Basic" "${TESTS_DIR}/test-ud3tn-basic.sh" || true
        ;;
    unibo)
        run_test "Unibo-BP Basic" "${TESTS_DIR}/test-unibo-basic.sh" || true
        ;;
    cross)
        run_test "Cross-Integration" "${TESTS_DIR}/test-cross-integration.sh" || true
        ;;
esac

# Summary
echo ""
echo -e "${BLUE}=================================================="
echo "  Test Summary"
echo "=================================================="

PASSED=0
FAILED=0
for result in "${TEST_RESULTS[@]}"; do
    if [ "$result" == "PASS" ]; then
        ((PASSED++))
    else
        ((FAILED++))
    fi
done

echo -e "Total Tests: ${PASSED + FAILED}"
echo -e "${GREEN}Passed: ${PASSED}${NC}"
echo -e "${RED}Failed: ${FAILED}${NC}"

if [ ${#FAILED_TESTS[@]} -gt 0 ]; then
    echo ""
    echo -e "${RED}Failed tests:${NC}"
    for test in "${FAILED_TESTS[@]}"; do
        echo -e "  - $test"
    done
fi

echo -e "${BLUE}==================================================${NC}"
echo ""

# Cleanup or keep running
if [ $KEEP_RUNNING -eq 1 ]; then
    echo -e "${YELLOW}Services kept running. Stop with:${NC}"
    echo "  ${COMPOSE_CMD[*]} -f ${COMPOSE_FILE} down"
else
    echo -e "${BLUE}Stopping services...${NC}"
    compose -f "${COMPOSE_FILE}" down
    echo -e "${GREEN}✓ Cleanup complete${NC}"
fi

# Exit with appropriate code
if [ $FAILED -eq 0 ]; then
    exit 0
else
    exit 1
fi

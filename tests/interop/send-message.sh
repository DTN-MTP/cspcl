#!/bin/bash
#
# send-message.sh - Run message exchange scenarios over CSPCL using Docker stack
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

SCENARIO="all"
TRANSPORT="zmqhub"
START_STACK=0
PREPARE_HOST_VCAN=0

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  --scenario NAME        ud3tn | unibo | cross | all (default)
  --transport TYPE       zmqhub (default) or can
  --start-stack          Start the Docker stack before sending messages
  --prepare-host-vcan    Also prepare host vcan0 (used with --transport can)
  -h, --help             Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --scenario)
            SCENARIO="$2"
            shift 2
            ;;
        --transport)
            TRANSPORT="$2"
            shift 2
            ;;
        --start-stack)
            START_STACK=1
            shift
            ;;
        --prepare-host-vcan)
            PREPARE_HOST_VCAN=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

if [[ "${SCENARIO}" != "ud3tn" && "${SCENARIO}" != "unibo" && "${SCENARIO}" != "cross" && "${SCENARIO}" != "all" ]]; then
    echo "ERROR: invalid scenario '${SCENARIO}'"
    exit 1
fi

if [[ "${TRANSPORT}" != "zmqhub" && "${TRANSPORT}" != "can" ]]; then
    echo "ERROR: invalid transport '${TRANSPORT}'"
    exit 1
fi

if [[ ${START_STACK} -eq 1 ]]; then
    STACK_ARGS=(--transport "${TRANSPORT}" --no-build)
    if [[ "${TRANSPORT}" == "can" && ${PREPARE_HOST_VCAN} -eq 1 ]]; then
        STACK_ARGS+=(--prepare-host-vcan)
    fi
    "${SCRIPT_DIR}/stack-up.sh" "${STACK_ARGS[@]}"
fi

run_case() {
    local label="$1"
    local script="$2"
    echo "=================================================="
    echo "Scenario: ${label} (transport=${TRANSPORT})"
    echo "=================================================="
    "${script}"
}

case "${SCENARIO}" in
    ud3tn)
        run_case "uD3TN -> uD3TN" "${SCRIPT_DIR}/test-ud3tn-basic.sh"
        ;;
    unibo)
        run_case "Unibo -> Unibo" "${SCRIPT_DIR}/test-unibo-basic.sh"
        ;;
    cross)
        run_case "uD3TN <-> Unibo" "${SCRIPT_DIR}/test-cross-integration.sh"
        ;;
    all)
        run_case "uD3TN -> uD3TN" "${SCRIPT_DIR}/test-ud3tn-basic.sh"
        run_case "Unibo -> Unibo" "${SCRIPT_DIR}/test-unibo-basic.sh"
        run_case "uD3TN <-> Unibo" "${SCRIPT_DIR}/test-cross-integration.sh"
        ;;
esac

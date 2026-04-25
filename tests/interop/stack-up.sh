#!/bin/bash
#
# stack-up.sh - Start CSPCL Docker DTN stack for a selected transport
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DOCKER_DIR="${PROJECT_ROOT}/docker"

TRANSPORT="zmqhub"
BUILD_IMAGES=1
PREPARE_HOST_VCAN=0

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  --transport TYPE       zmqhub (default) or can
  --no-build             Skip docker image build
  --prepare-host-vcan    Prepare host vcan0 (recommended for CI + can)
  -h, --help             Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --transport)
            TRANSPORT="$2"
            shift 2
            ;;
        --no-build)
            BUILD_IMAGES=0
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

if [[ "${TRANSPORT}" != "zmqhub" && "${TRANSPORT}" != "can" ]]; then
    echo "ERROR: invalid transport '${TRANSPORT}'"
    exit 1
fi

if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
elif docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
else
    echo "ERROR: Docker Compose not found"
    exit 1
fi

compose() {
    "${COMPOSE_CMD[@]}" "$@"
}

COMPOSE_FILE="${DOCKER_DIR}/docker-compose.${TRANSPORT}.yml"

if [[ "${TRANSPORT}" == "can" && ${PREPARE_HOST_VCAN} -eq 1 ]]; then
    "${SCRIPT_DIR}/setup-vcan-host.sh"
fi

cd "${PROJECT_ROOT}"

if [[ ${BUILD_IMAGES} -eq 1 ]]; then
    docker build -t cspcl-base:latest -f docker/base/Dockerfile .
    compose -f "${COMPOSE_FILE}" build
fi

compose -f "${COMPOSE_FILE}" up -d
compose -f "${COMPOSE_FILE}" ps

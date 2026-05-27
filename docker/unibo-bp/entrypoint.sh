#!/bin/bash
#
# Unibo-BP entrypoint script for Docker deployment
# Configures and starts Unibo-BP with CSPCL daemon
#

set -e

# Configuration from environment variables
CSP_ADDR=${CSP_ADDR:-1}
CSP_PORT=${CSP_PORT:-10}
TRANSPORT=${TRANSPORT:-zmqhub}
UNIBO_SOCKET=${UNIBO_SOCKET:-2001}
UNIBO_NODE_DIR=${UNIBO_NODE_DIR:-/tmp/unibo-node}
UNIBO_ADMIN_EID=${UNIBO_ADMIN_EID:-ipn:1.0}
UNIBO_ADMIN_DTN_EID=${UNIBO_ADMIN_DTN_EID:-dtn://a.dtn/}

echo "=================================================="
echo "  Unibo-BP with CSPCL - Docker Container"
echo "=================================================="
echo "CSP Address:       ${CSP_ADDR}"
echo "CSP Port:          ${CSP_PORT}"
echo "Transport:         ${TRANSPORT}"
echo "Unibo Socket:      ${UNIBO_SOCKET}"
echo "Node Directory:    ${UNIBO_NODE_DIR}"
echo "Admin EID (IPN):   ${UNIBO_ADMIN_EID}"
echo "Admin EID (DTN):   ${UNIBO_ADMIN_DTN_EID}"
echo "=================================================="

# Ensure node directory exists and is clean
mkdir -p "${UNIBO_NODE_DIR}"
find "${UNIBO_NODE_DIR}" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
cd "${UNIBO_NODE_DIR}"

# Clean up any previous state
"${UNIBO_BP_BIN}/unibo-bp-admin" stop 2>/dev/null || true
sleep 1

# Wait for ZMQ broker if using ZMQHUB transport
if [ "$TRANSPORT" = "zmqhub" ]; then
    if [ -n "$ZMQ_BROKER_HOST" ]; then
        echo "Waiting for ZMQ broker at ${ZMQ_BROKER_HOST}:6000..."
        timeout=30
        while ! nc -z "${ZMQ_BROKER_HOST}" 6000 2>/dev/null; do
            timeout=$((timeout - 1))
            if [ "$timeout" -le 0 ]; then
                echo "ERROR: ZMQ broker not available after 30 seconds"
                exit 1
            fi
            sleep 1
        done
        echo "ZMQ broker is ready"
    fi
fi

# Start Unibo-BP daemon
echo "Starting Unibo-BP daemon..."
"${UNIBO_BP_BIN}/unibo-bp" start \
    --set-storage-size 50000000 \
    --dtn-admin "${UNIBO_ADMIN_DTN_EID}" \
    --ipn-admin "${UNIBO_ADMIN_EID}" \
    --daemon

# Wait for daemon to be ready
sleep 2

# Start CSPCL daemon
# Build interface spec for cspcl_daemon:
#   zmqhub[:host] | can[:iface] | loopback
IFACE_SPEC="${TRANSPORT}"
if [ "${TRANSPORT}" = "zmqhub" ] && [ -n "${ZMQ_BROKER_HOST}" ]; then
    IFACE_SPEC="zmqhub:${ZMQ_BROKER_HOST}"
fi

echo "Starting CSPCL daemon..."
exec stdbuf -oL -eL "${UNIBO_BP_BIN}/unibo-bp-cspcl" \
    "${CSP_ADDR}" \
    "${CSP_PORT}" \
    "${IFACE_SPEC}" \
    "${UNIBO_SOCKET}" \
    "${UNIBO_NODE_DIR}"

#!/bin/bash
#
# picoD3TN entrypoint script for Docker deployment
# Configures and starts picoD3TN with the cspcl convergence layer
#

set -e

# Configuration from environment variables
CSP_ADDR=${CSP_ADDR:-5}
CSP_PORT=${CSP_PORT:-10}
TRANSPORT=${TRANSPORT:-zmqhub}
PICOD3TN_LOG_LEVEL=${PICOD3TN_LOG_LEVEL:-info}

# picoD3TN reads its configuration from PICOD3TN_* variables
export PICOD3TN_NODE_ID=${PICOD3TN_NODE_ID:-dtn://e.dtn/}
export PICOD3TN_AAP2_LISTEN=${PICOD3TN_AAP2_LISTEN:-unix:/var/run/picod3tn/aap2.sock}
# Loopback only: nodes sharing a network namespace (CAN stack) set distinct ports
export PICOD3TN_MTCPCL_LISTEN_ADDRESS=${PICOD3TN_MTCPCL_LISTEN_ADDRESS:-127.0.0.1:4224}
export PICOD3TN_CSPCL_LOCAL_ADDR="${CSP_ADDR}"
export PICOD3TN_CSPCL_PORT="${CSP_PORT}"
export RUST_LOG=${RUST_LOG:-${PICOD3TN_LOG_LEVEL}}

case "$TRANSPORT" in
    zmqhub) export PICOD3TN_CSPCL_INTERFACE="zmqhub:${ZMQ_BROKER_HOST:-localhost}" ;;
    can) export PICOD3TN_CSPCL_INTERFACE="can:vcan0" ;;
    loopback) export PICOD3TN_CSPCL_INTERFACE="loopback" ;;
    *)
        echo "ERROR: unknown TRANSPORT '${TRANSPORT}' (zmqhub, can or loopback)"
        exit 1
        ;;
esac

echo "=================================================="
echo "  picoD3TN with CSPCL - Docker Container"
echo "=================================================="
echo "CSP Address:    ${CSP_ADDR}"
echo "CSP Port:       ${CSP_PORT}"
echo "Transport:      ${TRANSPORT} (${PICOD3TN_CSPCL_INTERFACE})"
echo "Node ID:        ${PICOD3TN_NODE_ID}"
echo "AAP2:           ${PICOD3TN_AAP2_LISTEN}"
echo "=================================================="

# Clean up any previous socket file
rm -f /var/run/picod3tn/aap2.sock

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

echo "Starting picoD3TN..."
cd /var/run/picod3tn
exec /opt/picod3tn/bin/picod3tn "$@"

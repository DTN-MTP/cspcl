#!/bin/bash
#
# uD3TN entrypoint script for Docker deployment
# Configures and starts uD3TN with CSP convergence layer adapter
#

set -e

# Configuration from environment variables
CSP_ADDR=${CSP_ADDR:-1}
CSP_PORT=${CSP_PORT:-10}
TRANSPORT=${TRANSPORT:-zmqhub}
UD3TN_EID=${UD3TN_EID:-dtn://node.dtn/}

echo "=================================================="
echo "  uD3TN with CSPCL - Docker Container"
echo "=================================================="
echo "CSP Address:    ${CSP_ADDR}"
echo "CSP Port:       ${CSP_PORT}"
echo "Transport:      ${TRANSPORT}"
echo "uD3TN EID:      ${UD3TN_EID}"
echo "=================================================="

# Clean up any previous socket files
rm -f /tmp/*.aap2.socket
rm -f /var/run/ud3tn/*.aap2.socket

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

# Start uD3TN with CSP CLA
echo "Starting uD3TN..."
cd /var/run/ud3tn

exec /opt/ud3tn-src/build/posix/ud3tn \
    --eid "${UD3TN_EID}" \
    --cla "csp:${CSP_ADDR},${CSP_PORT}" \
    "$@"

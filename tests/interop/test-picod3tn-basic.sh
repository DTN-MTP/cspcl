#!/bin/bash
#
# test-picod3tn-basic.sh - Basic 2-node bundle transfer test for picoD3TN
# Tests: picoD3TN Node E <-> picoD3TN Node F, in both directions
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tests/interop/lib-picod3tn.sh disable=SC1091
source "${SCRIPT_DIR}/lib-picod3tn.sh"

echo "=================================================="
echo "  picoD3TN Basic Interoperability Test"
echo "=================================================="
echo "Test: Send bundles between Node E (CSP 5) and Node F (CSP 6)"
echo ""

# Configuration
NODE_E_CONTAINER="${NODE_E_CONTAINER:-cspcl-picod3tn-node-e}"
NODE_F_CONTAINER="${NODE_F_CONTAINER:-cspcl-picod3tn-node-f}"
NODE_E_ID="dtn://e.dtn/"
NODE_F_ID="dtn://f.dtn/"
TEST_MESSAGE_1="Hello from picoD3TN Node E via CSPCL!"
TEST_MESSAGE_2="Hello from picoD3TN Node F via CSPCL!"
RECV_LOG="/var/run/picod3tn/picod3tn-recv.log"
TIMEOUT=60

# Wait for nodes to be ready
echo "[1/5] Waiting for picoD3TN nodes to be ready..."
if picod3tn_wait_ready "$NODE_E_CONTAINER" "$TIMEOUT" && \
   picod3tn_wait_ready "$NODE_F_CONTAINER" "$TIMEOUT"; then
    echo "✓ Both cspcl links are open"
else
    echo "✗ FAILED: cspcl links not open after ${TIMEOUT}s"
    docker logs --tail 20 "$NODE_E_CONTAINER"
    docker logs --tail 20 "$NODE_F_CONTAINER"
    exit 1
fi

echo "[2/5] Configuring routes..."
picod3tn_route "$NODE_E_CONTAINER" "$NODE_E_ID" "$NODE_F_ID" "cspcl:6:10" > /dev/null
picod3tn_route "$NODE_F_CONTAINER" "$NODE_F_ID" "$NODE_E_ID" "cspcl:5:10" > /dev/null
echo "✓ Routes configured"

echo "[3/5] Starting bundle receivers..."
picod3tn_listen "$NODE_F_CONTAINER" "${NODE_F_ID}bundlesink" "$RECV_LOG"
picod3tn_listen "$NODE_E_CONTAINER" "${NODE_E_ID}bundlesink" "$RECV_LOG"
sleep 2
echo "✓ Receivers started"

echo "[4/5] Sending bundles..."
picod3tn_send "$NODE_E_CONTAINER" "${NODE_E_ID}source" "${NODE_F_ID}bundlesink" "$TEST_MESSAGE_1" > /dev/null
picod3tn_send "$NODE_F_CONTAINER" "${NODE_F_ID}source" "${NODE_E_ID}bundlesink" "$TEST_MESSAGE_2" > /dev/null
echo "✓ Bundles sent"

echo "[5/5] Verifying bundle reception..."
RECEIVED_1=0
RECEIVED_2=0
picod3tn_wait_received "$NODE_F_CONTAINER" "$RECV_LOG" "$TEST_MESSAGE_1" 30 && RECEIVED_1=1
picod3tn_wait_received "$NODE_E_CONTAINER" "$RECV_LOG" "$TEST_MESSAGE_2" 30 && RECEIVED_2=1

if (( RECEIVED_1 == 1 && RECEIVED_2 == 1 )); then
    echo "✓ Bundle received on Node F"
    echo "✓ Bundle received on Node E"
    echo ""
    echo "=================================================="
    echo "  TEST PASSED"
    echo "=================================================="
    exit 0
else
    (( RECEIVED_1 == 1 )) || echo "✗ Bundle not received on Node F (E -> F)"
    (( RECEIVED_2 == 1 )) || echo "✗ Bundle not received on Node E (F -> E)"
    echo ""
    echo "Node E logs (last 20 lines):"
    docker logs --tail 20 "$NODE_E_CONTAINER"
    echo ""
    echo "Node F logs (last 20 lines):"
    docker logs --tail 20 "$NODE_F_CONTAINER"
    echo ""
    echo "=================================================="
    echo "  TEST FAILED"
    echo "=================================================="
    exit 1
fi

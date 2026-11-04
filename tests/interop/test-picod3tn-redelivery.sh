#!/bin/bash
#
# test-picod3tn-redelivery.sh - A bundle to a stopped peer is kept, then delivered
# Tests: picoD3TN Node E -> Node F while F is down; F is restarted and receives it
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tests/interop/lib-picod3tn.sh disable=SC1091
source "${SCRIPT_DIR}/lib-picod3tn.sh"

echo "=================================================="
echo "  picoD3TN Redelivery Test"
echo "=================================================="
echo "Test: Node E (CSP 5) keeps a bundle for Node F (CSP 6) while F is down"
echo ""

# Configuration
NODE_E_CONTAINER="${NODE_E_CONTAINER:-cspcl-picod3tn-node-e}"
NODE_F_CONTAINER="${NODE_F_CONTAINER:-cspcl-picod3tn-node-f}"
NODE_E_ID="dtn://e.dtn/"
NODE_F_ID="dtn://f.dtn/"
TEST_MESSAGE="Kept by picoD3TN Node E while Node F was down"
RECV_LOG="/var/run/picod3tn/picod3tn-redelivery.log"
TIMEOUT=60
# A send to a stopped peer can take about 20 s to time out, and the link
# about 30 s more to recover once the peer is back.
DELIVERY_TIMEOUT=90

# shellcheck disable=SC2329
cleanup() {
    # Leave F running for the next test, even if this one failed midway.
    # shellcheck disable=SC2317
    docker start "$NODE_F_CONTAINER" > /dev/null 2>&1 || true
    # shellcheck disable=SC2317
    picod3tn_stop_listeners "$NODE_F_CONTAINER"
}
trap cleanup EXIT

fail() {
    echo "✗ FAILED: $1"
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
}

echo "[1/6] Waiting for picoD3TN nodes to be ready..."
if picod3tn_wait_ready "$NODE_E_CONTAINER" "$TIMEOUT" && \
   picod3tn_wait_ready "$NODE_F_CONTAINER" "$TIMEOUT"; then
    echo "✓ Both cspcl links are open"
else
    fail "cspcl links not open after ${TIMEOUT}s"
fi

echo "[2/6] Configuring route E -> F..."
picod3tn_route "$NODE_E_CONTAINER" "$NODE_E_ID" "$NODE_F_ID" "cspcl:6:10" > /dev/null
echo "✓ Route configured"

echo "[3/6] Stopping Node F..."
docker stop "$NODE_F_CONTAINER" > /dev/null
echo "✓ Node F stopped"

echo "[4/6] Sending a bundle to the stopped node..."
SENT_AT=$(date +%s)
picod3tn_send "$NODE_E_CONTAINER" "${NODE_E_ID}source" "${NODE_F_ID}redelivery" "$TEST_MESSAGE" > /dev/null
DEFERRED=0
for ((i=1; i<=TIMEOUT; i++)); do
    if docker logs --since "$SENT_AT" "$NODE_E_CONTAINER" 2>&1 | \
        grep -q "failed to transmit bundle via cspcl to 6:10"; then
        DEFERRED=1
        break
    fi
    sleep 1
done
(( DEFERRED == 1 )) || fail "Node E did not report a failed send to F within ${TIMEOUT}s"
echo "✓ Node E failed to send it, and deferred it"

echo "[5/6] Restarting Node F..."
STARTED_AT=$(date +%s)
docker start "$NODE_F_CONTAINER" > /dev/null
picod3tn_wait_ready "$NODE_F_CONTAINER" "$TIMEOUT" "$STARTED_AT" || \
    fail "Node F's cspcl link not open after ${TIMEOUT}s"
# A bundle that arrives first waits in F's spool for the listener.
picod3tn_listen "$NODE_F_CONTAINER" "${NODE_F_ID}redelivery" "$RECV_LOG" $((DELIVERY_TIMEOUT + 10))
echo "✓ Node F is back"

echo "[6/6] Waiting for the bundle on Node F..."
picod3tn_wait_received "$NODE_F_CONTAINER" "$RECV_LOG" "$TEST_MESSAGE" "$DELIVERY_TIMEOUT" || \
    fail "bundle not delivered within ${DELIVERY_TIMEOUT}s of F's restart"
echo "✓ Bundle received on Node F, $(( $(date +%s) - STARTED_AT ))s after its restart"
echo ""
echo "=================================================="
echo "  TEST PASSED"
echo "=================================================="

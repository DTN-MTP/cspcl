#!/bin/bash
#
# test-ud3tn-basic.sh - Basic 2-node bundle transfer test for uD3TN
# Tests: uD3TN Node A -> uD3TN Node B bundle transfer
#

set -e

echo "=================================================="
echo "  uD3TN Basic Interoperability Test"
echo "=================================================="
echo "Test: Send bundle from Node A (CSP 1) to Node B (CSP 2)"
echo ""

# Configuration
NODE_A_CONTAINER="${NODE_A_CONTAINER:-cspcl-ud3tn-node-a}"
NODE_B_CONTAINER="${NODE_B_CONTAINER:-cspcl-ud3tn-node-b}"
TEST_MESSAGE="Hello from uD3TN Node A via CSPCL!"
TIMEOUT=60

# Wait for nodes to be healthy
echo "[1/5] Waiting for uD3TN nodes to be ready..."
for ((i=1; i<=TIMEOUT; i++)); do
    if docker exec "$NODE_A_CONTAINER" pgrep -f ud3tn > /dev/null 2>&1 && \
       docker exec "$NODE_B_CONTAINER" pgrep -f ud3tn > /dev/null 2>&1; then
        echo "✓ Both nodes are running"
        break
    fi

    if (( i == TIMEOUT )); then
        echo "✗ FAILED: Nodes not ready after ${TIMEOUT}s"
        exit 1
    fi

    sleep 1
done

echo "[2/5] Configuring routes..."

# Configure route from Node A to Node B (CSP address 2)
docker exec "$NODE_A_CONTAINER" \
    /opt/ud3tn-src/build/posix/aap2/aap2_config \
    --socket /var/run/ud3tn/ud3tn.aap2.socket \
    --schedule 1 3600 100000 \
    --reaches dtn://b.dtn/bundlesink \
    dtn://b.dtn/ "csp:2,10" \
    > /dev/null 2>&1 || echo "Route may already exist"

echo "✓ Routes configured"

# Start receiver on Node B
echo "[3/5] Starting bundle receiver on Node B..."
docker exec "$NODE_B_CONTAINER" sh -c "rm -f /var/run/ud3tn/ud3tn-recv.log"
docker exec -d "$NODE_B_CONTAINER" bash -c "
    /opt/ud3tn-src/build/posix/aap2/aap2_receive \
        --socket /var/run/ud3tn/ud3tn.aap2.socket \
        --agentid bundlesink \
        --count 1 \
        --newline \
        > /var/run/ud3tn/ud3tn-recv.log 2>&1
"

sleep 2
echo "✓ Receiver started"

# Send bundle from Node A
echo "[4/5] Sending bundle from Node A..."
docker exec "$NODE_A_CONTAINER" \
    /opt/ud3tn-src/build/posix/aap2/aap2_send \
    --socket /var/run/ud3tn/ud3tn.aap2.socket \
    dtn://b.dtn/bundlesink \
    "$TEST_MESSAGE" \
    > /dev/null 2>&1

echo "✓ Bundle sent"

# Verify reception (check Node B logs)
echo "[5/5] Verifying bundle reception..."
sleep 3

# Check if bundle was received in receiver output
RECEIVED=0
for ((i=1; i<=10; i++)); do
    if docker exec "$NODE_B_CONTAINER" sh -c "grep -q '${TEST_MESSAGE}' /var/run/ud3tn/ud3tn-recv.log" 2>/dev/null; then
        RECEIVED=1
        break
    fi
    sleep 1
done

if (( RECEIVED == 1 )); then
    echo "✓ Bundle received on Node B"
    echo ""
    echo "=================================================="
    echo "  TEST PASSED"
    echo "=================================================="
    exit 0
else
    echo "✗ Bundle not received within timeout"
    echo ""
    echo "Node A logs (last 20 lines):"
    docker logs --tail 20 "$NODE_A_CONTAINER"
    echo ""
    echo "Node B logs (last 20 lines):"
    docker logs --tail 20 "$NODE_B_CONTAINER"
    echo ""
    echo "=================================================="
    echo "  TEST FAILED"
    echo "=================================================="
    exit 1
fi

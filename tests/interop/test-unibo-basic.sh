#!/bin/bash
#
# test-unibo-basic.sh - Basic 2-node bundle transfer test for Unibo-BP
# Tests: Unibo-BP Node 1 -> Unibo-BP Node 2 bundle transfer
#

set -e

echo "=================================================="
echo "  Unibo-BP Basic Interoperability Test"
echo "=================================================="
echo "Test: Send bundle from Node 1 (CSP 3) to Node 2 (CSP 4)"
echo ""

# Configuration
NODE_1_CONTAINER=${NODE_1_CONTAINER:-cspcl-unibo-node-1}
NODE_2_CONTAINER=${NODE_2_CONTAINER:-cspcl-unibo-node-2}
TEST_MESSAGE="Hello from Unibo-BP Node 1 via CSPCL!"
TIMEOUT=60

# Wait for nodes to be healthy
echo "[1/5] Waiting for Unibo-BP nodes to be ready..."
for ((i=1; i<=TIMEOUT; i++)); do
    if docker exec "$NODE_1_CONTAINER" pgrep -f "unibo-bp" > /dev/null 2>&1 && \
       docker exec "$NODE_2_CONTAINER" pgrep -f "unibo-bp" > /dev/null 2>&1; then
        echo "✓ Both nodes are running"
        break
    fi

    if (( i == TIMEOUT )); then
        echo "✗ FAILED: Nodes not ready after ${TIMEOUT}s"
        exit 1
    fi

    sleep 1
done

# Configure nodes (regions, ranges, contacts, routing)
echo "[2/5] Configuring Unibo-BP nodes..."

# Get reference time
REFERENCE_TIME=$(docker exec "$NODE_1_CONTAINER" \
    /opt/unibo-bp/bin/unibo-bp-utility --get-utc-time +0 2>/dev/null || echo "0")

# Configure Node 1
docker exec "$NODE_1_CONTAINER" bash -c "
    cd /tmp/unibo-node1
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:3.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:4.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:4.0 --owlt 0 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:4.0 --receiver ipn:3.0 --owlt 0 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:4.0 --xmit-rate 1000000 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin routing static add --destination ipn:4.55 --gateway ipn:4.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin routing static add --destination ipn:4.0 --gateway ipn:4.0 2>/dev/null || true
" > /dev/null 2>&1

# Configure Node 2
docker exec "$NODE_2_CONTAINER" bash -c "
    cd /tmp/unibo-node2
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:3.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:4.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:4.0 --owlt 0 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:4.0 --receiver ipn:3.0 --owlt 0 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:4.0 --xmit-rate 1000000 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin routing static add --destination ipn:3.55 --gateway ipn:3.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin routing static add --destination ipn:3.0 --gateway ipn:3.0 2>/dev/null || true
" > /dev/null 2>&1

echo "✓ Nodes configured"

# Start receiver on Node 2
echo "[3/5] Starting bundle sink on Node 2..."
docker exec -d "$NODE_2_CONTAINER" bash -c "
    cd /tmp/unibo-node2
    /opt/unibo-bp/bin/unibo-bp-sink ipn:4.55
"

sleep 2
echo "✓ Receiver started"

# Send bundle from Node 1
echo "[4/5] Sending bundle from Node 1..."
docker exec "$NODE_1_CONTAINER" bash -c "
    cd /tmp/unibo-node1
    /opt/unibo-bp/bin/unibo-bp-send \
        --source ipn:3.55 \
        --destination ipn:4.55 \
        --lifetime 600000 \
        --payload-string '$TEST_MESSAGE'
" > /dev/null 2>&1

echo "✓ Bundle sent"

# Verify reception
echo "[5/5] Verifying bundle reception..."
sleep 3

# Check if bundle was received (look in Node 2 logs for sink output)
RECEIVED=0
for ((i=1; i<=10; i++)); do
    if docker logs "$NODE_2_CONTAINER" 2>&1 | grep -q "Received.*bytes from ipn:3"; then
        RECEIVED=1
        break
    fi
    sleep 1
done

if (( RECEIVED == 1 )); then
    echo "✓ Bundle received on Node 2"
    echo ""
    echo "=================================================="
    echo "  TEST PASSED"
    echo "=================================================="
    exit 0
else
    echo "✗ Bundle not received within timeout"
    echo ""
    echo "Node 1 logs (last 30 lines):"
    docker logs --tail 30 "$NODE_1_CONTAINER"
    echo ""
    echo "Node 2 logs (last 30 lines):"
    docker logs --tail 30 "$NODE_2_CONTAINER"
    echo ""
    echo "=================================================="
    echo "  TEST FAILED"
    echo "=================================================="
    exit 1
fi

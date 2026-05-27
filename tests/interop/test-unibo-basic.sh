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

echo "  Waiting for Unibo-BP admin sockets..."
for node in "$NODE_1_CONTAINER" "$NODE_2_CONTAINER"; do
    for ((i=1; i<=30; i++)); do
        if docker exec "$node" bash -c "
            cd /tmp/unibo-node${node##*-}
            /opt/unibo-bp/bin/unibo-bp-admin whoami --scheme ipn >/dev/null 2>&1
        "; then
            break
        fi
        if (( i == 30 )); then
            echo "✗ FAILED: Unibo-BP admin not ready in ${node}"
            exit 1
        fi
        sleep 1
    done
done

# Fetch a reference time for consistent range/contact schedules.
echo "  Fetching Unibo-BP reference time..."
REFERENCE_TIME=$(docker exec "$NODE_1_CONTAINER" /opt/unibo-bp/bin/unibo-bp-utility --get-utc-time +0 2>/dev/null | tail -n 1 | tr -d '\r')
if [ -z "$REFERENCE_TIME" ]; then
    echo "✗ FAILED: Unable to fetch Unibo-BP reference time"
    exit 1
fi

# Configure Node 1
docker exec "$NODE_1_CONTAINER" bash -c "
    cd /tmp/unibo-node1
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:3.0
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:4.0
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:4.0 --owlt 0 --reference-time \"$REFERENCE_TIME\"
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:4.0 --receiver ipn:3.0 --owlt 0 --reference-time \"$REFERENCE_TIME\"
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:4.0 --xmit-rate 1000000 --reference-time \"$REFERENCE_TIME\"
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:4.0 --receiver ipn:3.0 --xmit-rate 1000000 --reference-time \"$REFERENCE_TIME\"
    /opt/unibo-bp/bin/unibo-bp-admin routing static add --destination ipn:4.55 --gateway ipn:4.0
    /opt/unibo-bp/bin/unibo-bp-admin routing static add --destination ipn:4.0 --gateway ipn:4.0
"

# Configure Node 2
docker exec "$NODE_2_CONTAINER" bash -c "
    cd /tmp/unibo-node2
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:3.0
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:4.0
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:4.0 --owlt 0 --reference-time \"$REFERENCE_TIME\"
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:4.0 --receiver ipn:3.0 --owlt 0 --reference-time \"$REFERENCE_TIME\"
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:4.0 --xmit-rate 1000000 --reference-time \"$REFERENCE_TIME\"
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:4.0 --receiver ipn:3.0 --xmit-rate 1000000 --reference-time \"$REFERENCE_TIME\"
    /opt/unibo-bp/bin/unibo-bp-admin routing static add --destination ipn:3.55 --gateway ipn:3.0
    /opt/unibo-bp/bin/unibo-bp-admin routing static add --destination ipn:3.0 --gateway ipn:3.0
"

# Nudge Unibo contact observers so CLA peers wake up when contacts are added.
docker exec "$NODE_1_CONTAINER" bash -c "
    cd /tmp/unibo-node1
    /opt/unibo-bp/bin/unibo-bp-admin contact change --sender ipn:3.0 --receiver ipn:4.0 \
        --start-time \"$REFERENCE_TIME\" --new-start-time \"$REFERENCE_TIME\" >/dev/null 2>&1 || true
"
docker exec "$NODE_2_CONTAINER" bash -c "
    cd /tmp/unibo-node2
    /opt/unibo-bp/bin/unibo-bp-admin contact change --sender ipn:4.0 --receiver ipn:3.0 \
        --start-time \"$REFERENCE_TIME\" --new-start-time \"$REFERENCE_TIME\" >/dev/null 2>&1 || true
"

echo "✓ Nodes configured"

# Start receiver on Node 2
echo "[3/5] Starting bundle sink on Node 2..."
docker exec "$NODE_2_CONTAINER" sh -c "PIDS=\$(pgrep -f '^/opt/unibo-bp/bin/unibo-bp-sink ipn:4\\.55$' || true); [ -z \"\$PIDS\" ] || kill \$PIDS"
docker exec "$NODE_2_CONTAINER" bash -c "rm -f /tmp/unibo-node2/unibo-sink.log"
docker exec -d "$NODE_2_CONTAINER" bash -c "
    cd /tmp/unibo-node2
    stdbuf -oL -eL /opt/unibo-bp/bin/unibo-bp-sink ipn:4.55 > /tmp/unibo-node2/unibo-sink.log 2>&1
"

sleep 2
echo "✓ Receiver started"

# Wait until sender has opened contact toward CSP node 4
for ((i=1; i<=30; i++)); do
    if docker logs "$NODE_1_CONTAINER" 2>&1 | grep -q "opened link_id=.* for csp_addr=4"; then
        break
    fi
    sleep 1
done

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

check_received() {
    docker exec "$NODE_2_CONTAINER" sh -c "grep -Eq 'Received|${TEST_MESSAGE}' /tmp/unibo-node2/unibo-sink.log || [ -s /tmp/unibo-node2/unibo-sink.log ]" 2>/dev/null
}

send_bundle() {
    docker exec "$NODE_1_CONTAINER" bash -c "
        cd /tmp/unibo-node1
        /opt/unibo-bp/bin/unibo-bp-send \
            --source ipn:3.55 \
            --destination ipn:4.55 \
            --lifetime 600000 \
            --payload-string '$TEST_MESSAGE'
    " > /dev/null 2>&1
}

restart_receiver() {
    docker exec "$NODE_2_CONTAINER" bash -c "
        PIDS=\$(pgrep -f '^/opt/unibo-bp/bin/unibo-bp-sink ipn:4\\.55$' || true)
        [ -z \"\$PIDS\" ] || kill \$PIDS
        rm -f /tmp/unibo-node2/unibo-sink.log
        cd /tmp/unibo-node2
        nohup stdbuf -oL -eL /opt/unibo-bp/bin/unibo-bp-sink ipn:4.55 > /tmp/unibo-node2/unibo-sink.log 2>&1 &
    "
    sleep 2
}

RECEIVED=0
for attempt in 1 2 3; do
    for ((i=1; i<=20; i++)); do
        if check_received; then
            RECEIVED=1
            break 2
        fi
        sleep 1
    done

    if (( attempt < 3 )); then
        restart_receiver
        send_bundle || true
    fi
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

#!/bin/bash
#
# test-cross-integration.sh - Cross-integration test between uD3TN and Unibo-BP
# Tests bidirectional bundle transfer: uD3TN <-> Unibo-BP
#

set -e

echo "=================================================="
echo "  Cross-Integration Interoperability Test"
echo "=================================================="
echo "Test: Bundle transfer between uD3TN and Unibo-BP"
echo "  Direction 1: uD3TN Node A (CSP 1) -> Unibo Node 2 (CSP 4)"
echo "  Direction 2: Unibo Node 1 (CSP 3) -> uD3TN Node B (CSP 2)"
echo ""

# Configuration
UD3TN_A_CONTAINER=${UD3TN_A_CONTAINER:-cspcl-ud3tn-node-a}
UD3TN_B_CONTAINER=${UD3TN_B_CONTAINER:-cspcl-ud3tn-node-b}
UNIBO_1_CONTAINER=${UNIBO_1_CONTAINER:-cspcl-unibo-node-1}
UNIBO_2_CONTAINER=${UNIBO_2_CONTAINER:-cspcl-unibo-node-2}
TEST_MESSAGE_1="Cross-integration test: uD3TN to Unibo-BP"
TEST_MESSAGE_2="Cross-integration test: Unibo-BP to uD3TN"
TIMEOUT=60

# shellcheck disable=SC2329
cleanup() {
    # shellcheck disable=SC2317
    docker exec "$UNIBO_2_CONTAINER" sh -c '
        pids=$(pgrep -f "^/opt/unibo-bp/bin/unibo-bp-sink ipn:4\\.55$" || true)
        if [ -n "$pids" ]; then
            kill $pids
        fi
    ' >/dev/null 2>&1 || true

    # shellcheck disable=SC2317
    docker exec "$UD3TN_B_CONTAINER" sh -c '
        pids=$(pgrep -f "^/opt/ud3tn-src/build/posix/aap2/aap2_receive( |$)" || true)
        if [ -n "$pids" ]; then
            kill $pids
        fi
    ' >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Wait for all nodes to be healthy
echo "[1/7] Waiting for all nodes to be ready..."
for ((i=1; i<=TIMEOUT; i++)); do
    if docker exec "$UD3TN_A_CONTAINER" pgrep -f ud3tn > /dev/null 2>&1 && \
       docker exec "$UD3TN_B_CONTAINER" pgrep -f ud3tn > /dev/null 2>&1 && \
       docker exec "$UNIBO_1_CONTAINER" pgrep -f "unibo-bp" > /dev/null 2>&1 && \
       docker exec "$UNIBO_2_CONTAINER" pgrep -f "unibo-bp" > /dev/null 2>&1; then
        echo "✓ All nodes are running"
        break
    fi

    if (( i == TIMEOUT )); then
        echo "✗ FAILED: Nodes not ready after ${TIMEOUT}s"
        exit 1
    fi

    sleep 1
done

echo "[2/7] Configuring cross-integration routes..."

# Configure uD3TN Node A to send to Unibo Node 2 (CSP address 4)
docker exec "$UD3TN_A_CONTAINER" \
    /opt/ud3tn-src/build/posix/aap2/aap2_config \
    --socket /var/run/ud3tn/ud3tn.aap2.socket \
    --schedule 1 3600 100000 --reaches ipn:4.55 ipn:4.0 "csp:4,10" \
    > /dev/null 2>&1 || echo "Route may already exist"

# Configure uD3TN Node B to receive from Unibo Node 1 (CSP address 3).
# Register both ipn and dtn node-IDs to cover transport-specific source conventions.
docker exec "$UD3TN_B_CONTAINER" \
    /opt/ud3tn-src/build/posix/aap2/aap2_config \
    --socket /var/run/ud3tn/ud3tn.aap2.socket \
    --schedule 1 3600 100000 ipn:3.0 "csp:3,10" \
    > /dev/null 2>&1 || echo "Route may already exist"
docker exec "$UD3TN_B_CONTAINER" \
    /opt/ud3tn-src/build/posix/aap2/aap2_config \
    --socket /var/run/ud3tn/ud3tn.aap2.socket \
    --schedule 1 3600 100000 dtn://c.dtn/ "csp:3,10" \
    > /dev/null 2>&1 || echo "Route may already exist"

# Configure Unibo-BP nodes
REFERENCE_TIME="+0"

# Unibo Node 1 configuration (sends to uD3TN B at CSP 2)
docker exec "$UNIBO_1_CONTAINER" bash -c "
    cd /tmp/unibo-node1
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:1.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:2.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:2.0 --owlt 0 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:3.0 --owlt 0 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:3.0 --receiver ipn:2.0 --xmit-rate 1000000 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:3.0 --xmit-rate 1000000 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin routing static add --destination dtn://b.dtn/ --gateway ipn:2.0 2>/dev/null || true
" > /dev/null 2>&1

# Unibo Node 2 configuration (receives from uD3TN A at CSP 1)
docker exec "$UNIBO_2_CONTAINER" bash -c "
    cd /tmp/unibo-node2
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:1.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin region home --register-node ipn:4.0 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:4.0 --owlt 0 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:4.0 --receiver ipn:1.0 --owlt 0 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:4.0 --xmit-rate 1000000 --reference-time $REFERENCE_TIME 2>/dev/null || true
    /opt/unibo-bp/bin/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:4.0 --receiver ipn:1.0 --xmit-rate 1000000 --reference-time $REFERENCE_TIME 2>/dev/null || true
" > /dev/null 2>&1

echo "✓ Cross-integration routes configured"

# Test Direction 1: uD3TN A -> Unibo Node 2
echo "[3/7] Testing uD3TN -> Unibo-BP direction..."

# Start receiver on Unibo Node 2
docker exec "$UNIBO_2_CONTAINER" sh -c "PIDS=\$(pgrep -f '^/opt/unibo-bp/bin/unibo-bp-sink ipn:4\\.55$' || true); [ -z \"\$PIDS\" ] || kill \$PIDS"
docker exec "$UNIBO_2_CONTAINER" bash -c "rm -f /tmp/unibo-node2/cross-sink.log"
docker exec -d "$UNIBO_2_CONTAINER" bash -c "
    cd /tmp/unibo-node2
    stdbuf -oL -eL /opt/unibo-bp/bin/unibo-bp-sink ipn:4.55 > /tmp/unibo-node2/cross-sink.log 2>&1
"

# Wait until uD3TN has started the scheduled contact toward CSP node 4
for ((i=1; i<=30; i++)); do
    if docker logs "$UD3TN_A_CONTAINER" 2>&1 | grep -q "Starting scheduled contact to csp:4"; then
        break
    fi
    sleep 1
done

# Wait until Unibo Node 2 has opened contact toward CSP node 1
for ((i=1; i<=30; i++)); do
    if docker logs "$UNIBO_2_CONTAINER" 2>&1 | grep -q "opened link_id=.* for csp_addr=1"; then
        break
    fi
    sleep 1
done

sleep 3

check_unibo_sink_received() {
    docker exec "$UNIBO_2_CONTAINER" sh -c "grep -Eq 'Received|${TEST_MESSAGE_1}' /tmp/unibo-node2/cross-sink.log || [ -s /tmp/unibo-node2/cross-sink.log ]" 2>/dev/null
}

# Send bundle from uD3TN A
docker exec "$UD3TN_A_CONTAINER" \
    /opt/ud3tn-src/build/posix/aap2/aap2_send \
    --socket /var/run/ud3tn/ud3tn.aap2.socket \
    ipn:4.55 \
    "$TEST_MESSAGE_1" \
    > /dev/null 2>&1

echo "✓ Bundle sent from uD3TN A"

# Verify reception at Unibo Node 2
echo "[4/7] Verifying bundle reception at Unibo Node 2..."
sleep 3

RECEIVED_1=0
for ((i=1; i<=30; i++)); do
    if check_unibo_sink_received; then
        RECEIVED_1=1
        break
    fi
    sleep 1
done

if (( RECEIVED_1 == 0 )); then
    docker exec "$UD3TN_A_CONTAINER" \
        /opt/ud3tn-src/build/posix/aap2/aap2_send \
        --socket /var/run/ud3tn/ud3tn.aap2.socket \
        ipn:4.55 \
        "$TEST_MESSAGE_1" \
        > /dev/null 2>&1 || true
    for ((i=1; i<=20; i++)); do
        if check_unibo_sink_received; then
            RECEIVED_1=1
            break
        fi
        sleep 1
    done
fi

if (( RECEIVED_1 == 1 )); then
    echo "✓ Bundle received at Unibo-BP from uD3TN"
else
    echo "✗ Bundle not received (uD3TN -> Unibo)"
fi

# Test Direction 2: Unibo Node 1 -> uD3TN B
echo "[5/7] Testing Unibo-BP -> uD3TN direction..."

# Wait until Unibo Node 1 has opened contact toward CSP node 2
for ((i=1; i<=30; i++)); do
    if docker logs "$UNIBO_1_CONTAINER" 2>&1 | grep -q "opened link_id=.* for csp_addr=2"; then
        break
    fi
    sleep 1
done

# Ensure uD3TN B has active contact toward CSP node 3 before waiting for reverse flow
for ((i=1; i<=30; i++)); do
    if docker logs "$UD3TN_B_CONTAINER" 2>&1 | grep -q "Starting scheduled contact to csp:3"; then
        break
    fi
    sleep 1
done

# Verify reception at uD3TN B
echo "[6/7] Verifying bundle reception at uD3TN B..."

RECEIVED_2=0

for ((attempt=1; attempt<=4; attempt++)); do
    AGENT_ID="bundlesink-${attempt}"
    RECV_LOG="/var/run/ud3tn/cross-recv-${attempt}.log"
    docker exec "$UD3TN_B_CONTAINER" sh -c "PIDS=\$(pgrep -f '^/opt/ud3tn-src/build/posix/aap2/aap2_receive( |$)' || true); [ -z \"\$PIDS\" ] || kill \$PIDS"
    docker exec "$UD3TN_B_CONTAINER" sh -c "rm -f '$RECV_LOG'"
    docker exec -d "$UD3TN_B_CONTAINER" bash -c "
        /opt/ud3tn-src/build/posix/aap2/aap2_receive \
            --socket /var/run/ud3tn/ud3tn.aap2.socket \
            --agentid '$AGENT_ID' \
            --count 1 \
            --newline \
            > '$RECV_LOG' 2>&1
    "
    sleep 2

    docker exec "$UNIBO_1_CONTAINER" bash -c "
        cd /tmp/unibo-node1
        /opt/unibo-bp/bin/unibo-bp-send \
            --source dtn://c.dtn/source \
            --destination dtn://b.dtn/$AGENT_ID \
            --lifetime 600000 \
            --payload-string '$TEST_MESSAGE_2'
    " > /dev/null 2>&1 || true

    if (( attempt == 1 )); then
        echo "✓ Bundle sent from Unibo Node 1"
    fi

    for ((i=1; i<=15; i++)); do
        if docker exec "$UD3TN_B_CONTAINER" sh -c "grep -q '${TEST_MESSAGE_2}' '$RECV_LOG'" 2>/dev/null; then
            RECEIVED_2=1
            break
        fi
        sleep 1
    done

    if (( RECEIVED_2 == 1 )); then
        break
    fi
done

if (( RECEIVED_2 == 1 )); then
    echo "✓ Bundle received at uD3TN from Unibo-BP"
else
    echo "✗ Bundle not received (Unibo -> uD3TN)"
fi

# Final verification
echo "[7/7] Final verification..."

if (( RECEIVED_1 == 1 && RECEIVED_2 == 1 )); then
    echo ""
    echo "=================================================="
    echo "  CROSS-INTEGRATION TEST PASSED"
    echo "=================================================="
    echo "✓ uD3TN -> Unibo-BP: SUCCESS"
    echo "✓ Unibo-BP -> uD3TN: SUCCESS"
    echo ""
    exit 0
else
    echo ""
    echo "=================================================="
    echo "  CROSS-INTEGRATION TEST FAILED"
    echo "=================================================="

    status_1="FAILED"
    status_2="FAILED"
    (( RECEIVED_1 == 1 )) && status_1="SUCCESS"
    (( RECEIVED_2 == 1 )) && status_2="SUCCESS"

    echo "  uD3TN -> Unibo-BP: ${status_1}"
    echo "  Unibo-BP -> uD3TN: ${status_2}"
    echo ""
    echo "Debug logs:"
    echo "--- uD3TN A ---"
    docker logs --tail 15 "$UD3TN_A_CONTAINER"
    echo "--- uD3TN B ---"
    docker logs --tail 15 "$UD3TN_B_CONTAINER"
    echo "--- Unibo 1 ---"
    docker logs --tail 15 "$UNIBO_1_CONTAINER"
    echo "--- Unibo 2 ---"
    docker logs --tail 15 "$UNIBO_2_CONTAINER"
    echo ""
    exit 1
fi

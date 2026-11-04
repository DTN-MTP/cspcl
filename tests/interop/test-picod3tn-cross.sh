#!/bin/bash
#
# test-picod3tn-cross.sh - Cross-integration test of picoD3TN with uD3TN and Unibo-BP
# Tests bidirectional bundle transfer: picoD3TN <-> uD3TN and picoD3TN <-> Unibo-BP
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tests/interop/lib-picod3tn.sh disable=SC1091
source "${SCRIPT_DIR}/lib-picod3tn.sh"

echo "=================================================="
echo "  picoD3TN Cross-Integration Interoperability Test"
echo "=================================================="
echo "Test: Bundle transfer between picoD3TN, uD3TN and Unibo-BP"
echo "  Direction 1: picoD3TN Node E (CSP 5) -> uD3TN Node A (CSP 1)"
echo "  Direction 2: uD3TN Node A (CSP 1) -> picoD3TN Node E (CSP 5)"
echo "  Direction 3: picoD3TN Node F (CSP 6) -> Unibo Node 2 (CSP 4)"
echo "  Direction 4: Unibo Node 1 (CSP 3) -> picoD3TN Node F (CSP 6)"
echo ""

# Configuration
PICO_E_CONTAINER=${PICO_E_CONTAINER:-cspcl-picod3tn-node-e}
PICO_F_CONTAINER=${PICO_F_CONTAINER:-cspcl-picod3tn-node-f}
UD3TN_A_CONTAINER=${UD3TN_A_CONTAINER:-cspcl-ud3tn-node-a}
UNIBO_1_CONTAINER=${UNIBO_1_CONTAINER:-cspcl-unibo-node-1}
UNIBO_2_CONTAINER=${UNIBO_2_CONTAINER:-cspcl-unibo-node-2}
PICO_E_ID="dtn://e.dtn/"
PICO_F_ID="dtn://f.dtn/"
UD3TN_SOCKET="/var/run/ud3tn/ud3tn.aap2.socket"
UD3TN_AAP2="/opt/ud3tn-src/build/posix/aap2"
UNIBO_BIN="/opt/unibo-bp/bin"
TEST_MESSAGE_1="picoD3TN cross test: picoD3TN to uD3TN"
TEST_MESSAGE_2="picoD3TN cross test: uD3TN to picoD3TN"
TEST_MESSAGE_3="picoD3TN cross test: picoD3TN to Unibo-BP"
TEST_MESSAGE_4="picoD3TN cross test: Unibo-BP to picoD3TN"
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
    docker exec "$UD3TN_A_CONTAINER" sh -c '
        pids=$(pgrep -f "^/opt/ud3tn-src/build/posix/aap2/aap2_receive( |$)" || true)
        if [ -n "$pids" ]; then
            kill $pids
        fi
    ' >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Wait for all nodes to be ready
echo "[1/7] Waiting for all nodes to be ready..."
for ((i=1; i<=TIMEOUT; i++)); do
    if docker exec "$UD3TN_A_CONTAINER" pgrep -f ud3tn > /dev/null 2>&1 && \
       docker exec "$UNIBO_1_CONTAINER" pgrep -f "unibo-bp" > /dev/null 2>&1 && \
       docker exec "$UNIBO_2_CONTAINER" pgrep -f "unibo-bp" > /dev/null 2>&1; then
        break
    fi

    if (( i == TIMEOUT )); then
        echo "✗ FAILED: Nodes not ready after ${TIMEOUT}s"
        exit 1
    fi

    sleep 1
done
if ! picod3tn_wait_ready "$PICO_E_CONTAINER" "$TIMEOUT" || \
   ! picod3tn_wait_ready "$PICO_F_CONTAINER" "$TIMEOUT"; then
    echo "✗ FAILED: picoD3TN cspcl links not open after ${TIMEOUT}s"
    exit 1
fi
echo "✓ All nodes are running"

echo "[2/7] Configuring cross-integration routes..."

# picoD3TN E <-> uD3TN A
picod3tn_route "$PICO_E_CONTAINER" "$PICO_E_ID" "dtn://a.dtn/" "cspcl:1:10" > /dev/null
docker exec "$UD3TN_A_CONTAINER" \
    "${UD3TN_AAP2}/aap2_config" \
    --socket "$UD3TN_SOCKET" \
    --schedule 1 3600 100000 "$PICO_E_ID" "csp:5,10" \
    > /dev/null 2>&1 || echo "Route may already exist"

# picoD3TN F <-> Unibo-BP. Unibo-BP maps ipn node N to CSP address N, so
# picoD3TN F (CSP 6) is ipn:6.0 on that side.
picod3tn_route "$PICO_F_CONTAINER" "$PICO_F_ID" "ipn:4.0" "cspcl:4:10" > /dev/null

# Fetch a reference time for consistent Unibo range/contact schedules.
REFERENCE_TIME=$(docker exec "$UNIBO_1_CONTAINER" "${UNIBO_BIN}/unibo-bp-utility" --get-utc-time +0 2>/dev/null | tail -n 1 | tr -d '\r')
if [ -z "$REFERENCE_TIME" ]; then
    echo "✗ FAILED: Unable to fetch Unibo-BP reference time"
    exit 1
fi

# unibo_contact CONTAINER NODE_DIR LOCAL PEER
# Ranges and contacts in both directions between LOCAL and PEER, then a
# contact change so the CLA opens its link to the peer.
unibo_contact() {
    local container=$1 dir=$2 local_node=$3 peer=$4
    docker exec "$container" bash -c "
        cd $dir
        ${UNIBO_BIN}/unibo-bp-admin region home --register-node $peer 2>/dev/null || true
        ${UNIBO_BIN}/unibo-bp-admin region home --register-node $local_node 2>/dev/null || true
        ${UNIBO_BIN}/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender $local_node --receiver $peer --owlt 0 --reference-time \"$REFERENCE_TIME\" 2>/dev/null || true
        ${UNIBO_BIN}/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender $peer --receiver $local_node --owlt 0 --reference-time \"$REFERENCE_TIME\" 2>/dev/null || true
        ${UNIBO_BIN}/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender $local_node --receiver $peer --xmit-rate 1000000 --reference-time \"$REFERENCE_TIME\" 2>/dev/null || true
        ${UNIBO_BIN}/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender $peer --receiver $local_node --xmit-rate 1000000 --reference-time \"$REFERENCE_TIME\" 2>/dev/null || true
        ${UNIBO_BIN}/unibo-bp-admin contact change --sender $local_node --receiver $peer \
            --start-time \"$REFERENCE_TIME\" --new-start-time \"$REFERENCE_TIME\" 2>/dev/null || true
    " > /dev/null 2>&1
}

unibo_contact "$UNIBO_2_CONTAINER" /tmp/unibo-node2 ipn:4.0 ipn:6.0
unibo_contact "$UNIBO_1_CONTAINER" /tmp/unibo-node1 ipn:3.0 ipn:6.0
docker exec "$UNIBO_1_CONTAINER" bash -c "
    cd /tmp/unibo-node1
    ${UNIBO_BIN}/unibo-bp-admin routing static add --destination $PICO_F_ID --gateway ipn:6.0 2>/dev/null || true
" > /dev/null 2>&1

echo "✓ Cross-integration routes configured"

# Direction 1: picoD3TN E -> uD3TN A
echo "[3/7] Testing picoD3TN -> uD3TN direction..."
RECV_LOG_1="/var/run/ud3tn/pico-recv.log"
docker exec "$UD3TN_A_CONTAINER" sh -c "rm -f '$RECV_LOG_1'"
docker exec -d "$UD3TN_A_CONTAINER" bash -c "
    ${UD3TN_AAP2}/aap2_receive \
        --socket $UD3TN_SOCKET \
        --agentid picosink \
        --count 1 \
        --newline \
        > '$RECV_LOG_1' 2>&1
"
sleep 2

RECEIVED_1=0
for ((attempt=1; attempt<=3 && RECEIVED_1 == 0; attempt++)); do
    picod3tn_send "$PICO_E_CONTAINER" "${PICO_E_ID}source" "dtn://a.dtn/picosink" "$TEST_MESSAGE_1" > /dev/null || true
    for ((i=1; i<=15; i++)); do
        if docker exec "$UD3TN_A_CONTAINER" sh -c "grep -q '${TEST_MESSAGE_1}' '$RECV_LOG_1'" 2>/dev/null; then
            RECEIVED_1=1
            break
        fi
        sleep 1
    done
done

if (( RECEIVED_1 == 1 )); then
    echo "✓ Bundle received at uD3TN from picoD3TN"
else
    echo "✗ Bundle not received (picoD3TN -> uD3TN)"
fi

# Direction 2: uD3TN A -> picoD3TN E
echo "[4/7] Testing uD3TN -> picoD3TN direction..."
for ((i=1; i<=30; i++)); do
    if docker logs "$UD3TN_A_CONTAINER" 2>&1 | grep -q "Starting scheduled contact to csp:5"; then
        break
    fi
    sleep 1
done

RECV_LOG_2="/var/run/picod3tn/ud3tn-recv.log"
picod3tn_listen "$PICO_E_CONTAINER" "${PICO_E_ID}bundlesink" "$RECV_LOG_2"
sleep 2

RECEIVED_2=0
for ((attempt=1; attempt<=3 && RECEIVED_2 == 0; attempt++)); do
    docker exec "$UD3TN_A_CONTAINER" \
        "${UD3TN_AAP2}/aap2_send" \
        --socket "$UD3TN_SOCKET" \
        "${PICO_E_ID}bundlesink" \
        "$TEST_MESSAGE_2" \
        > /dev/null 2>&1 || true
    picod3tn_wait_received "$PICO_E_CONTAINER" "$RECV_LOG_2" "$TEST_MESSAGE_2" 15 && RECEIVED_2=1
done

if (( RECEIVED_2 == 1 )); then
    echo "✓ Bundle received at picoD3TN from uD3TN"
else
    echo "✗ Bundle not received (uD3TN -> picoD3TN)"
fi

# Direction 3: picoD3TN F -> Unibo Node 2
echo "[5/7] Testing picoD3TN -> Unibo-BP direction..."
for ((i=1; i<=30; i++)); do
    if docker logs "$UNIBO_2_CONTAINER" 2>&1 | grep -q "opened link_id=.* for csp_addr=6"; then
        break
    fi
    sleep 1
done

SINK_LOG="/tmp/unibo-node2/pico-sink.log"
docker exec "$UNIBO_2_CONTAINER" sh -c "PIDS=\$(pgrep -f '^/opt/unibo-bp/bin/unibo-bp-sink ipn:4\\.55$' || true); [ -z \"\$PIDS\" ] || kill \$PIDS"
docker exec "$UNIBO_2_CONTAINER" bash -c "rm -f $SINK_LOG"
docker exec -d "$UNIBO_2_CONTAINER" bash -c "
    cd /tmp/unibo-node2
    stdbuf -oL -eL ${UNIBO_BIN}/unibo-bp-sink ipn:4.55 > $SINK_LOG 2>&1
"
sleep 3

RECEIVED_3=0
for ((attempt=1; attempt<=3 && RECEIVED_3 == 0; attempt++)); do
    picod3tn_send "$PICO_F_CONTAINER" "${PICO_F_ID}source" "ipn:4.55" "$TEST_MESSAGE_3" > /dev/null || true
    for ((i=1; i<=20; i++)); do
        if docker exec "$UNIBO_2_CONTAINER" sh -c "grep -q '${TEST_MESSAGE_3}' $SINK_LOG" 2>/dev/null; then
            RECEIVED_3=1
            break
        fi
        sleep 1
    done
done

if (( RECEIVED_3 == 1 )); then
    echo "✓ Bundle received at Unibo-BP from picoD3TN"
else
    echo "✗ Bundle not received (picoD3TN -> Unibo)"
fi

# Direction 4: Unibo Node 1 -> picoD3TN F
echo "[6/7] Testing Unibo-BP -> picoD3TN direction..."
for ((i=1; i<=30; i++)); do
    if docker logs "$UNIBO_1_CONTAINER" 2>&1 | grep -q "opened link_id=.* for csp_addr=6"; then
        break
    fi
    sleep 1
done

RECV_LOG_4="/var/run/picod3tn/unibo-recv.log"
picod3tn_listen "$PICO_F_CONTAINER" "${PICO_F_ID}bundlesink" "$RECV_LOG_4"
sleep 2

RECEIVED_4=0
for ((attempt=1; attempt<=4 && RECEIVED_4 == 0; attempt++)); do
    docker exec "$UNIBO_1_CONTAINER" bash -c "
        cd /tmp/unibo-node1
        ${UNIBO_BIN}/unibo-bp-send \
            --source dtn://c.dtn/source \
            --destination ${PICO_F_ID}bundlesink \
            --lifetime 600000 \
            --payload-string '$TEST_MESSAGE_4'
    " > /dev/null 2>&1 || true
    picod3tn_wait_received "$PICO_F_CONTAINER" "$RECV_LOG_4" "$TEST_MESSAGE_4" 15 && RECEIVED_4=1
done

if (( RECEIVED_4 == 1 )); then
    echo "✓ Bundle received at picoD3TN from Unibo-BP"
else
    echo "✗ Bundle not received (Unibo -> picoD3TN)"
fi

# Final verification
echo "[7/7] Final verification..."

status() { if (( $1 == 1 )); then echo "SUCCESS"; else echo "FAILED"; fi; }

if (( RECEIVED_1 == 1 && RECEIVED_2 == 1 && RECEIVED_3 == 1 && RECEIVED_4 == 1 )); then
    echo ""
    echo "=================================================="
    echo "  PICOD3TN CROSS-INTEGRATION TEST PASSED"
    echo "=================================================="
    echo "✓ picoD3TN -> uD3TN:    SUCCESS"
    echo "✓ uD3TN -> picoD3TN:    SUCCESS"
    echo "✓ picoD3TN -> Unibo-BP: SUCCESS"
    echo "✓ Unibo-BP -> picoD3TN: SUCCESS"
    echo ""
    exit 0
else
    echo ""
    echo "=================================================="
    echo "  PICOD3TN CROSS-INTEGRATION TEST FAILED"
    echo "=================================================="
    echo "  picoD3TN -> uD3TN:    $(status "$RECEIVED_1")"
    echo "  uD3TN -> picoD3TN:    $(status "$RECEIVED_2")"
    echo "  picoD3TN -> Unibo-BP: $(status "$RECEIVED_3")"
    echo "  Unibo-BP -> picoD3TN: $(status "$RECEIVED_4")"
    echo ""
    echo "Debug logs:"
    echo "--- picoD3TN E ---"
    docker logs --tail 15 "$PICO_E_CONTAINER"
    echo "--- picoD3TN F ---"
    docker logs --tail 15 "$PICO_F_CONTAINER"
    echo "--- uD3TN A ---"
    docker logs --tail 15 "$UD3TN_A_CONTAINER"
    echo "--- Unibo 1 ---"
    docker logs --tail 15 "$UNIBO_1_CONTAINER"
    echo "--- Unibo 2 ---"
    docker logs --tail 15 "$UNIBO_2_CONTAINER"
    echo ""
    exit 1
fi

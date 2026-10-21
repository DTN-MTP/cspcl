#!/bin/bash
#
# lib-picod3tn.sh - Helpers for the picoD3TN interop tests (sourced)
# aapctl runs in the node's container, which sets AAPCTL_CONNECT.
#

# picod3tn_wait_ready CONTAINER TIMEOUT
# Waits until the node's cspcl link is open.
picod3tn_wait_ready() {
    local container=$1 timeout=$2 i
    for ((i=1; i<=timeout; i++)); do
        if docker logs "$container" 2>&1 | grep -q "cspcl link open as CSP"; then
            return 0
        fi
        sleep 1
    done
    return 1
}

# picod3tn_route CONTAINER NODE_ID PEER_NODE_ID CLA_ADDR
# Adds a FIB entry: PEER_NODE_ID is reached via CLA_ADDR (e.g. cspcl:6:10).
picod3tn_route() {
    local container=$1 node_id=$2 peer=$3 cla_addr=$4
    docker exec "$container" aapctl configure "${node_id}admin" "$peer" "$cla_addr" LINK_STATUS_UP
}

# picod3tn_listen CONTAINER EID LOG [SECONDS]
# Registers EID and writes the bundles it receives to LOG, in the background.
picod3tn_listen() {
    local container=$1 eid=$2 log=$3 seconds=${4:-90}
    docker exec "$container" rm -f "$log"
    docker exec -d "$container" sh -c "timeout ${seconds} aapctl listen '${eid}' > '${log}' 2>&1"
}

# picod3tn_stop_listeners CONTAINER
# Stops the aapctl listeners started by picod3tn_listen. A listener left
# running keeps its agent ID registered, and receives the next test's bundles.
picod3tn_stop_listeners() {
    local container=$1
    docker exec "$container" sh -c 'pkill -f "^aapctl listen " || true' > /dev/null 2>&1 || true
}

# picod3tn_send CONTAINER SRC_EID DST_EID MESSAGE
picod3tn_send() {
    local container=$1 src=$2 dst=$3 message=$4
    docker exec "$container" aapctl send "$src" "$dst" "$message"
}

# picod3tn_received CONTAINER LOG MESSAGE
# aapctl prints each bundle as a pretty Debug struct, with the payload as a
# list of bytes. Succeeds if LOG holds a bundle whose payload is MESSAGE.
picod3tn_received() {
    local container=$1 log=$2 message=$3 expected byte
    expected="payload:["
    for byte in $(printf '%s' "$message" | od -An -tu1); do
        expected+="${byte},"
    done
    expected+="]"
    docker exec "$container" cat "$log" 2>/dev/null | tr -d ' \n' | grep -qF "$expected"
}

# picod3tn_wait_received CONTAINER LOG MESSAGE TIMEOUT
picod3tn_wait_received() {
    local container=$1 log=$2 message=$3 timeout=$4 i
    for ((i=1; i<=timeout; i++)); do
        if picod3tn_received "$container" "$log" "$message"; then
            return 0
        fi
        sleep 1
    done
    return 1
}

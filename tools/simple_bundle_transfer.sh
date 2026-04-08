#!/bin/bash
#
# debug_bundle_transfer.sh - Debug script for CSP bundle transfer issues
#
# This script helps diagnose why bundles aren't being received between
# two uD3TN nodes using the CSP CLA.
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UD3TN_DIR="${UD3TN_PATH:-/home/mathias/ud3tn-src}"
UD3TN_BIN="${UD3TN_DIR}/build/posix/ud3tn"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_help() {
    echo "Usage: $(basename "$0") [--transport zmqhub|can] [--help]"
    echo ""
    echo "Options:"
    echo "  --transport   Select transport (default: zmqhub)"
    echo "  -h, --help    Show this help message"
}

TRANSPORT="zmqhub"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --transport)
            TRANSPORT="${2:-zmqhub}"
            shift 2
            ;;
        -h|--help)
            print_help
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_help
            exit 1
            ;;
    esac
done

echo "=============================================="
echo "  CSP Bundle Transfer Debugging Guide"
echo "=============================================="
echo "Transport: ${TRANSPORT}"
echo ""

# Check 1: ZMQ Hub broker
if [ "$TRANSPORT" = "zmqhub" ]; then
    echo -e "${YELLOW}[Check 1] ZMQ Hub Broker${NC}"
    if pgrep -f "zmqhub_broker.py" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ ZMQ Hub broker is running${NC}"
    else
        echo -e "${RED}✗ ZMQ Hub broker is NOT running${NC}"
        echo ""
        echo "  The CSP ZMQHUB interface requires a broker to route packets."
        echo "  Start the broker in a separate terminal:"
        echo ""
        echo "    python3 $SCRIPT_DIR/tools/zmqhub_broker.py -v"
        echo ""
    fi
fi

# Check 2: uD3TN processes
echo ""
echo -e "${YELLOW}[Check 2] uD3TN Processes${NC}"
UD3TN_PROCS=$(pgrep -f "ud3tn" 2>/dev/null | wc -l)
if [ "$UD3TN_PROCS" -ge 2 ]; then
    echo -e "${GREEN}✓ Found $UD3TN_PROCS uD3TN processes running${NC}"
    pgrep -a "ud3tn" | head -5
else
    echo -e "${RED}✗ Need at least 2 uD3TN instances for transfer${NC}"
    echo ""
    echo "  Start two instances in separate terminals:"
    echo ""
    echo "  Terminal 1 (Node A - CSP addr 1):"
    if [ "$TRANSPORT" = "can" ]; then
        echo "    $UD3TN_BIN --node-id dtn://a.dtn/ --aap-port 4242 \\"
        echo "               --aap2-socket 1.aap2.socket --cla \"csp:1,10,can\""
    else
        echo "    $UD3TN_BIN --node-id dtn://a.dtn/ --aap-port 4242 \\"
        echo "               --aap2-socket 1.aap2.socket --cla \"csp:1,10\""
    fi
    echo ""
    echo "  Terminal 2 (Node B - CSP addr 2):"
    if [ "$TRANSPORT" = "can" ]; then
        echo "    $UD3TN_BIN --node-id dtn://b.dtn/ --aap-port 4243 \\"
        echo "               --aap2-socket 2.aap2.socket --cla \"csp:2,10,can\""
    else
        echo "    $UD3TN_BIN --node-id dtn://b.dtn/ --aap-port 4243 \\"
        echo "               --aap2-socket 2.aap2.socket --cla \"csp:2,10\""
    fi
    echo ""
fi

# Check 3: ZMQ ports
if [ "$TRANSPORT" = "zmqhub" ]; then
    echo ""
    echo -e "${YELLOW}[Check 3] ZMQ Ports (6000, 7000)${NC}"
    if netstat -tlnp 2>/dev/null | grep -E ":(6000|7000)" > /dev/null; then
        echo -e "${GREEN}✓ ZMQ ports appear to be in use${NC}"
        netstat -tlnp 2>/dev/null | grep -E ":(6000|7000)" || true
    else
        echo -e "${YELLOW}? ZMQ ports not detected (may be normal if using different transport)${NC}"
    fi
fi

# Check 3b: CAN vcan interface
if [ "$TRANSPORT" = "can" ]; then
    echo ""
    echo -e "${YELLOW}[Check 3] vCAN Interface${NC}"
    if ip link show vcan0 > /dev/null 2>&1; then
        echo -e "${GREEN}✓ vcan0 exists${NC}"
    else
        echo -e "${YELLOW}? vcan0 not found${NC}"
        echo "  Create it with:"
        echo "    sudo modprobe vcan"
        echo "    sudo ip link add dev vcan0 type vcan"
        echo "    sudo ip link set up vcan0"
    fi
fi

# Check 4: AAP2 sockets
echo ""
echo -e "${YELLOW}[Check 4] AAP2 Socket Files${NC}"
for sock in 1.aap2.socket 2.aap2.socket; do
    if [ -S "$sock" ]; then
        echo -e "${GREEN}✓ $sock exists${NC}"
    else
        echo -e "${YELLOW}? $sock not found in current directory${NC}"
    fi
done

echo ""
echo "=============================================="
echo "  Quick Start Commands"
echo "=============================================="
echo ""
if [ "$TRANSPORT" = "zmqhub" ]; then
    echo "1. Start ZMQ broker (Terminal 1):"
    echo "   python3 $SCRIPT_DIR/tools/zmqhub_broker.py -v"
    echo ""
    echo "2. Start Node A (Terminal 2):"
    echo "   $UD3TN_BIN --node-id dtn://a.dtn/ --aap-port 4242 \\"
    echo "              --aap2-socket 1.aap2.socket --cla \"csp:1,10\""
    echo ""
    echo "3. Start Node B (Terminal 3):"
    echo "   $UD3TN_BIN --node-id dtn://b.dtn/ --aap-port 4243 \\"
    echo "              --aap2-socket 2.aap2.socket --cla \"csp:2,10\""
else
    echo "1. Create vcan0 (once):"
    echo "   sudo modprobe vcan"
    echo "   sudo ip link add dev vcan0 type vcan"
    echo "   sudo ip link set up vcan0"
    echo ""
    echo "2. Start Node A (Terminal 1):"
    echo "   $UD3TN_BIN --node-id dtn://a.dtn/ --aap-port 4242 \\"
    echo "              --aap2-socket 1.aap2.socket --cla \"csp:1,10,can\""
    echo ""
    echo "3. Start Node B (Terminal 2):"
    echo "   $UD3TN_BIN --node-id dtn://b.dtn/ --aap-port 4243 \\"
    echo "              --aap2-socket 2.aap2.socket --cla \"csp:2,10,can\""
fi
echo ""
echo "4. Configure route A->B (Terminal 4):"
echo "   aap2-config --socket 1.aap2.socket --schedule 1 3600 100000 dtn://b.dtn/ csp:2"
echo ""
echo "5. Start receiver on B (Terminal 5):"
echo "   aap2-receive --socket 2.aap2.socket --agentid bundlesink"
echo ""
echo "6. Send from A to B (Terminal 4):"
echo "   aap2-send --socket 1.aap2.socket dtn://b.dtn/bundlesink 'hello,world!'"
echo ""
echo "=============================================="
echo "  Debugging Tips"
echo "=============================================="
echo ""
echo "1. Watch ZMQ traffic in broker terminal"
echo "   - Should see packets flowing when bundles are sent"
echo ""
echo "2. Look for these log messages in uD3TN output:"
echo "   - 'CSP: Initialized with local address X, port Y'"
echo "   - 'CSP: Starting scheduled contact to csp:X'"
echo "   - 'CSP: Sending N bytes to csp:X'"
echo "   - 'CSP: RX task started'"
echo ""
echo "3. Common issues:"
echo "   - No ZMQ broker: Packets don't reach destination"
echo "   - Wrong route: Check csp:X matches destination CSP address"
echo "   - Socket already bound: Kill old uD3TN processes"
echo ""
echo "4. Enable debug logging:"
echo "   export UD3TN_LOG_LEVEL=debug"
echo ""

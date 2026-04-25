#!/bin/bash
#
# setup-vcan-host.sh - Prepare vcan0 on the host for CAN transport CI runs
#

set -e

if ! command -v ip >/dev/null 2>&1; then
    echo "ERROR: ip command not found (install iproute2)"
    exit 1
fi

if command -v modprobe >/dev/null 2>&1; then
    sudo modprobe vcan || true
fi

if ! ip link show vcan0 >/dev/null 2>&1; then
    sudo ip link add dev vcan0 type vcan
fi

sudo ip link set up vcan0
ip link show vcan0

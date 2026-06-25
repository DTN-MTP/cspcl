#!/bin/bash
set -e

SOCKET="${UD3TN_SOCKET:-/var/run/ud3tn/ud3tn.aap2.socket}"
CP_FILE="${ASABR_CP_FILE:-/config/contact_plan.cp}"
EID_MAP="${ASABR_EID_MAP:-/config/eid_map.json}"
ROUTER_TYPE="${ASABR_ROUTER_TYPE:-VolCgrHybridParenting}"

echo "=================================================="
echo "  A-SABR Bundle Dispatch Module"
echo "=================================================="
echo "Socket:       ${SOCKET}"
echo "Contact plan: ${CP_FILE}"
echo "EID map:      ${EID_MAP}"
echo "Router type:  ${ROUTER_TYPE}"
echo "=================================================="

echo "Waiting for µD3TN socket at ${SOCKET}..."
until [ -S "${SOCKET}" ]; do sleep 1; done
echo "Socket ready."

ARGS=(--socket "${SOCKET}" "${CP_FILE}" "${EID_MAP}" --router-type "${ROUTER_TYPE}")
if [ -n "${BDM_SECRET:-}" ]; then
    ARGS+=(--secret "${BDM_SECRET}")
fi

exec python3 /opt/asabr-bdm/main.py "${ARGS[@]}"

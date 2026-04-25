#!/bin/bash
set -e
PYTHONPATH=/opt/ud3tn-src/python-ud3tn-utils \
    exec python3 /opt/ud3tn-src/python-ud3tn-utils/ud3tn_utils/aap2/bin/aap2_send.py "$@"

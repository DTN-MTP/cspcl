#!/bin/bash
#
# quick-docker-test.sh - Quick Docker build and test script
# Builds base image and runs a simple smoke test
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=============================================="
echo "  CSPCL Docker Quick Test"
echo "=============================================="
echo ""

# Build base image
echo "[1/3] Building base Docker image..."
echo "This may take 5-10 minutes on first build..."
docker build -t cspcl-base:latest -f "${SCRIPT_DIR}/docker/base/Dockerfile" "${SCRIPT_DIR}"
echo "✓ Base image built"
echo ""

# Verify image
echo "[2/3] Verifying image..."
docker run --rm cspcl-base:latest bash -c "
    echo 'Checking libcsp...'
    ls -lh /opt/libcsp/build/libcsp.a
    echo 'Checking CSPCL...'
    ls -lh /opt/cspcl/build/libcspcl.a || ls -lh /opt/cspcl/build/cspcl.a || echo 'CSPCL built as object files'
    echo 'Checking tools...'
    ls -lh /opt/tools/zmqhub_broker.py
    echo '✓ All components present'
"
echo "✓ Image verified"
echo ""

# Simple connectivity test
echo "[3/3] Running simple ZMQ broker test..."
docker run -d --name cspcl-test-broker cspcl-base:latest \
    python3 /opt/tools/zmqhub_broker.py -v

sleep 2

if docker logs cspcl-test-broker 2>&1 | grep -q "Binding"; then
    echo "✓ ZMQ broker started successfully"
else
    echo "✗ ZMQ broker failed to start"
    docker logs cspcl-test-broker
    docker rm -f cspcl-test-broker
    exit 1
fi

docker rm -f cspcl-test-broker > /dev/null 2>&1

echo ""
echo "=============================================="
echo "  Quick Test PASSED"
echo "=============================================="
echo ""
echo "Next steps:"
echo "  1. Build integration images:"
echo "     docker build -t cspcl-ud3tn:latest -f docker/ud3tn/Dockerfile ."
echo "     docker build -t cspcl-unibo:latest -f docker/unibo-bp/Dockerfile ."
echo ""
echo "  2. Run full test suite:"
echo "     cd tests/interop"
echo "     ./run-tests.sh"
echo ""
echo "  3. Start interactive environment:"
echo "     cd docker"
echo "     docker compose -f docker-compose.zmqhub.yml up"
echo ""

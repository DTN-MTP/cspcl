# CSPCL Interoperability Tests

Automated test suite for validating CSPCL integration with uD3TN and Unibo-BP DTN implementations.

## Overview

This test suite validates three critical scenarios:

1. **uD3TN Basic** - Bundle transfer between two uD3TN nodes via CSPCL
2. **Unibo-BP Basic** - Bundle transfer between two Unibo-BP nodes via CSPCL
3. **Cross-Integration** - Bidirectional bundle transfer between uD3TN and Unibo-BP

## Test Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    Test Runner                            │
│                  (run-tests.sh)                           │
└───────────┬──────────────────────────────────────────────┘
            │
            ├─ 1. Start Docker Compose (zmqhub or can)
            ├─ 2. Wait for services to be healthy
            ├─ 3. Run test scenarios
            └─ 4. Collect results and cleanup
                       │
       ┌───────────────┼───────────────┐
       ▼               ▼               ▼
  test-ud3tn-    test-unibo-    test-cross-
   basic.sh       basic.sh     integration.sh
```

## Quick Start

```bash
cd tests/interop

# Run all tests with ZMQHUB transport
./run-tests.sh

# Run specific test
./run-tests.sh --test ud3tn

# Run with CAN transport
./run-tests.sh --transport can

# Interactive mode (manual testing)
./run-tests.sh --interactive

# Bring up only the stack and run message scenarios manually
./stack-up.sh --transport zmqhub
./send-message.sh --scenario all
```

## Test Scenarios

### 1. uD3TN Basic Test (`test-ud3tn-basic.sh`)

**Validates:** uD3TN integration with CSPCL CLA

**Topology:**
```
uD3TN Node A (CSP 1) ──[CSPCL]──> uD3TN Node B (CSP 2)
    dtn://a.dtn/                      dtn://b.dtn/
```

**Steps:**
1. Wait for both uD3TN nodes to start
2. Configure route from Node A to Node B (CSP address 2)
3. Start bundle receiver on Node B (`aap2-receive`)
4. Send bundle from Node A (`aap2-send`)
5. Verify bundle reception

**Success Criteria:**
- Bundle sent successfully from Node A
- Bundle received at Node B within 10 seconds
- No CSP errors in logs

**Expected Output:**
```
==================================================
  uD3TN Basic Interoperability Test
==================================================
[1/5] Waiting for uD3TN nodes to be ready...
✓ Both nodes are running
[2/5] Configuring routes...
✓ Routes configured
[3/5] Starting bundle receiver on Node B...
✓ Receiver started
[4/5] Sending bundle from Node A...
✓ Bundle sent
[5/5] Verifying bundle reception...
✓ Bundle received on Node B

==================================================
  TEST PASSED
==================================================
```

**Run Individually:**
```bash
# Ensure Docker Compose is running
cd ../../docker
docker-compose -f docker-compose.zmqhub.yml up -d

# Run test
cd ../tests/interop
./test-ud3tn-basic.sh
```

### 2. Unibo-BP Basic Test (`test-unibo-basic.sh`)

**Validates:** Unibo-BP integration with CSPCL daemon

**Topology:**
```
Unibo Node 1 (CSP 3) ──[CSPCL]──> Unibo Node 2 (CSP 4)
    ipn:3.0 / dtn://c.dtn/           ipn:4.0 / dtn://d.dtn/
```

**Steps:**
1. Wait for both Unibo-BP nodes to start
2. Configure contact graph (regions, ranges, contacts, routes)
3. Start bundle sink on Node 2 (`unibo-bp-sink ipn:4.55`)
4. Send bundle from Node 1 (`unibo-bp-send`)
5. Verify bundle reception

**Success Criteria:**
- Contact graph configured correctly
- Bundle sent from Node 1 (ipn:3.55 → ipn:4.55)
- Bundle received at Node 2 sink
- Payload integrity maintained

**Expected Output:**
```
==================================================
  Unibo-BP Basic Interoperability Test
==================================================
[1/5] Waiting for Unibo-BP nodes to be ready...
✓ Both nodes are running
[2/5] Configuring Unibo-BP nodes...
✓ Nodes configured
[3/5] Starting bundle sink on Node 2...
✓ Receiver started
[4/5] Sending bundle from Node 1...
✓ Bundle sent
[5/5] Verifying bundle reception...
✓ Bundle received on Node 2

==================================================
  TEST PASSED
==================================================
```

**Run Individually:**
```bash
cd ../../docker
docker-compose -f docker-compose.zmqhub.yml up -d

cd ../tests/interop
./test-unibo-basic.sh
```

### 3. Cross-Integration Test (`test-cross-integration.sh`)

**Validates:** Interoperability between uD3TN and Unibo-BP via CSPCL

**Topology:**
```
Direction 1: uD3TN A (CSP 1) ──> Unibo Node 2 (CSP 4)
Direction 2: Unibo Node 1 (CSP 3) ──> uD3TN B (CSP 2)
```

**Steps:**
1. Wait for all four nodes to start
2. Configure cross-integration routes:
   - uD3TN A → Unibo 2 (CSP 4)
   - Unibo 1 → uD3TN B (CSP 2)
3. **Test Direction 1:**
   - Start sink on Unibo Node 2
   - Send from uD3TN A
   - Verify reception
4. **Test Direction 2:**
   - Start receiver on uD3TN B
   - Send from Unibo Node 1
   - Verify reception

**Success Criteria:**
- Both directions succeed
- Bundle Protocol 7 compatibility
- CSP addressing works across implementations
- Payload integrity in both directions

**Expected Output:**
```
==================================================
  Cross-Integration Interoperability Test
==================================================
[1/7] Waiting for all nodes to be ready...
✓ All nodes are running
[2/7] Configuring cross-integration routes...
✓ Cross-integration routes configured
[3/7] Testing uD3TN -> Unibo-BP direction...
✓ Bundle sent from uD3TN A
[4/7] Verifying bundle reception at Unibo Node 2...
✓ Bundle received at Unibo-BP from uD3TN
[5/7] Testing Unibo-BP -> uD3TN direction...
✓ Bundle sent from Unibo Node 1
[6/7] Verifying bundle reception at uD3TN B...
✓ Bundle received at uD3TN from Unibo-BP
[7/7] Final verification...

==================================================
  CROSS-INTEGRATION TEST PASSED
==================================================
✓ uD3TN -> Unibo-BP: SUCCESS
✓ Unibo-BP -> uD3TN: SUCCESS
```

**Run Individually:**
```bash
cd ../../docker
docker-compose -f docker-compose.zmqhub.yml up -d

cd ../tests/interop
./test-cross-integration.sh
```

## Test Runner Options

### `run-tests.sh` Usage

```bash
./run-tests.sh [OPTIONS]

Options:
  --transport TYPE     Transport: zmqhub (default) or can
  --test SUITE         Test suite: all (default), ud3tn, unibo, cross
  --interactive        Start services and enter interactive mode
  --keep-running       Keep services running after tests complete
  --no-build           Skip building Docker images
  -h, --help           Show help message
```

### Examples

```bash
# Run all tests with default (ZMQHUB)
./run-tests.sh

# Run only uD3TN test
./run-tests.sh --test ud3tn

# Run with CAN transport
./run-tests.sh --transport can

# Keep services running for manual inspection
./run-tests.sh --keep-running

# Interactive mode (no automated tests)
./run-tests.sh --interactive

# Skip image rebuild (faster iteration)
./run-tests.sh --no-build
```

> `run-tests.sh` now enforces a clean stack state between suites (`docker compose down --volumes --remove-orphans`), so sequential `--test all` runs stay deterministic across ZMQHUB and CAN.

## Interactive Mode

Interactive mode starts all services but doesn't run automated tests. Use this for manual exploration and debugging.

```bash
./run-tests.sh --interactive
```

**What you can do:**

### Send bundles manually

**uD3TN:**
```bash
docker exec cspcl-ud3tn-node-a \
  /opt/ud3tn-src/build/posix/aap2/aap2_send \
  --socket /var/run/ud3tn/ud3tn.aap2.socket \
  dtn://b.dtn/bundlesink \
  "Manual test message"
```

**Unibo-BP:**
```bash
docker exec cspcl-unibo-node-1 bash -c "
  cd /tmp/unibo-node1
  /opt/unibo-bp/bin/unibo-bp-send \
    --source ipn:3.55 \
    --destination ipn:4.55 \
    --lifetime 600000 \
    --payload-string 'Manual test'
"
```

### Receive bundles

**uD3TN:**
```bash
docker exec cspcl-ud3tn-node-b \
  /opt/ud3tn-src/build/posix/aap2/aap2_receive \
  --socket /var/run/ud3tn/ud3tn.aap2.socket \
  --agentid bundlesink \
  --count 1
```

**Unibo-BP:**
```bash
docker exec cspcl-unibo-node-2 bash -c "
  cd /tmp/unibo-node2
  /opt/unibo-bp/bin/unibo-bp-sink ipn:4.55
"
```

### View logs in real-time

```bash
docker logs -f cspcl-zmq-broker
docker logs -f cspcl-ud3tn-node-a
docker logs -f cspcl-unibo-node-1
```

### Execute shell in containers

```bash
docker exec -it cspcl-ud3tn-node-a /bin/bash
docker exec -it cspcl-unibo-node-1 /bin/bash
```

## Helper scripts for manual workflows

```bash
# Start stack only
./stack-up.sh --transport zmqhub
./stack-up.sh --transport can --prepare-host-vcan

# Send messages through CSPCL without running the full test runner
./send-message.sh --scenario ud3tn
./send-message.sh --scenario unibo
./send-message.sh --scenario cross
./send-message.sh --scenario all
```

`send-message.sh` reuses the validated scenario scripts (`test-ud3tn-basic.sh`, `test-unibo-basic.sh`, `test-cross-integration.sh`) so manual and CI flows stay aligned.

## Troubleshooting Tests

### Test Fails: "Nodes not ready"

**Cause:** Services didn't start in time

**Solution:**
```bash
# Check service status
docker-compose -f ../../docker/docker-compose.zmqhub.yml ps

# View logs
docker-compose -f ../../docker/docker-compose.zmqhub.yml logs

# Restart services
docker-compose -f ../../docker/docker-compose.zmqhub.yml restart
```

### Test Fails: "Bundle not received"

**Cause:** Routing, timing, or communication issue

**Debug steps:**
```bash
# 1. Check all processes are running
docker exec cspcl-ud3tn-node-a pgrep -fa ud3tn
docker exec cspcl-unibo-node-1 pgrep -fa unibo-bp

# 2. Check ZMQ broker
docker logs cspcl-zmq-broker

# 3. Check node logs for CSP errors
docker logs cspcl-ud3tn-node-a | grep -i error
docker logs cspcl-unibo-node-1 | grep -i error

# 4. Verify routes (see test scripts for commands)

# 5. Try manual send/receive (see Interactive Mode above)
```

### Test Fails on CAN Transport

**Cause:** vcan module or privileged containers not supported

**Check:**
```bash
# On host
lsmod | grep vcan
modprobe vcan

# In container
docker exec cspcl-vcan-setup ip link show vcan0
```

**Solution:**
- Ensure Linux kernel supports vcan
- Run Docker with privileged mode enabled
- Consider using ZMQHUB transport instead

## Adding New Tests

### Create a new test script

```bash
cd tests/interop
touch test-my-scenario.sh
chmod +x test-my-scenario.sh
```

### Test script template

```bash
#!/bin/bash
set -e

echo "=================================================="
echo "  My Test Scenario"
echo "=================================================="

# 1. Wait for services
# 2. Configure routes/nodes
# 3. Start receivers
# 4. Send bundles
# 5. Verify reception

# Exit 0 on success, 1 on failure
if [ $SUCCESS ]; then
    echo "TEST PASSED"
    exit 0
else
    echo "TEST FAILED"
    exit 1
fi
```

### Add to test runner

Edit `run-tests.sh` and add your test to the test suite logic.

## CI/CD Integration

Tests are designed for CI/CD pipelines. Exit codes indicate success/failure.

### Example GitHub Actions

```yaml
- name: Prepare VCAN for CAN transport
  run: ./tests/interop/setup-vcan-host.sh

- name: Run CSPCL tests (full stack, both transports)
  run: |
    cd tests/interop
    ./run-tests.sh --transport zmqhub
    ./run-tests.sh --transport can
```

### Example GitLab CI

```yaml
test:
  script:
    - ./tests/interop/setup-vcan-host.sh
    - cd tests/interop
    - ./run-tests.sh --transport zmqhub
    - ./run-tests.sh --transport can
  artifacts:
    when: on_failure
    paths:
      - docker-logs/
```

## Performance Expectations

### Test Duration (ZMQHUB transport)

- **uD3TN Basic:** ~15-20 seconds
- **Unibo-BP Basic:** ~20-30 seconds  
- **Cross-Integration:** ~30-45 seconds
- **Full Suite:** ~1-2 minutes

### CAN transport may be slower due to kernel module initialization.

## Further Reading

- [Docker README](../../docker/README.md) - Docker deployment guide
- [uD3TN Integration](../../ud3tn-integration/README.md) - uD3TN specifics
- [Unibo Commands](../../unibo-integration/COMMANDS.md) - Unibo-BP usage

## Contributing

Found a bug or want to add tests? See [CONTRIBUTING.md](../../CONTRIBUTING.md).

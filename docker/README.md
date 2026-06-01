# CSPCL Docker Deployment

Docker-based deployment infrastructure for CSPCL (CubeSat Space Protocol Convergence Layer) with support for both uD3TN and Unibo-BP DTN implementations.

## Quick Start

### Option 1: Use Pre-built Images from GitHub Container Registry (Recommended)

```bash
# Pull and run using pre-built images
cd cspcl/docker
docker-compose -f docker-compose.ghcr.yml up
```

**Benefits:**
- No build time required
- Consistent images from CI/CD
- Automatic updates when pulling `:latest`
- Suitable for testing and development

### Option 2: Build Locally

```bash
# Clone the repository
git clone https://github.com/dtn-mtp/cspcl.git
cd cspcl

# Build base image
docker build -t cspcl-base:latest -f docker/base/Dockerfile .

# Run all tests with ZMQHUB transport
cd tests/interop
./run-tests.sh

# Or use Docker Compose directly
cd docker
docker-compose -f docker-compose.zmqhub.yml up
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│         (Bundle sending/receiving applications)              │
└─────────────────────────────────────────────────────────────┘
                            ▲
                            │ BP7 Bundles
                            ▼
┌──────────────────┬────────────────────┬────────────────────┐
│   uD3TN Node A   │   uD3TN Node B     │   Unibo-BP Nodes   │
│   (CSP addr 1)   │   (CSP addr 2)     │   (CSP addr 3,4)   │
└──────────────────┴────────────────────┴────────────────────┘
                            │
                    ┌───────┴────────┐
                    │     CSPCL      │  ← This library
                    │  (Convergence  │
                    │     Layer)     │
                    └───────┬────────┘
                            │
                    ┌───────┴────────┐
                    │    libcsp      │
                    │   v1.6 Stack   │
                    └───────┬────────┘
                            │
            ┌───────────────┼───────────────┐
            ▼               ▼               ▼
        ZMQHUB          SocketCAN       Physical CAN
      (virtual)       (vcan0 test)    (hardware)
```

## Docker Images

All images are available on GitHub Container Registry (GHCR):
- `ghcr.io/dtn-mtp/cspcl/cspcl-base:latest`
- `ghcr.io/dtn-mtp/cspcl/cspcl-ud3tn:latest`
- `ghcr.io/dtn-mtp/cspcl/cspcl-unibo:latest`

These images are automatically built and published by CI/CD on every commit to main branch and pull requests.

### Base Image (`cspcl-base`)
- Ubuntu 22.04
- libcsp v1.6 with ZMQHUB, SocketCAN, and RDP support
- CSPCL library
- Python 3 with ZMQ broker utility

**Pull from GHCR:**
```bash
docker pull ghcr.io/dtn-mtp/cspcl/cspcl-base:latest
```

**Build locally:**
```bash
docker build -t cspcl-base:latest -f docker/base/Dockerfile .
```

### uD3TN Integration (`cspcl-ud3tn`)
- Extends base image
- uD3TN bundle processor
- CSPCL CLA (Convergence Layer Adapter)
- AAP2 tools (send, receive, config)

**Pull from GHCR:**
```bash
docker pull ghcr.io/dtn-mtp/cspcl/cspcl-ud3tn:latest
```

**Build locally:**
```bash
docker build -t cspcl-ud3tn:latest -f docker/ud3tn/Dockerfile .
```

### Unibo-BP Integration (`cspcl-unibo`)
- Extends base image
- Unibo-BP bundle processor
- CSPCL daemon
- Unibo-BP admin and messaging tools

**Pull from GHCR:**
```bash
docker pull ghcr.io/dtn-mtp/cspcl/cspcl-unibo:latest
```

**Build locally:**
```bash
docker build -t cspcl-unibo:latest -f docker/unibo-bp/Dockerfile .
```

**Note:** The Docker build now compiles Unibo-BP from source, so runtime binary mounts are not required.

## Docker Compose Configurations

### Using Pre-built Images (`docker-compose.ghcr.yml`) - Recommended

Uses images from GitHub Container Registry. No build step required.

**Services:**
- ZMQ broker (routes CSP packets)
- 2x uD3TN nodes (CSP addresses 1, 2)
- 2x Unibo-BP nodes (CSP addresses 3, 4)

**Usage:**
```bash
cd docker
docker-compose -f docker-compose.ghcr.yml up

# Run in background
docker-compose -f docker-compose.ghcr.yml up -d

# View logs
docker-compose -f docker-compose.ghcr.yml logs -f

# Stop
docker-compose -f docker-compose.ghcr.yml down
```

### ZMQHUB Transport (`docker-compose.zmqhub.yml`)

Builds images locally. Virtual messaging transport using ZeroMQ. **Recommended for development.**

**Services:**
- `zmq-broker` - ZMQ hub routing CSP packets (ports 6000/7000)
- `ud3tn-node-a` - uD3TN at CSP address 1
- `ud3tn-node-b` - uD3TN at CSP address 2
- `unibo-node-1` - Unibo-BP at CSP address 3
- `unibo-node-2` - Unibo-BP at CSP address 4

**Start:**
```bash
cd docker
docker-compose -f docker-compose.zmqhub.yml up -d
```

**Stop:**
```bash
docker-compose -f docker-compose.zmqhub.yml down
```

### CAN Transport (`docker-compose.can.yml`)

Virtual CAN interface using `vcan0`. Requires privileged containers.

**Services:**
- `vcan-setup` - Creates and manages vcan0 interface
- Node services (same as ZMQHUB but using CAN transport)

**Requirements:**
- Linux kernel with vcan module
- Privileged container support

**Start:**
```bash
cd docker
docker-compose -f docker-compose.can.yml up -d
```

## Environment Variables

### uD3TN Containers

| Variable | Description | Default |
|----------|-------------|---------|
| `CSP_ADDR` | CSP address (0-255) | `1` |
| `CSP_PORT` | CSP port for BP | `10` |
| `TRANSPORT` | Transport type (zmqhub/can) | `zmqhub` |
| `UD3TN_EID` | uD3TN endpoint identifier | `dtn://a.dtn/` |
| `ZMQ_BROKER_HOST` | ZMQ broker hostname | `zmq-broker` |

### Unibo-BP Containers

| Variable | Description | Default |
|----------|-------------|---------|
| `CSP_ADDR` | CSP address (0-255) | `1` |
| `CSP_PORT` | CSP port for BP | `10` |
| `TRANSPORT` | Transport type (zmqhub/can) | `zmqhub` |
| `UNIBO_SOCKET` | Unibo socket port | `2001` |
| `UNIBO_NODE_DIR` | Node data directory | `/tmp/unibo-node` |
| `UNIBO_ADMIN_EID` | IPN admin EID | `ipn:1.0` |
| `UNIBO_ADMIN_DTN_EID` | DTN admin EID | `dtn://a.dtn/` |
| `ZMQ_BROKER_HOST` | ZMQ broker hostname | `zmq-broker` |

## Common Operations

### View Logs

```bash
# All services
docker-compose -f docker-compose.zmqhub.yml logs -f

# Specific service
docker logs -f cspcl-ud3tn-node-a

# Last 50 lines
docker logs --tail 50 cspcl-unibo-node-1
```

### Execute Commands in Containers

```bash
# Interactive shell
docker exec -it cspcl-ud3tn-node-a /bin/bash

# Send a bundle (uD3TN)
docker exec cspcl-ud3tn-node-a \
  /opt/ud3tn-src/build/posix/aap2/aap2_send \
  --socket /var/run/ud3tn/ud3tn.aap2.socket \
  dtn://b.dtn/bundlesink \
  "Hello World"

# Send a bundle (Unibo-BP)
docker exec cspcl-unibo-node-1 bash -c "
  cd /tmp/unibo-node1
  /opt/unibo-bp/bin/unibo-bp-send \
    --source ipn:3.55 \
    --destination ipn:4.55 \
    --lifetime 600000 \
    --payload-string 'Hello from Unibo'
"
```

### Restart a Service

```bash
docker-compose -f docker-compose.zmqhub.yml restart ud3tn-node-a
```

### Check Service Health

```bash
# Health status
docker-compose -f docker-compose.zmqhub.yml ps

# Inspect specific container
docker inspect cspcl-ud3tn-node-a
```

## Running Tests

See [tests/interop/README.md](../tests/interop/README.md) for detailed test documentation.

### Automated Test Suite

```bash
cd tests/interop

# Run all tests with ZMQHUB
./run-tests.sh

# Run all tests with CAN
./run-tests.sh --transport can

# Run specific test suite
./run-tests.sh --test ud3tn
./run-tests.sh --test unibo
./run-tests.sh --test cross

# Keep services running after tests
./run-tests.sh --keep-running
```

### Manual stack bring-up and message flows

```bash
cd tests/interop

# Bring up stack only (without running tests)
./stack-up.sh --transport zmqhub
./stack-up.sh --transport can --prepare-host-vcan

# Run message scenarios on an already running stack
./send-message.sh --scenario ud3tn
./send-message.sh --scenario unibo
./send-message.sh --scenario cross
./send-message.sh --scenario all
```

### Interactive Mode

```bash
cd tests/interop
./run-tests.sh --interactive

# Services start and remain running
# You can manually send bundles and explore
```

### Individual Tests

```bash
cd tests/interop

# Ensure services are running first
cd ../../docker
docker-compose -f docker-compose.zmqhub.yml up -d

# Run individual tests
cd ../tests/interop
./test-ud3tn-basic.sh
./test-unibo-basic.sh
./test-cross-integration.sh
```

## Troubleshooting

### Port Already in Use

**Error:** `port is already allocated` or `Port 6000 is already in use`

**Solution:**
```bash
# Stop any existing services
docker-compose -f docker-compose.zmqhub.yml down

# Check for lingering processes
docker ps -a
docker rm -f $(docker ps -aq)

# Check host processes using ports
lsof -i :6000
lsof -i :7000
```

### CSP Port Binding Fails

**Error:** `CSP port 10 is already in use`

**Solution:**
```bash
# Restart the specific container
docker-compose -f docker-compose.zmqhub.yml restart ud3tn-node-a

# Or clean restart
docker-compose -f docker-compose.zmqhub.yml down
docker-compose -f docker-compose.zmqhub.yml up -d
```

### Bundle Not Received

**Checklist:**
1. Verify all services are healthy: `docker-compose ps`
2. Check logs for errors: `docker-compose logs`
3. Ensure routes are configured (see test scripts for examples)
4. Verify CSP addresses don't conflict
5. Check ZMQ broker is running (for ZMQHUB transport)
6. Verify vcan0 interface exists (for CAN transport)

**Debug:**
```bash
# Check CSP communication in logs
docker logs cspcl-zmq-broker

# Verify processes are running
docker exec cspcl-ud3tn-node-a pgrep -fa ud3tn
docker exec cspcl-unibo-node-1 pgrep -fa unibo-bp

# Check network connectivity
docker exec cspcl-ud3tn-node-a nc -zv zmq-broker 6000
```

### Image Build Fails

**Error:** `libcsp build failed` or `Unibo-BP source build failed`

**For libcsp:**
```bash
# Ensure build dependencies are installed in Dockerfile
# libcsp v1.6 requires Python 3.11 or compatible waf version
```

**For Unibo-BP:**
```bash
# Validate that the upstream source is reachable from your environment
git ls-remote https://gitlab.com/unibo-dtn/unibo-bp.git

# Rebuild the image without cache to inspect full build logs
docker build --no-cache -t cspcl-unibo:latest -f docker/unibo-bp/Dockerfile .
```

### CAN Transport Not Working

**Requirements:**
- Linux kernel with `vcan` module
- Privileged container support
- `ip` command availability

**Check:**
```bash
# On host
modprobe vcan
lsmod | grep vcan

# In vcan-setup container
docker exec cspcl-vcan-setup ip link show vcan0
```

## Volume Mounts

### Persistent Data

Volumes are automatically created for node data:
- `ud3tn-a-data` - uD3TN Node A state
- `ud3tn-b-data` - uD3TN Node B state
- `unibo-1-data` - Unibo Node 1 state
- `unibo-2-data` - Unibo Node 2 state

**Inspect volumes:**
```bash
docker volume ls
docker volume inspect ud3tn-a-data
```

**Remove volumes:**
```bash
docker-compose -f docker-compose.zmqhub.yml down -v
```

### Custom Mounts

Edit `docker-compose.*.yml` to add custom volume mounts:

```yaml
services:
  ud3tn-node-a:
    volumes:
      - ./custom-config:/config:ro
      - ./logs:/var/log/ud3tn
```

## System Requirements

- **Docker:** 20.10 or later
- **Docker Compose:** v2.0 or later (plugin version)
- **OS:** Linux (recommended), macOS, Windows with WSL2
- **Memory:** 2GB minimum, 4GB recommended
- **Disk:** 2GB for images

### For CAN Transport
- Linux kernel 4.x or later with `vcan` module
- Privileged container support

## CI/CD Integration

### GitHub Actions Example

```yaml
name: CSPCL Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Build images
        run: |
          docker build -t cspcl-base:latest -f docker/base/Dockerfile .
          
      - name: Prepare VCAN
        run: ./tests/interop/setup-vcan-host.sh

      - name: Run full-stack tests (both transports)
        run: |
          cd tests/interop
          ./run-tests.sh --transport zmqhub --no-build
          ./run-tests.sh --transport can --no-build
          
      - name: Upload logs on failure
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: docker-logs
          path: docker-logs/
```

## Further Reading

- [Main README](../../README.md) - Project overview
- [Test Documentation](../tests/interop/README.md) - Detailed test scenarios
- [uD3TN Integration Guide](../ud3tn-integration/README.md) - uD3TN specifics
- [Unibo-BP Commands](../unibo-integration/COMMANDS.md) - Unibo-BP setup

## Contributing

Found an issue or want to improve the Docker setup? See [CONTRIBUTING.md](../../CONTRIBUTING.md).

## License

MIT License - See [LICENSE](../../LICENSE) for details.

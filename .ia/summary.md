# CSPCL (CubeSat Space Protocol Convergence Layer) Requirements

## Overview

Convergence layer adapter for Bundle Protocol (BP7) over CubeSat Space Protocol (CSP).

## Architecture

```
BP7 Bundle → CSPCL (this layer) → libcsp v1.6 → Physical Interface
```

### Layer Stack

| Layer | Description |
|-------|-------------|
| **BP7** | Bundle Protocol layer (RFC 9171) |
| **CSPCL** | Convergence Layer adapter (to be implemented) |
| **CSUM** | Custom protocol layer (handles ACK/retransmissions) |
| **CSP** | CubeSat Space Protocol v1.6 (routing layer) |
| **Physical** | CAN, ZMQHUB, SocketCAN |

## CSP Configuration

### Version

- **CSP v1.6** (NOT v2 - packet format differs)

### Transport Mode

#### CSP UDP (unreliable, connectionless)
- Used by CSUM packets
- No automatic ACK at CSP level

#### CSP RDP (Reliable Data Protocol)
- Available but not currently used
- Could be used for BP packets in future
- Provides reliable transmission with ACK
- Both UDP and RDP can coexist on same network

### Disabled Features

- ❌ CSP RDP (Reliable Data Protocol) - currently unused
- ❌ CMP (CSP Management Protocol)
- ❌ HMAC authentication
- ❌ XTEA encryption
- ❌ CRC32 checksums
- ❌ PRIO field (priority)

### Port Mapping

CSP uses ports to multiplex traffic:

- **Current usage:** 1 CSP port = 1 CSUM packet type
- Different from TCP/IP where 1 port = 1 protocol
- BP packets could use separate CSP ports

## Physical Interfaces

### Ground Segment (Earth)

- **ZMQHUB:** ZeroMQ-based communication hub
- **SocketCAN:** Linux CAN bus interface

### Space Segment (Satellite)

- **CAN:** Controller Area Network bus

## Operating System Support

- **FreeRTOS:** Primary target (microcontroller-based systems)
- **POSIX:** Alternative if Linux runs on CubeSat

## CSUM Protocol Integration

### Current Architecture

- CSUM handles ACK and retransmissions (application layer protocol)
- CSUM packets transported over CSP UDP
- Used exclusively by subsystem applications
- Not used for inter-segment routing

### CSP Address Domain

- Single address space spanning both ground and space segments
- CSP handles routing between ground station ↔ satellite
- No automatic ACK between UHF/S-band ground station and satellite (currently)

### CSUM Protocol Details

- Documentation exists but not publicly available
- Handles packet acknowledgment and retransmissions
- Operates above CSP layer
- Each CSUM packet type maps to a CSP port

## BP7 Integration Strategy

### Option 1: BP over CSP UDP (No CSUM)

```
BP7 → CSPCL → CSP UDP → Physical
```

- ✅ No built-in reliability at CSP layer
- ✅ BP handles end-to-end reliability

### Option 2: BP over CSP RDP

```
BP7 → CSPCL → CSP RDP → Physical
```

- ✅ CSP provides hop-by-hop reliability
- ✅ Automatic ACK between nodes
- ✅ Could coexist with CSUM over UDP

### Option 3: BP over CSUM (Future)

```
BP7 → CSPCL → CSUM → CSP UDP → Physical
```

- ⚠️ Requires CSUM protocol specification
- ⚠️ Needs packet type detection mechanism
- ✅ CSUM handles retransmissions

## Key Design Questions

### 1. Port/Type Detection

**Problem:** BP7 bundles don't have a "port" concept like TCP/IP.

**Solutions:**
- **Fixed CSP port:** Dedicate specific CSP port(s) for BP traffic
- **Service ID mapping:** Map BP service IDs to CSP ports
- **Header inspection:** CSPCL examines bundle headers for routing

### 2. Reliability Model

| Approach | Reliability | Notes |
|----------|-------------|-------|
| **CSP UDP** | None | BP handles retransmissions end-to-end |
| **CSP RDP** | Hop-by-hop | May interfere with BP's congestion control |
| **CSUM integration** | Application-level | Requires CSUM spec, wraps BP bundles |

### 3. Fragmentation

**Challenge:** CSP packet size limits (typically ~256 bytes), BP bundles can be large (MBs)

**Options:**
1. Use BP fragmentation (RFC 9171 §5.8)
2. Implement CSPCL-level segmentation
3. Rely on CSP's built-in fragmentation (if available in v1.6)

## Implementation Scope

### Minimum Viable CSPCL

- [ ] BP7 bundle serialization (CBOR encoding/decoding)
- [ ] CSP interface (send/receive via libcsp)
- [ ] Address translation (BP endpoint IDs ↔ CSP addresses)
- [ ] Port allocation (dedicated CSP ports for BP)
- [ ] MTU handling (bundle fragmentation if needed)

### Optional Features

- [ ] CSUM protocol adapter (if spec provided)
- [ ] CSP RDP mode support
- [ ] Multiple physical interface support
- [ ] Routing table management

## Next Steps

1. **Obtain CSUM protocol specification** from CSUM team
2. **Decide on reliability strategy** (UDP/RDP/CSUM)
3. **Define CSP port allocation scheme** for BP
4. **Determine bundle fragmentation approach**
5. **Create CSPCL API specification**

---

*Last updated: [Date]*

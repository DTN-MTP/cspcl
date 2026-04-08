# CSPCL Implementation Strategy

## Recommended Approach: BP over CSP UDP

Based on the requirements and architecture analysis, **Option 1: BP over CSP UDP** is the recommended solution:

```
BP7 → CSPCL → CSP UDP → Physical
```

---

## Why This is Best

### 1. Reliability Philosophy Alignment

- ✅ **BP7 is designed for DTN** (Delay/Disruption Tolerant Networking)
- ✅ **BP already has end-to-end custody transfer** and retransmissions built-in
- ✅ **No redundant mechanisms**: Adding hop-by-hop reliability (RDP) creates conflicting mechanisms
- ✅ **CSUM is application-specific** and not publicly documented

### 2. Implementation Simplicity

- ✅ **CSP UDP is already in use** (for CSUM packets)
- ✅ **No RDP integration needed**: No state machines to implement
- ✅ **No CSUM dependency**: No need for protocol specification
- ✅ **Straightforward API**: Simple `csp_sendto()` / `csp_recvfrom()` calls

### 3. No Protocol Conflicts

- ✅ **BP's congestion control** won't interfere with CSP RDP's windowing
- ✅ **No duplicate ACKs** consuming bandwidth
- ✅ **Clean separation**: CSP routes, BP ensures delivery

### 4. Proven Pattern

- ✅ **Similar to UDPCL** (UDP Convergence Layer) in RFC 7122
- ✅ **BP over unreliable transport** is standard practice
- ✅ **ION compatibility**: NASA's BP implementation commonly runs over UDP

---

## Implementation Strategy

### Architecture Diagram

```
┌─────────────────────────────────────────────┐
│ BP7 Bundle (with custody transfer)          │
└─────────────────┬───────────────────────────┘
                  │
         ┌────────▼─────────┐
         │  CSPCL Adapter   │
         │  • Serialize     │
         │  • Fragment      │
         │  • Add header    │
         └────────┬─────────┘
                  │
         ┌────────▼─────────┐
         │  CSP UDP         │ ← Already working
         │  • Port: 10 (BP) │
         │  • Connectionless│
         └────────┬─────────┘
                  │
         ┌────────▼─────────┐
         │ CAN/ZMQHUB/      │
         │ SocketCAN        │
         └──────────────────┘
```

---

## Key Design Decisions

### 1. CSP Port Allocation

Dedicate a specific CSP port for Bundle Protocol traffic:

```c
#define CSP_PORT_BP 10  // Dedicated port for Bundle Protocol
```

### 2. Fragmentation Approach

**Use CSPCL-level fragmentation** (not BP fragmentation)

| Aspect | Decision |
|--------|----------|
| **Why** | Keeps bundles intact for custody transfer |
| **Method** | Fragment into CSP MTU chunks (~200-250 bytes) |
| **Tracking** | Simple sequence numbering in CSPCL header |

### 3. CSPCL Header Format

```c
struct cspcl_header {
    uint8_t version;          // Protocol version
    uint8_t flags;            // Fragment flags (first/last/more)
    uint16_t fragment_id;     // Fragment identifier
    uint16_t fragment_offset; // Offset in original bundle
    uint32_t bundle_size;     // Total bundle size
} __attribute__((packed));
```

**Flag Bits:**
- `0x01`: First fragment
- `0x02`: Last fragment
- `0x04`: More fragments follow

---

## Why NOT the Other Options

### ❌ Option 2: CSP RDP

| Issue | Description |
|-------|-------------|
| **Complexity** | Adds connection setup, windowing, timeouts |
| **Conflicts** | Interferes with BP's custody transfer |
| **Inefficiency** | Overkill for space-ground links (long delays make RDP inefficient) |
| **Overhead** | Duplicate ACKs consume bandwidth |

### ❌ Option 3: CSUM Integration

| Issue | Description |
|-------|-------------|
| **Missing spec** | Requires CSUM protocol specification (not available) |
| **Tight coupling** | Couples CSPCL to CSUM-specific implementation |
| **Portability** | Reduces reusability in other contexts |
| **Wrong layer** | CSUM is application-level, not transport-level |

---

## Minimal Implementation

### Core API

#### CSPCL Send Function

```c
/**
 * Send a BP7 bundle over CSP
 * @param bundle Serialized bundle data
 * @param len Bundle length in bytes
 * @param dest_addr Destination CSP address
 * @return 0 on success, -1 on error
 */
int cspcl_send_bundle(const uint8_t *bundle, size_t len,
                      uint8_t dest_addr);
```

#### CSPCL Receive Function

```c
/**
 * Receive a BP7 bundle from CSP
 * @param bundle Buffer for bundle data
 * @param len Pointer to buffer size (in) / received size (out)
 * @return 0 on success, -1 on error
 */
int cspcl_recv_bundle(uint8_t *bundle, size_t *len);
```

### Implementation Components

#### 1. Fragment/Reassembly Logic

- **Sending**: Split bundles into CSP MTU chunks
- **Header**: Add CSPCL header with sequence info
- **Receiving**: Reassemble using fragment map

#### 2. Address Mapping

```c
// BP Endpoint ID → CSP Address mapping
// ipn:1.0 → CSP addr 1
// ipn:2.0 → CSP addr 2
// ipn:3.0 → CSP addr 3

uint8_t cspcl_endpoint_to_addr(const char *endpoint_id);
```

#### 3. State Management

```c
struct cspcl_reassembly {
    uint16_t fragment_id;
    uint32_t bundle_size;
    uint8_t *buffer;
    uint32_t received_bytes;
    uint64_t timestamp;  // For timeout cleanup
};
```

---

## Expected Code Size

- **~500-800 lines** for complete CSPCL implementation
- **Uses existing libcsp v1.6** UDP functions
- **Minimal state**: Just fragment reassembly buffers

---

## Implementation Phases

### Phase 1: Basic Transmission
- [ ] Implement CSPCL header serialization
- [ ] Basic send/receive functions
- [ ] Single-packet bundles (no fragmentation)

### Phase 2: Fragmentation
- [ ] Fragment large bundles
- [ ] Reassembly state machine
- [ ] Timeout handling for incomplete fragments

### Phase 3: Address Translation
- [ ] BP endpoint ID parsing
- [ ] CSP address mapping
- [ ] Routing table integration

### Phase 4: Testing & Optimization
- [ ] Unit tests for fragmentation
- [ ] Integration tests with libcsp
- [ ] Performance optimization
- [ ] Memory leak detection

---

## Benefits Summary

| Benefit | Description |
|---------|-------------|
| **Reliability** | BP's custody transfer ensures end-to-end delivery |
| **Simplicity** | Minimal code, reuses CSP UDP infrastructure |
| **Standard** | Follows proven DTN patterns (UDPCL-like) |
| **Flexibility** | Works with existing CSUM traffic on same network |
| **Portability** | No proprietary protocol dependencies |

---

*This approach gives you BP's reliability (custody transfer, retransmissions) over CSP's routing (space ↔ ground), with minimal complexity and proven DTN patterns.*


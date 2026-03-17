---
layout: default
title: C API Reference
nav_order: 1
parent: API Reference
permalink: /api/c/
---

# C API Reference

Include the header and link against `libcspcl.a`:

```c
#include "cspcl.h"
```

---

## Types

### `cspcl_t`

Opaque instance struct. Allocate on the stack or heap and pass a pointer to every function.

```c
typedef struct {
    bool     initialized;
    uint8_t  local_addr;
    void    *rx_socket;   /* internal — do not access directly */
} cspcl_t;
```

---

### `cspcl_error_t`

Return type for all CSPCL functions.

| Value                        | Meaning                                 |
| ---------------------------- | --------------------------------------- |
| `CSPCL_OK`                   | Success                                 |
| `CSPCL_ERR_INVALID_PARAM`    | NULL pointer or out-of-range argument   |
| `CSPCL_ERR_NO_MEMORY`        | Memory allocation failed                |
| `CSPCL_ERR_BUNDLE_TOO_LARGE` | Bundle exceeds `CSPCL_MAX_BUNDLE_SIZE`  |
| `CSPCL_ERR_CSP_SEND`         | CSP send returned an error              |
| `CSPCL_ERR_CSP_RECV`         | CSP receive returned an error           |
| `CSPCL_ERR_TIMEOUT`          | Operation timed out                     |
| `CSPCL_ERR_SFP`              | SFP fragmentation or reassembly error   |
| `CSPCL_ERR_NOT_INITIALIZED`  | `cspcl_init()` was not called           |
| `CSPCL_ERR_CONNECTION`       | CSP connection could not be established |

---

## Configuration Constants

Defined in `cspcl_config.h`. Override with `-D` flags or by editing the file.

| Constant                | Default | Description                                        |
| ----------------------- | ------- | -------------------------------------------------- |
| `CSPCL_PORT_BP`         | `10`    | CSP port used for Bundle Protocol traffic          |
| `CSPCL_CSP_MTU`         | `256`   | Maximum CSP packet size (bytes)                    |
| `CSPCL_SFP_HEADER_SIZE` | `8`     | Bytes consumed by the SFP offset/totalsize header  |
| `CSPCL_MAX_PAYLOAD`     | `248`   | Usable payload per CSP packet (`MTU - SFP header`) |
| `CSPCL_MAX_BUNDLE_SIZE` | `65535` | Maximum reassembled bundle size (bytes)            |
| `CSPCL_CSP_TIMEOUT_MS`  | `1000`  | Timeout for opening a CSP connection (ms)          |
| `CSPCL_SFP_TIMEOUT_MS`  | `5000`  | Timeout for receiving an SFP fragment (ms)         |

---

## Runtime Configuration

### Environment Variables

The following environment variables can be set at runtime to configure CSPCL behavior:

| Variable                   | Type     | Description                                                    |
| -------------------------- | -------- | -------------------------------------------------------------- |
| `CSPCL_MAX_CONN_AGE_MS`    | uint32_t | Maximum age of cached connections in milliseconds. When set to a positive value, connections older than this age will be automatically invalidated and recreated. Default: `0` (disabled - connections never age out). Valid range: `0` to `4294967295`. Invalid values are logged and ignored. |

**Example:**
```bash
# Configure connections to age out after 60 seconds
export CSPCL_MAX_CONN_AGE_MS=60000
./my_cspcl_app

# Disable connection aging (default)
export CSPCL_MAX_CONN_AGE_MS=0
./my_cspcl_app
```

---

## Lifecycle Functions

### `cspcl_init`

```c
cspcl_error_t cspcl_init(cspcl_t *cspcl, uint8_t local_addr);
```

Initialize a CSPCL instance. Must be called before any other function.

| Parameter    | Description                           |
| ------------ | ------------------------------------- |
| `cspcl`      | Pointer to a `cspcl_t` to initialize  |
| `local_addr` | CSP node address of this node (0-255) |

**Returns** `CSPCL_OK` on success.

---

### `cspcl_cleanup`

```c
void cspcl_cleanup(cspcl_t *cspcl);
```

Release all resources held by the instance. Closes the RX socket if open.

---

## Socket Functions

### `cspcl_open_rx_socket`

```c
cspcl_error_t cspcl_open_rx_socket(cspcl_t *cspcl);
```

Create and bind a server socket to `CSPCL_PORT_BP`. Call once after `cspcl_init()`.
Calling again when already open is a no-op.

---

### `cspcl_close_rx_socket`

```c
void cspcl_close_rx_socket(cspcl_t *cspcl);
```

Close the server socket. Called automatically by `cspcl_cleanup()`.

---

## Bundle Transfer Functions

### `cspcl_send_bundle`

```c
cspcl_error_t cspcl_send_bundle(cspcl_t       *cspcl,
                                 const uint8_t *bundle,
                                 size_t         len,
                                 uint8_t        dest_addr);
```

Send a serialized BP7 bundle to `dest_addr`. Opens a new CSP connection, fragments
the bundle via SFP using `CSPCL_MAX_PAYLOAD` as the MTU, then closes the connection.

| Parameter   | Description                       |
| ----------- | --------------------------------- |
| `cspcl`     | Initialized CSPCL instance        |
| `bundle`    | Pointer to serialized bundle data |
| `len`       | Bundle length in bytes            |
| `dest_addr` | Destination CSP node address      |

**Returns** `CSPCL_OK` on success, or one of `CSPCL_ERR_BUNDLE_TOO_LARGE`,
`CSPCL_ERR_CONNECTION`, `CSPCL_ERR_CSP_SEND`.

---

### `cspcl_recv_bundle`

```c
cspcl_error_t cspcl_recv_bundle(cspcl_t  *cspcl,
                                 uint8_t  *bundle,
                                 size_t   *len,
                                 uint8_t  *src_addr,
                                 uint32_t  timeout_ms);
```

Block until a complete bundle is received, or until `timeout_ms` milliseconds elapse.
Accepts an incoming connection, reassembles all SFP fragments, and copies the result
into `bundle`.

| Parameter    | Description                                          |
| ------------ | ---------------------------------------------------- |
| `cspcl`      | Initialized CSPCL instance (RX socket must be open)  |
| `bundle`     | Output buffer                                        |
| `len`        | In: buffer capacity. Out: bytes written              |
| `src_addr`   | Out: source CSP node address (may be `NULL`)         |
| `timeout_ms` | Receive timeout in ms; `0` uses the internal default |

**Returns** `CSPCL_OK` on success, or one of `CSPCL_ERR_TIMEOUT`, `CSPCL_ERR_SFP`,
`CSPCL_ERR_NO_MEMORY`.

---

## Address Translation

### `cspcl_endpoint_to_addr`

```c
uint8_t cspcl_endpoint_to_addr(const char *endpoint_id);
```

Parse a BP endpoint ID and return the corresponding CSP node address.

| Endpoint ID        | CSP address                  |
| ------------------ | ---------------------------- |
| `ipn:5.0`          | `5`                          |
| `ipn:5.42`         | `5` (service number ignored) |
| `dtn://node7/sink` | `7`                          |

Returns `0` on parse error.

---

### `cspcl_addr_to_endpoint`

```c
cspcl_error_t cspcl_addr_to_endpoint(uint8_t addr, char *endpoint, size_t len);
```

Convert CSP node address `addr` to an IPN endpoint string of the form `ipn:<addr>.0`.
Buffer must be at least 12 bytes.

---

## Utility

### `cspcl_strerror`

```c
const char *cspcl_strerror(cspcl_error_t err);
```

Return a human-readable string for an error code.

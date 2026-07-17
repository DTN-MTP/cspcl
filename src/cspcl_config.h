// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
/**
 * @file cspcl_config.h
 * @brief CSPCL Configuration Constants
 *
 * Central configuration for CSPCL library and integrations.
 * All integration-specific and core configuration values are defined here
 * to ensure consistency across implementations.
 *
 * @version 1.0
 */

#ifndef CSPCL_CONFIG_H
#define CSPCL_CONFIG_H

/* ========================================================================== */
/* CSP Configuration                                                          */
/* ========================================================================== */

/** CSP port dedicated to Bundle Protocol traffic */
#ifndef CSPCL_PORT_BP
#define CSPCL_PORT_BP 10
#endif

/** CSP connection timeout in milliseconds */
#ifndef CSPCL_CSP_TIMEOUT_MS
#define CSPCL_CSP_TIMEOUT_MS 1000
#endif

/** CSP connection options for outbound connections (bitmask of CSP_O_*) */
#ifndef CSPCL_CSP_CONN_OPTIONS
#define CSPCL_CSP_CONN_OPTIONS CSP_O_RDP
#endif

/** CSP socket options for inbound server sockets (bitmask of CSP_SO_*) */
#ifndef CSPCL_CSP_SOCKET_OPTIONS
/* Temporarily disabled RDPREQ to diagnose socket acceptance issue */
#define CSPCL_CSP_SOCKET_OPTIONS 0
#endif

/* ========================================================================== */
/* Fragmentation Configuration                                                */
/* ========================================================================== */

/** Maximum CSP MTU size */
#ifndef CSPCL_CSP_MTU
#define CSPCL_CSP_MTU 256
#endif

/** Maximum bundle size supported (64KB) */
#ifndef CSPCL_MAX_BUNDLE_SIZE
#define CSPCL_MAX_BUNDLE_SIZE 65535
#endif

/** SFP header size (offset + totalsize = 8 bytes) */
#ifndef CSPCL_SFP_HEADER_SIZE
#define CSPCL_SFP_HEADER_SIZE 8
#endif

/** CSP RDP header size */
#ifndef CSPCL_CSP_RDP_HEADER_SIZE
#define CSPCL_CSP_RDP_HEADER_SIZE 5
#endif

/** Maximum user-data bytes per SFP fragment */
#ifndef CSPCL_MAX_PAYLOAD
#define CSPCL_MAX_PAYLOAD (CSPCL_CSP_MTU - CSPCL_SFP_HEADER_SIZE - CSPCL_CSP_RDP_HEADER_SIZE)
#endif

/** CSP SFP receive timeout in milliseconds */
#ifndef CSPCL_SFP_TIMEOUT_MS
#define CSPCL_SFP_TIMEOUT_MS 5000
#endif

/* ========================================================================== */
/* Logging Configuration                                                      */
/* ========================================================================== */

/** Debug logging macro - define to enable debug output */
#ifndef CSPCL_DEBUG
#define CSPCL_DEBUG(fmt, ...)
#endif

/** Info logging macro */
#ifndef CSPCL_LOG
#define CSPCL_LOG(fmt, ...)
#endif

/** Warning logging macro */
#ifndef CSPCL_WARN
#define CSPCL_WARN(fmt, ...)
#endif

/* ========================================================================== */
/* Connection Pool Configuration                                              */
/* ========================================================================== */

/** Maximum number of cached outbound CSP connections */
#ifndef CSPCL_CONN_POOL_SIZE
#define CSPCL_CONN_POOL_SIZE 16
#endif

/** Maximum number of live inbound CSP connections kept open for reading.
 *  Senders pool their outbound connections and reuse them for multiple
 *  bundles, so the receiver must keep accepted connections open and keep
 *  reading from them instead of closing after the first bundle. */
#ifndef CSPCL_RX_CONN_TABLE_SIZE
#define CSPCL_RX_CONN_TABLE_SIZE 8
#endif

/** How long cspcl_recv_bundle() polls csp_accept() for new inbound
 *  connections when it already has live connections to read from */
#ifndef CSPCL_RX_ACCEPT_POLL_MS
#define CSPCL_RX_ACCEPT_POLL_MS 100
#endif

/* ========================================================================== */
/* Buffer Management (for integrations)                                       */
/* ========================================================================== */

/** Standard RX buffer size (bytes) - used by all integrations
 *  Can be overridden at compile time: -DCSPCL_RX_BUFFER_SIZE=131072 */
#ifndef CSPCL_RX_BUFFER_SIZE
#define CSPCL_RX_BUFFER_SIZE 65536
#endif

/** Standard TX buffer size (bytes) - used by all integrations
 *  Can be overridden at compile time: -DCSPCL_TX_BUFFER_SIZE=131072 */
#ifndef CSPCL_TX_BUFFER_SIZE
#define CSPCL_TX_BUFFER_SIZE 65536
#endif

/** Chunk size for streaming operations (e.g., Unibo integration)
 *  Used when reading/writing bundles in fixed-size chunks */
#ifndef CSPCL_STREAM_CHUNK_SIZE
#define CSPCL_STREAM_CHUNK_SIZE 4096
#endif

/* ========================================================================== */
/* Connection & Link Management                                               */
/* ========================================================================== */

/** Maximum concurrent links/peers managed by integrations */
#ifndef CSPCL_MAX_LINKS
#define CSPCL_MAX_LINKS 16
#endif

/** CSP socket receive timeout (milliseconds) */
#ifndef CSPCL_SOCKET_TIMEOUT_MS
#define CSPCL_SOCKET_TIMEOUT_MS 1000
#endif

/* ========================================================================== */
/* Interface Defaults                                                          */
/* ========================================================================== */

/** Default ZMQHUB server address */
#ifndef CSPCL_ZMQHUB_ADDR_DEFAULT
#define CSPCL_ZMQHUB_ADDR_DEFAULT "localhost"
#endif

/** Default CAN interface name */
#ifndef CSPCL_CAN_IFACE_DEFAULT
#define CSPCL_CAN_IFACE_DEFAULT "vcan0"
#endif

/** Maximum length for interface parameters (host, device name, etc.) */
#ifndef CSPCL_IFACE_PARAM_MAX
#define CSPCL_IFACE_PARAM_MAX 64
#endif

/* ========================================================================== */
/* CSP Stack Configuration                                                    */
/* ========================================================================== */

/** CSP buffer count (core library global setting) */
#ifndef CSPCL_CSP_BUFFERS
#define CSPCL_CSP_BUFFERS 100
#endif

/** CSP buffer data size (bytes) - must match CSPCL_CSP_MTU */
#ifndef CSPCL_CSP_BUFFER_DATA_SIZE
#define CSPCL_CSP_BUFFER_DATA_SIZE 256
#endif

/** CSP connection queue length */
#ifndef CSPCL_CSP_CONN_QUEUE_LENGTH
#define CSPCL_CSP_CONN_QUEUE_LENGTH 100
#endif

/** CSP FIFO length */
#ifndef CSPCL_CSP_FIFO_LENGTH
#define CSPCL_CSP_FIFO_LENGTH 25
#endif

/** CSP maximum port to bind (0-31 for CSP v1.6) */
#ifndef CSPCL_CSP_PORT_MAX_BIND
#define CSPCL_CSP_PORT_MAX_BIND 31
#endif

/** CSP RDP maximum window size */
#ifndef CSPCL_CSP_RDP_MAX_WINDOW
#define CSPCL_CSP_RDP_MAX_WINDOW 20
#endif

/* ========================================================================== */
/* Runtime Defaults                                                            */
/* ========================================================================== */

/** Default CLA name for integrations */
#ifndef CSPCL_CLA_NAME_DEFAULT
#define CSPCL_CLA_NAME_DEFAULT "csp"
#endif

/** CSP model identification string */
#ifndef CSPCL_CSP_MODEL
#define CSPCL_CSP_MODEL "csp-cla"
#endif

/** CSP model revision */
#ifndef CSPCL_CSP_REVISION
#define CSPCL_CSP_REVISION "1.0"
#endif

#endif /* CSPCL_CONFIG_H */

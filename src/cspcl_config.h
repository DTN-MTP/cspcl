/**
 * @file cspcl_config.h
 * @brief CSPCL Configuration Options
 *
 * User-configurable options for the CSPCL convergence layer.
 * Copy this file and modify values as needed for your platform.
 */

#ifndef CSPCL_CONFIG_H
#define CSPCL_CONFIG_H

/*===========================================================================*/
/* Platform Selection                                                         */
/*===========================================================================*/

/**
 * Select the platform/OS for time functions and threading.
 * Uncomment ONE of the following:
 */

/* Linux/POSIX platform */
#define CSPCL_PLATFORM_POSIX

/* FreeRTOS platform */
/* #define CSPCL_PLATFORM_FREERTOS */

/*===========================================================================*/
/* CSP Configuration                                                          */
/*===========================================================================*/

/** CSP port dedicated to Bundle Protocol traffic */
#ifndef CSPCL_PORT_BP
#define CSPCL_PORT_BP 10
#endif

/** CSP connection timeout in milliseconds */
#ifndef CSPCL_CSP_TIMEOUT_MS
#define CSPCL_CSP_TIMEOUT_MS 1000
#endif

/** RDP window size for new outgoing connections */
#ifndef CSPCL_RDP_WINDOW_SIZE
#define CSPCL_RDP_WINDOW_SIZE 4
#endif

/** RDP connection timeout in milliseconds */
#ifndef CSPCL_RDP_CONN_TIMEOUT_MS
#define CSPCL_RDP_CONN_TIMEOUT_MS 1500
#endif

/** RDP retransmission interval in milliseconds */
#ifndef CSPCL_RDP_PACKET_TIMEOUT_MS
#define CSPCL_RDP_PACKET_TIMEOUT_MS 300
#endif

/** Enable delayed RDP acknowledgements */
#ifndef CSPCL_RDP_DELAYED_ACKS
#define CSPCL_RDP_DELAYED_ACKS 0
#endif

/** RDP delayed ACK timeout in milliseconds */
#ifndef CSPCL_RDP_ACK_TIMEOUT_MS
#define CSPCL_RDP_ACK_TIMEOUT_MS 0
#endif

/** RDP delayed ACK packet count */
#ifndef CSPCL_RDP_ACK_DELAY_COUNT
#define CSPCL_RDP_ACK_DELAY_COUNT 0
#endif

/** CSP connection options for outbound connections (bitmask of CSP_O_*) */
#ifndef CSPCL_CSP_CONN_OPTIONS
#define CSPCL_CSP_CONN_OPTIONS CSP_O_RDP
#endif

/** CSP socket options for inbound server sockets (bitmask of CSP_SO_*) */
#ifndef CSPCL_CSP_SOCKET_OPTIONS
#define CSPCL_CSP_SOCKET_OPTIONS CSP_SO_RDPREQ
#endif

/*===========================================================================*/
/* Fragmentation Configuration                                                */
/*===========================================================================*/

/**
 * Maximum CSP MTU size
 * Typical values:
 *   - CAN: 256 bytes (8 bytes per CAN frame, fragmented by CSP)
 *   - UART: 256 bytes
 *   - TCP/IP: 1400 bytes
 */
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

/** CSP SFP receive timeout in milliseconds */
#ifndef CSPCL_SFP_TIMEOUT_MS
#define CSPCL_SFP_TIMEOUT_MS 5000
#endif

/*===========================================================================*/
/* Debug Configuration                                                        */
/*===========================================================================*/

/** Enable debug output */
/* #define CSPCL_DEBUG */

/** Enable verbose debug output */
/* #define CSPCL_DEBUG_VERBOSE */

#ifdef CSPCL_DEBUG
#include <stdio.h>
#define CSPCL_LOG(fmt, ...) printf("[CSPCL] " fmt "\n", ##__VA_ARGS__)
#define CSPCL_DEBUG(fmt, ...) printf("[CSPCL DEBUG] " fmt "\n", ##__VA_ARGS__)
#define CSPCL_WARN(fmt, ...) printf("[CSPCL WARN] " fmt "\n", ##__VA_ARGS__)
#else
#define CSPCL_LOG(fmt, ...)
#define CSPCL_DEBUG(fmt, ...)
#define CSPCL_WARN(fmt, ...)
#endif

#ifdef CSPCL_DEBUG_VERBOSE
#define CSPCL_LOG_V(fmt, ...) printf("[CSPCL] " fmt "\n", ##__VA_ARGS__)
#else
#define CSPCL_LOG_V(fmt, ...)
#endif

#endif /* CSPCL_CONFIG_H */

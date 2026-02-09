/**
 * @file cspcl.h
 * @brief CSPCL - CubeSat Space Protocol Convergence Layer for Bundle Protocol
 *
 * This convergence layer adapter enables BP7 bundles to be transmitted over
 * CSP (CubeSat Space Protocol) using UDP mode (connectionless, unreliable).
 *
 * Architecture:
 *   BP7 Bundle → CSPCL → CSP UDP → Physical (CAN/ZMQHUB/SocketCAN)
 *
 * @version 1.0
 * @note Designed for CSP v1.6 (not v2)
 */

#ifndef CSPCL_H
#define CSPCL_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/*===========================================================================*/
/* Configuration Constants                                                    */
/*===========================================================================*/

/** Dedicated CSP port for Bundle Protocol traffic */
#define CSPCL_PORT_BP               10

/** CSPCL protocol version */
#define CSPCL_VERSION               1

/** Maximum CSP MTU (typical CAN-based CSP MTU) */
#define CSPCL_CSP_MTU               256

/** CSPCL header size */
#define CSPCL_HEADER_SIZE           10

/** Maximum payload per CSP packet */
#define CSPCL_MAX_PAYLOAD           (CSPCL_CSP_MTU - CSPCL_HEADER_SIZE)

/** Maximum bundle size supported */
#define CSPCL_MAX_BUNDLE_SIZE       65535

/** Maximum number of concurrent reassembly contexts */
#define CSPCL_MAX_REASSEMBLY_CTX    8

/** Reassembly timeout in milliseconds */
#define CSPCL_REASSEMBLY_TIMEOUT_MS 30000

/** CSP connection timeout in milliseconds */
#define CSPCL_CSP_TIMEOUT_MS        1000

/*===========================================================================*/
/* CSPCL Header Flags                                                         */
/*===========================================================================*/

/** First fragment of a bundle */
#define CSPCL_FLAG_FIRST            0x01

/** Last fragment of a bundle */
#define CSPCL_FLAG_LAST             0x02

/** More fragments follow */
#define CSPCL_FLAG_MORE             0x04

/*===========================================================================*/
/* CSPCL Header Structure                                                     */
/*===========================================================================*/

/**
 * @brief CSPCL packet header
 *
 * This header is prepended to each CSP packet containing bundle data.
 * It enables fragmentation and reassembly of bundles larger than CSP MTU.
 */
typedef struct __attribute__((packed)) {
    uint8_t  version;           /**< Protocol version (CSPCL_VERSION) */
    uint8_t  flags;             /**< Fragment flags (FIRST/LAST/MORE) */
    uint16_t fragment_id;       /**< Unique identifier for this bundle transfer */
    uint16_t fragment_offset;   /**< Offset of this fragment in original bundle */
    uint32_t bundle_size;       /**< Total size of the complete bundle */
} cspcl_header_t;

/*===========================================================================*/
/* Error Codes                                                                */
/*===========================================================================*/

typedef enum {
    CSPCL_OK = 0,               /**< Success */
    CSPCL_ERR_INVALID_PARAM,    /**< Invalid parameter */
    CSPCL_ERR_NO_MEMORY,        /**< Memory allocation failed */
    CSPCL_ERR_BUNDLE_TOO_LARGE, /**< Bundle exceeds maximum size */
    CSPCL_ERR_CSP_SEND,         /**< CSP send failed */
    CSPCL_ERR_CSP_RECV,         /**< CSP receive failed */
    CSPCL_ERR_TIMEOUT,          /**< Operation timed out */
    CSPCL_ERR_REASSEMBLY,       /**< Reassembly failed */
    CSPCL_ERR_VERSION_MISMATCH, /**< Protocol version mismatch */
    CSPCL_ERR_NOT_INITIALIZED,  /**< CSPCL not initialized */
    CSPCL_ERR_INVALID_HEADER,   /**< Invalid CSPCL header */
} cspcl_error_t;

/*===========================================================================*/
/* Reassembly Context                                                         */
/*===========================================================================*/

/**
 * @brief Context for reassembling fragmented bundles
 */
typedef struct {
    bool        in_use;             /**< Context slot is active */
    uint8_t     src_addr;           /**< Source CSP address */
    uint16_t    fragment_id;        /**< Fragment ID being reassembled */
    uint32_t    bundle_size;        /**< Expected total bundle size */
    uint8_t    *buffer;             /**< Reassembly buffer */
    uint32_t    received_bytes;     /**< Bytes received so far */
    uint32_t    received_mask[256]; /**< Bitmask of received fragments (8KB coverage) */
    uint64_t    timestamp_ms;       /**< Creation timestamp for timeout */
} cspcl_reassembly_ctx_t;

/*===========================================================================*/
/* CSPCL Instance                                                             */
/*===========================================================================*/

/**
 * @brief CSPCL instance configuration and state
 */
typedef struct {
    bool        initialized;        /**< Instance is initialized */
    uint8_t     local_addr;         /**< Local CSP address */
    uint16_t    next_fragment_id;   /**< Next fragment ID to use */
    void       *rx_socket;          /**< Persistent RX socket (csp_socket_t*) */
    cspcl_reassembly_ctx_t reassembly[CSPCL_MAX_REASSEMBLY_CTX];
} cspcl_t;

/*===========================================================================*/
/* Initialization Functions                                                   */
/*===========================================================================*/

/**
 * @brief Initialize the CSPCL instance
 *
 * @param cspcl     Pointer to CSPCL instance
 * @param local_addr Local CSP address for this node
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_init(cspcl_t *cspcl, uint8_t local_addr);

/**
 * @brief Cleanup and free CSPCL resources
 *
 * @param cspcl     Pointer to CSPCL instance
 */
void cspcl_cleanup(cspcl_t *cspcl);

/*===========================================================================*/
/* Bundle Transmission Functions                                              */
/*===========================================================================*/

/**
 * @brief Send a BP7 bundle over CSP
 *
 * This function fragments the bundle if necessary and transmits it
 * over CSP UDP to the specified destination.
 *
 * @param cspcl     Pointer to CSPCL instance
 * @param bundle    Serialized bundle data
 * @param len       Bundle length in bytes
 * @param dest_addr Destination CSP address
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_send_bundle(cspcl_t *cspcl,
                                 const uint8_t *bundle,
                                 size_t len,
                                 uint8_t dest_addr);

/**
 * @brief Receive a BP7 bundle from CSP
 *
 * This function receives CSP packets and reassembles them into
 * a complete bundle. It blocks until a complete bundle is received
 * or timeout occurs.
 *
 * @param cspcl     Pointer to CSPCL instance
 * @param bundle    Buffer for bundle data
 * @param len       Pointer to buffer size (in) / received size (out)
 * @param src_addr  Pointer to store source CSP address (can be NULL)
 * @param timeout_ms Timeout in milliseconds (0 = no timeout)
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_recv_bundle(cspcl_t *cspcl,
                                 uint8_t *bundle,
                                 size_t *len,
                                 uint8_t *src_addr,
                                 uint32_t timeout_ms);

/**
 * @brief Open and bind persistent receive socket
 *
 * Call this once during initialization to create a socket bound
 * to the BP port for receiving bundles. The socket is automatically
 * opened on first recv if not already open.
 *
 * @param cspcl     Pointer to CSPCL instance
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_open_rx_socket(cspcl_t *cspcl);

/**
 * @brief Close the receive socket
 *
 * @param cspcl     Pointer to CSPCL instance
 */
void cspcl_close_rx_socket(cspcl_t *cspcl);

/*===========================================================================*/
/* Address Translation Functions                                              */
/*===========================================================================*/

/**
 * @brief Convert BP endpoint ID to CSP address
 *
 * Supports IPN scheme: ipn:X.Y → CSP address X
 *
 * @param endpoint_id BP endpoint ID string (e.g., "ipn:1.0")
 * @return CSP address, or 0 on error
 */
uint8_t cspcl_endpoint_to_addr(const char *endpoint_id);

/**
 * @brief Convert CSP address to BP endpoint ID
 *
 * @param addr      CSP address
 * @param endpoint  Buffer for endpoint ID string
 * @param len       Buffer length
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_addr_to_endpoint(uint8_t addr,
                                      char *endpoint,
                                      size_t len);

/*===========================================================================*/
/* Utility Functions                                                          */
/*===========================================================================*/

/**
 * @brief Get current timestamp in milliseconds
 *
 * @return Current time in milliseconds
 */
uint64_t cspcl_get_time_ms(void);

/**
 * @brief Clean up expired reassembly contexts
 *
 * @param cspcl Pointer to CSPCL instance
 */
void cspcl_cleanup_expired(cspcl_t *cspcl);

/**
 * @brief Get error string for error code
 *
 * @param err Error code
 * @return Human-readable error string
 */
const char *cspcl_strerror(cspcl_error_t err);

#ifdef __cplusplus
}
#endif

#endif /* CSPCL_H */


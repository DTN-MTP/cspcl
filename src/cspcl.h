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
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifndef FREERTOS
#include <pthread.h>
#include <time.h>
#else
#include <FreeRTOS.h>
#include <semphr.h>
#endif

#include <csp/csp.h>
#include <csp/csp_types.h>
/* libcsp headers for direct CSP stack control */
#include <csp/csp_rtable.h>
#include <csp/interfaces/csp_if_zmqhub.h>
/* CAN interface support - requires libcsp built with CAN driver */
#ifdef CSP_HAVE_LIBSOCKETCAN
#include <csp/drivers/can_socketcan.h>
#include <csp/interfaces/csp_if_can.h>

#endif
#define CSP_IFACE_PARAM_MAX 64

#ifdef __cplusplus
extern "C" {
#endif

/*===========================================================================*/
/* Configuration Constants                                                    */
/*===========================================================================*/

/** Dedicated CSP port for Bundle Protocol traffic */
#define CSPCL_PORT_BP 10

/** Maximum CSP MTU (typical CAN-based CSP MTU) */
#define CSPCL_CSP_MTU 256

/** SFP header size (offset + totalsize = 8 bytes) */
#define CSPCL_SFP_HEADER_SIZE 8

/** Maximum payload per CSP packet when using SFP */
#define CSPCL_MAX_PAYLOAD (CSPCL_CSP_MTU - CSPCL_SFP_HEADER_SIZE)

/** Maximum bundle size supported */
#define CSPCL_MAX_BUNDLE_SIZE 65535

/** Maximum number of cached outbound CSP connections */
#define CSPCL_CONN_POOL_SIZE 16

/** CSP connection timeout in milliseconds */
#define CSPCL_CSP_TIMEOUT_MS 1000

/** CSP SFP receive timeout in milliseconds */
#define CSPCL_SFP_TIMEOUT_MS 5000

/*===========================================================================*/
/* Error Codes                                                                */
/*===========================================================================*/

typedef enum {
  CSPCL_OK = 0,                    /**< Success */
  CSPCL_ERR_INVALID_PARAM,         /**< Invalid parameter */
  CSPCL_ERR_NO_MEMORY,             /**< Memory allocation failed */
  CSPCL_ERR_BUNDLE_TOO_LARGE,      /**< Bundle exceeds maximum size */
  CSPCL_ERR_CSP_SEND,              /**< CSP send failed */
  CSPCL_ERR_CSP_RECV,              /**< CSP receive failed */
  CSPCL_ERR_TIMEOUT,               /**< Operation timed out */
  CSPCL_ERR_SFP,                   /**< SFP fragmentation/reassembly error */
  CSPCL_ERR_NOT_INITIALIZED,       /**< CSPCL not initialized */
  CSPCL_ERR_CONNECTION,            /**< CSP connection error */
  CSPCL_ERR_CSPINIT,               /**< CSP init error (generic) */
  CSPCL_ERR_CSP_STACK_INIT,        /**< csp_init() failed */
  CSPCL_ERR_CSP_ZMQHUB_INIT,       /**< ZMQ hub interface init failed */
  CSPCL_ERR_CSP_CAN_INIT,          /**< CAN interface init failed */
  CSPCL_ERR_CSP_CAN_NOT_SUPPORTED, /**< CAN support not compiled in */
  CSPCL_ERR_CSP_ROUTER,            /**< CSP router task start failed */
  CSPCL_ERR_POOL_FULL              /**< Connection pool full, LRU eviction was forced */
} cspcl_error_t;

enum csp_iface_type {
  CSP_IFACE_ZMQHUB,  /* ZeroMQ hub - for testing/ground segment */
  CSP_IFACE_CAN,     /* CAN bus - for space segment */
  CSP_IFACE_LOOPBACK /* Loopback - for local testing */
};

/*===========================================================================*/
/* Connection Pool                                                           */
/*===========================================================================*/

/**
 * @brief Pool operation statistics counters
 */
typedef struct {
  uint32_t hits;             /**< Cache hits: existing connection reused */
  uint32_t misses;           /**< Cache misses: new connection created */
  uint32_t evictions;        /**< LRU evictions due to pool full */
  uint32_t connect_failures; /**< Failed csp_connect() calls */
  uint32_t invalidations;    /**< Connections invalidated (send error or age) */
} cspcl_conn_pool_stats_t;

typedef struct {
  bool used;
  uint8_t dest_addr;
  uint8_t dest_port;
  csp_conn_t *conn;
  uint32_t last_used; /**< Monotonic tick at last access, for LRU eviction */
#ifndef FREERTOS
  time_t connected_at; /**< Wall-clock time of connection creation */
#endif
} cspcl_conn_pool_entry_t;

typedef struct {
  bool initialized;
#ifndef FREERTOS
  pthread_mutex_t lock;
#else
  SemaphoreHandle_t lock;
#endif
  cspcl_conn_pool_entry_t entries[CSPCL_CONN_POOL_SIZE];
  uint32_t tick;            /**< Monotonic counter incremented on each access */
  uint32_t max_conn_age_ms; /**< Max connection age in ms (0 = disabled) */
  cspcl_conn_pool_stats_t stats; /**< Pool operation counters */
} cspcl_conn_pool_t;

/*===========================================================================*/
/* CSPCL Instance                                                             */
/*===========================================================================*/

/**
 * @brief CSPCL instance configuration and state
 *
 * Uses CSP's SFP (Simple Fragmentation Protocol) for automatic
 * fragmentation and reassembly of bundles.
 */
typedef struct {
  bool initialized;   /**< Instance is initialized */
  uint8_t local_addr; /**< Local CSP address */
  void *
      rx_socket; /**< Server socket for accepting connections (csp_socket_t*) */

  /* CSP port for BP traffic */
  uint8_t csp_port;

  /* Interface selection */
  enum csp_iface_type iface_type;

  /* ZMQHUB: broker host (e.g. "localhost" or "192.168.1.10") */
  char zmqhub_addr[CSP_IFACE_PARAM_MAX];

  /* CAN: SocketCAN interface name (e.g. "vcan0" or "can0") */
  char can_iface[CSP_IFACE_PARAM_MAX];

  /* Internal outbound CSP connection pool */
  cspcl_conn_pool_t conn_pool;

  csp_iface_t *active_iface;
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
cspcl_error_t cspcl_init(cspcl_t *cspcl);

/**
 * @brief Cleanup and free CSPCL resources
 *
 * @param cspcl     Pointer to CSPCL instance
 */
void cspcl_cleanup(cspcl_t *cspcl);

/**
 * @brief Initialize a connection pool used for outbound bundle traffic
 *
 * @param pool      Pointer to pool instance
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_conn_pool_init(cspcl_conn_pool_t *pool);

/**
 * @brief Close all cached connections and release pool resources
 *
 * @param pool      Pointer to pool instance
 */
void cspcl_conn_pool_cleanup(cspcl_conn_pool_t *pool);

/**
 * @brief Read pool statistics counters
 *
 * @param pool   Pointer to pool instance (may be NULL — safe no-op)
 * @param stats  Output buffer to fill
 */
void cspcl_conn_pool_get_stats(const cspcl_conn_pool_t *pool,
                               cspcl_conn_pool_stats_t *stats);

/*===========================================================================*/
/* Bundle Transmission Functions                                              */
/*===========================================================================*/

/**
 * @brief Send a BP7 bundle over CSP
 *
 * This function uses CSP's SFP (Simple Fragmentation Protocol) to
 * automatically fragment large bundles if necessary and transmit
 * them over a reliable CSP connection to the specified destination.
 *
 * @param cspcl     Pointer to CSPCL instance
 * @param bundle    Serialized bundle data
 * @param len       Bundle length in bytes
 * @param dest_addr Destination CSP address
 * @param dest_port Destination CSP port
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_send_bundle(cspcl_t *cspcl, const uint8_t *bundle,
                                size_t len, uint8_t dest_addr,
                                uint8_t dest_port);

/**
 * @brief Receive a BP7 bundle from CSP
 *
 * This function accepts incoming CSP connections and uses SFP
 * (Simple Fragmentation Protocol) to automatically reassemble
 * fragmented bundles. It blocks until a complete bundle is received
 * or timeout occurs.
 *
 * @param cspcl     Pointer to CSPCL instance
 * @param bundle    Buffer for bundle data
 * @param len       Pointer to buffer size (in) / received size (out)
 * @param src_addr  Pointer to store source CSP address (can be NULL)
 * @param timeout_ms Timeout in milliseconds (0 = no timeout)
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_recv_bundle(cspcl_t *cspcl, uint8_t *bundle, size_t *len,
                                uint8_t *src_addr, uint8_t *src_port,
                                uint32_t timeout_ms);

/**
 * @brief Open and bind server socket for incoming connections
 *
 * Call this once during initialization to create a socket bound
 * to the BP port for receiving bundle connections.
 *
 * @param cspcl     Pointer to CSPCL instance
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_open_rx_socket(cspcl_t *cspcl);

/**
 * @brief Close the server socket
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
cspcl_error_t cspcl_addr_to_endpoint(uint8_t addr, char *endpoint, size_t len);

/*===========================================================================*/
/* Utility Functions                                                          */
/*===========================================================================*/

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

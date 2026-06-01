/**
 * @file cspcl.c
 * @brief CSPCL - CubeSat Space Protocol Convergence Layer Implementation
 *
 * This implementation uses CSP's SFP (Simple Fragmentation Protocol)
 * for automatic fragmentation and reassembly of bundles.
 *
 * @version 2.0
 */

#include "cspcl.h"

#include "cspcl_config.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef FREERTOS
#include <time.h>
#endif

/* CSP library headers */
#include <csp/arch/csp_malloc.h>
#include <csp/csp.h>
/* libcsp headers for direct CSP stack control */
#include <csp/csp_rtable.h>
#include <csp/interfaces/csp_if_zmqhub.h>
/* CAN interface support - requires libcsp built with CAN driver */
#ifdef CSP_HAVE_LIBSOCKETCAN
#include <csp/drivers/can_socketcan.h>
#include <csp/interfaces/csp_if_can.h>
#endif

/*===========================================================================*/
/* CSP Global State                                                          */
/*===========================================================================*/

/* libcsp is a global singleton. Allow multiple CSPCL instances (and unit
 * tests) to call cspcl_init() without re-running csp_init() or starting the
 * router task multiple times. */
#ifndef FREERTOS
static pthread_mutex_t g_cspcl_global_lock = PTHREAD_MUTEX_INITIALIZER;
#endif

static bool g_cspcl_stack_initialized = false;
static bool g_cspcl_router_started = false;
static uint8_t g_cspcl_global_addr = 0;
static enum csp_iface_type g_cspcl_global_iface_type = CSP_IFACE_LOOPBACK;
static csp_iface_t *g_cspcl_global_iface = NULL;

/* libcsp v1.6 tracks bound ports globally and does not provide a safe public
 * "unbind" for server sockets. Keep a process-wide singleton RX socket bound to
 * the BP port so multiple cspcl_init()/cspcl_cleanup() cycles (e.g., unit tests)
 * do not fail with "Port X is already in use". */
static csp_socket_t *g_cspcl_global_rx_socket = NULL;
static uint8_t g_cspcl_global_rx_port = 0;

/*===========================================================================*/
/* Internal Pool Helpers                                                     */
/*===========================================================================*/

static int cspcl_pool_lock(cspcl_conn_pool_t *pool)
{
#ifndef FREERTOS
  return pthread_mutex_lock(&pool->lock);
#else
  return xSemaphoreTake(pool->lock, portMAX_DELAY) == pdTRUE ? 0 : -1;
#endif
}

static int cspcl_pool_unlock(cspcl_conn_pool_t *pool)
{
#ifndef FREERTOS
  return pthread_mutex_unlock(&pool->lock);
#else
  return xSemaphoreGive(pool->lock) == pdTRUE ? 0 : -1;
#endif
}

static csp_conn_t *cspcl_pool_get_or_create_locked(cspcl_conn_pool_t *pool, uint8_t dest_addr,
                                                   uint8_t dest_port)
{
  pool->tick++;
  size_t free_slot = CSPCL_CONN_POOL_SIZE;

  for (size_t i = 0; i < CSPCL_CONN_POOL_SIZE; i++) {
    if (!pool->entries[i].used) {
      if (free_slot == CSPCL_CONN_POOL_SIZE) {
        free_slot = i;
      }
      continue;
    }

    if (pool->entries[i].dest_addr == dest_addr && pool->entries[i].dest_port == dest_port &&
        pool->entries[i].conn != NULL) {
#ifndef FREERTOS
      /* Age-based invalidation (disabled when max_conn_age_ms == 0) */
      if (pool->max_conn_age_ms > 0) {
        uint32_t age_s = (uint32_t) (time(NULL) - pool->entries[i].connected_at);
        if (age_s > pool->max_conn_age_ms / 1000u) {
          csp_close(pool->entries[i].conn);
          pool->entries[i].conn = NULL;
          pool->entries[i].used = false;
          pool->stats.invalidations++;
          if (free_slot == CSPCL_CONN_POOL_SIZE) {
            free_slot = i;
          }
          break; /* Fall through to create a fresh connection */
        }
      }
#endif
      /* Cache hit */
      pool->entries[i].last_used = pool->tick;
      pool->stats.hits++;
      return pool->entries[i].conn;
    }
  }

  csp_conn_t *conn = csp_connect(CSP_PRIO_NORM, dest_addr, dest_port, CSPCL_CSP_TIMEOUT_MS,
                                 CSPCL_CSP_CONN_OPTIONS);
  if (conn == NULL) {
    pool->stats.connect_failures++;
    return NULL;
  }

  /* LRU eviction when pool is full */
  if (free_slot == CSPCL_CONN_POOL_SIZE) {
    size_t lru = 0;
    uint32_t oldest = pool->entries[0].last_used;
    for (size_t i = 1; i < CSPCL_CONN_POOL_SIZE; i++) {
      if (pool->entries[i].last_used < oldest) {
        oldest = pool->entries[i].last_used;
        lru = i;
      }
    }
    free_slot = lru;
    if (pool->entries[free_slot].conn != NULL) {
      csp_close(pool->entries[free_slot].conn);
    }
    pool->stats.evictions++;
    CSPCL_LOG("pool full, evicted LRU entry (addr=%u port=%u)",
              (unsigned) pool->entries[free_slot].dest_addr,
              (unsigned) pool->entries[free_slot].dest_port);
  }

  pool->entries[free_slot].used = true;
  pool->entries[free_slot].dest_addr = dest_addr;
  pool->entries[free_slot].dest_port = dest_port;
  pool->entries[free_slot].conn = conn;
  pool->entries[free_slot].last_used = pool->tick;
#ifndef FREERTOS
  pool->entries[free_slot].connected_at = time(NULL);
#endif
  pool->stats.misses++;
  return conn;
}

static void cspcl_pool_invalidate_locked(cspcl_conn_pool_t *pool, uint8_t dest_addr,
                                         uint8_t dest_port)
{
  for (size_t i = 0; i < CSPCL_CONN_POOL_SIZE; i++) {
    if (!pool->entries[i].used) {
      continue;
    }

    if (pool->entries[i].dest_addr == dest_addr && pool->entries[i].dest_port == dest_port) {
      if (pool->entries[i].conn != NULL) {
        csp_close(pool->entries[i].conn);
      }
      pool->entries[i].conn = NULL;
      pool->entries[i].used = false;
      pool->stats.invalidations++;
      return;
    }
  }
}

static void cspcl_release_conn_pool(cspcl_t *cspcl)
{
  if (cspcl == NULL) {
    return;
  }

  cspcl_conn_pool_cleanup(&cspcl->conn_pool);
}

/*===========================================================================*/
/* Initialization Functions                                                   */
/*===========================================================================*/

cspcl_error_t cspcl_init(cspcl_t *cspcl)
{
  if (cspcl == NULL) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (!cspcl->initialized) {
    cspcl_error_t pool_err = cspcl_conn_pool_init(&cspcl->conn_pool);
    if (pool_err != CSPCL_OK) {
      return pool_err;
    }

#ifndef FREERTOS
    pthread_mutex_lock(&g_cspcl_global_lock);
#endif

    if (!g_cspcl_stack_initialized) {
      /* Configure CSP (global singleton) */
      csp_conf_t csp_conf;
      csp_conf_get_defaults(&csp_conf);
      csp_conf.address = cspcl->local_addr;
      csp_conf.hostname = "ud3tn";
      csp_conf.model = "csp-cla";
      csp_conf.revision = "1.0";
      csp_conf.conn_max = CSPCL_CONN_POOL_SIZE + 4; /* Must exceed pool size */
      csp_conf.conn_queue_length = 100;
      csp_conf.fifo_length = 25;
      csp_conf.port_max_bind = 31;
      csp_conf.rdp_max_window = 20;
      csp_conf.buffers = 100;
      csp_conf.buffer_data_size = 256;

      int ret = csp_init(&csp_conf);
      if (ret != CSP_ERR_NONE) {
#ifndef FREERTOS
        pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
        cspcl_release_conn_pool(cspcl);
        return CSPCL_ERR_CSP_STACK_INIT;
      }

      /* Initialize the selected interface (global) */
      switch (cspcl->iface_type) {
      case CSP_IFACE_ZMQHUB:
        ret = csp_zmqhub_init(cspcl->local_addr, cspcl->zmqhub_addr, 0, &cspcl->active_iface);
        if (ret != CSP_ERR_NONE) {
#ifndef FREERTOS
          pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
          cspcl_release_conn_pool(cspcl);
          return CSPCL_ERR_CSP_ZMQHUB_INIT;
        }
        break;

      case CSP_IFACE_CAN:
#ifdef CSP_HAVE_LIBSOCKETCAN
        ret = csp_can_socketcan_open_and_add_interface(
            cspcl->can_iface, /* CAN device name (can0, vcan0, etc.) */
            cspcl->can_iface, /* CSP interface name */
            0,                /* Bitrate (0 = don't change) */
            true,             /* Promisc mode */
            &cspcl->active_iface);
        if (ret != CSP_ERR_NONE) {
#ifndef FREERTOS
          pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
          cspcl_release_conn_pool(cspcl);
          return CSPCL_ERR_CSP_CAN_INIT;
        }
#else
#ifndef FREERTOS
        pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
        cspcl_release_conn_pool(cspcl);
        return CSPCL_ERR_CSP_CAN_NOT_SUPPORTED;
#endif
        break;

      case CSP_IFACE_LOOPBACK:
        cspcl->active_iface = NULL; /* Will use default loopback */
        break;
      }

      g_cspcl_global_addr = cspcl->local_addr;
      g_cspcl_global_iface_type = cspcl->iface_type;
      g_cspcl_global_iface = cspcl->active_iface;

      /* Set default route via active interface */
      if (cspcl->active_iface != NULL) {
        csp_rtable_set(CSP_DEFAULT_ROUTE, 0, cspcl->active_iface, CSP_NODE_MAC);
      }

      /* Start the CSP router task (global singleton) */
      ret = csp_route_start_task(500, 0);
      if (ret != CSP_ERR_NONE) {
#ifndef FREERTOS
        pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
        cspcl_release_conn_pool(cspcl);
        return CSPCL_ERR_CSP_ROUTER;
      }

      g_cspcl_router_started = true;
      g_cspcl_stack_initialized = true;

    } else {
      /* Stack already initialized: enforce consistent configuration. */
      if (cspcl->local_addr != g_cspcl_global_addr ||
          cspcl->iface_type != g_cspcl_global_iface_type) {
#ifndef FREERTOS
        pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
        cspcl_release_conn_pool(cspcl);
        return CSPCL_ERR_CSPINIT;
      }

      cspcl->active_iface = g_cspcl_global_iface;
    }

#ifndef FREERTOS
    pthread_mutex_unlock(&g_cspcl_global_lock);
#endif

    cspcl->initialized = true;
  }

  /* Open RX socket (bind to BP port once) */
  cspcl_error_t err = cspcl_open_rx_socket(cspcl);
  if (err != CSPCL_OK) {
    cspcl_cleanup(cspcl);
    return err;
  }

  return CSPCL_OK;
}

void cspcl_cleanup(cspcl_t *cspcl)
{
  if (cspcl == NULL) {
    return;
  }

  /* Close server socket */
  cspcl_close_rx_socket(cspcl);
  cspcl_release_conn_pool(cspcl);

  cspcl->initialized = false;
}

cspcl_error_t cspcl_conn_pool_init(cspcl_conn_pool_t *pool)
{
  if (pool == NULL) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  memset(pool, 0, sizeof(*pool));

#ifndef FREERTOS
  if (pthread_mutex_init(&pool->lock, NULL) != 0) {
    return CSPCL_ERR_NO_MEMORY;
  }
#else
  pool->lock = xSemaphoreCreateMutex();
  if (pool->lock == NULL) {
    return CSPCL_ERR_NO_MEMORY;
  }
#endif

  /* Read max connection age from environment variable */
  const char *max_age_env = getenv("CSPCL_MAX_CONN_AGE_MS");
  if (max_age_env != NULL) {
    char *endptr;
    long max_age_val = strtol(max_age_env, &endptr, 10);

    /* Validate that the entire string was consumed and value is in valid range */
    if (*endptr == '\0' && max_age_val >= 0 && max_age_val <= (long) UINT32_MAX) {
      pool->max_conn_age_ms = (uint32_t) max_age_val;
      CSPCL_DEBUG("Connection pool max age set to %u ms from CSPCL_MAX_CONN_AGE_MS",
                  pool->max_conn_age_ms);
    } else {
      CSPCL_WARN("Invalid CSPCL_MAX_CONN_AGE_MS value '%s', ignoring (must be 0-%u)", max_age_env,
                 UINT32_MAX);
    }
  }

  pool->initialized = true;
  return CSPCL_OK;
}

void cspcl_conn_pool_cleanup(cspcl_conn_pool_t *pool)
{
  if (pool == NULL || !pool->initialized) {
    return;
  }

  if (cspcl_pool_lock(pool) != 0) {
    return;
  }

  for (size_t i = 0; i < CSPCL_CONN_POOL_SIZE; i++) {
    if (pool->entries[i].used && pool->entries[i].conn != NULL) {
      csp_close(pool->entries[i].conn);
    }
    pool->entries[i].used = false;
    pool->entries[i].conn = NULL;
  }

  /* Set initialized=false while holding the lock so concurrent senders
   * that observe it are guaranteed all connections are already closed. */
  pool->initialized = false;

  if (cspcl_pool_unlock(pool) != 0) {
    CSPCL_LOG("pool unlock failed during cleanup");
  }

#ifndef FREERTOS
  pthread_mutex_destroy(&pool->lock);
#else
  vSemaphoreDelete(pool->lock);
  pool->lock = NULL;
#endif
}

/*===========================================================================*/
/* Bundle Transmission Functions                                              */
/*===========================================================================*/

cspcl_error_t cspcl_send_bundle(cspcl_t *cspcl, const uint8_t *bundle, size_t len,
                                uint8_t dest_addr, uint8_t dest_port)
{
  if (cspcl == NULL || bundle == NULL || len == 0) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (!cspcl->initialized) {
    return CSPCL_ERR_NOT_INITIALIZED;
  }

  if (len > CSPCL_MAX_BUNDLE_SIZE) {
    return CSPCL_ERR_BUNDLE_TOO_LARGE;
  }

  cspcl_conn_pool_t *pool = &cspcl->conn_pool;
  if (!pool->initialized) {
    return CSPCL_ERR_NOT_INITIALIZED;
  }

  if (cspcl_pool_lock(pool) != 0) {
    return CSPCL_ERR_CONNECTION;
  }

  /* Reuse pooled connection or open a new one on miss */
  csp_conn_t *conn = cspcl_pool_get_or_create_locked(pool, dest_addr, dest_port);
  if (conn == NULL) {
    (void) cspcl_pool_unlock(pool);
    return CSPCL_ERR_CONNECTION;
  }

  /* Use CSP's SFP to send the bundle with automatic fragmentation */
  int ret = csp_sfp_send(conn, bundle, (unsigned int) len, CSPCL_MAX_PAYLOAD, CSPCL_CSP_TIMEOUT_MS);

  if (ret != CSP_ERR_NONE) {
    cspcl_pool_invalidate_locked(pool, dest_addr, dest_port);
  }

  if (cspcl_pool_unlock(pool) != 0) {
    CSPCL_LOG("pool unlock failed after send");
  }

  if (ret != CSP_ERR_NONE) {
    return CSPCL_ERR_CSP_SEND;
  }

  return CSPCL_OK;
}

/**
 * @brief Open and bind a server socket for incoming connections
 *
 * This should be called once during initialization to create
 * a socket bound to the BP port for receiving bundle connections.
 *
 * @param cspcl CSPCL instance
 * @return CSPCL_OK on success, error code otherwise
 */
cspcl_error_t cspcl_open_rx_socket(cspcl_t *cspcl)
{
  if (cspcl == NULL || !cspcl->initialized) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (cspcl->rx_socket != NULL) {
    return CSPCL_OK; /* Already open */
  }

#ifndef FREERTOS
  pthread_mutex_lock(&g_cspcl_global_lock);
#endif

  /* Reuse the process-wide RX socket if it already exists. */
  if (g_cspcl_global_rx_socket != NULL) {
    if (g_cspcl_global_rx_port != cspcl->csp_port) {
#ifndef FREERTOS
      pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
      return CSPCL_ERR_CSPINIT;
    }
    cspcl->rx_socket = g_cspcl_global_rx_socket;
#ifndef FREERTOS
    pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
    return CSPCL_OK;
  }

  /* Create socket for connection-oriented mode */
  csp_socket_t *sock = csp_socket(CSPCL_CSP_SOCKET_OPTIONS);
  if (sock == NULL) {
#ifndef FREERTOS
    pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
    return CSPCL_ERR_NO_MEMORY;
  }

  /* Bind to BP port */
  int bind_result = csp_bind(sock, cspcl->csp_port);
  if (bind_result != CSP_ERR_NONE) {
    csp_close(sock);
#ifndef FREERTOS
    pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
    return CSPCL_ERR_CSP_RECV;
  }

  /* Set socket to listen mode */
  int listen_result = csp_listen(sock, 5);
  if (listen_result != CSP_ERR_NONE) {
    csp_close(sock);
#ifndef FREERTOS
    pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
    return CSPCL_ERR_CSP_RECV;
  }

  g_cspcl_global_rx_socket = sock;
  g_cspcl_global_rx_port = cspcl->csp_port;
  cspcl->rx_socket = sock;

#ifndef FREERTOS
  pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
  return CSPCL_OK;
}

/**
 * @brief Close the server socket
 *
 * @param cspcl CSPCL instance
 */
void cspcl_close_rx_socket(cspcl_t *cspcl)
{
  if (cspcl != NULL) {
    /* Do not csp_close() the server socket: libcsp v1.6 keeps port bindings
     * in a global table and there is no safe public unbind. We keep a
     * process-wide RX socket for the lifetime of the process. */
    cspcl->rx_socket = NULL;
  }
}

cspcl_error_t cspcl_recv_bundle(cspcl_t *cspcl, uint8_t *bundle, size_t *len, uint8_t *src_addr,
                                uint8_t *src_port, uint32_t timeout_ms)
{
  if (cspcl == NULL || bundle == NULL || len == NULL) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (!cspcl->initialized) {
    return CSPCL_ERR_NOT_INITIALIZED;
  }

  size_t max_len = *len;
  *len = 0;

  if (cspcl->rx_socket == NULL) {
    cspcl_error_t err = cspcl_open_rx_socket(cspcl);
    if (err != CSPCL_OK) {
      return err;
    }
  }

  /* Accept incoming connection */
  csp_conn_t *conn = csp_accept((csp_socket_t *) cspcl->rx_socket,
                                timeout_ms > 0 ? timeout_ms : CSPCL_CSP_TIMEOUT_MS);
  if (conn == NULL) {
    return CSPCL_ERR_TIMEOUT;
  }

  /* Get source address from connection */
  uint8_t pkt_src_addr = csp_conn_src(conn);
  uint8_t pkt_src_port = csp_conn_sport(conn);

  /* Use CSP's SFP to receive the bundle with automatic reassembly */
  void *data = NULL;
  int datasize = 0;

  int ret = csp_sfp_recv(conn, &data, &datasize, CSPCL_SFP_TIMEOUT_MS);

  /* Close connection */
  csp_close(conn);

  if (ret != CSP_ERR_NONE) {
    if (data != NULL) {
      csp_free(data);
    }
    if (ret == CSP_ERR_TIMEDOUT) {
      return CSPCL_ERR_TIMEOUT;
    }
    return CSPCL_ERR_SFP;
  }

  if (data == NULL || datasize <= 0) {
    return CSPCL_ERR_CSP_RECV;
  }

  /* Check if bundle fits in output buffer */
  if ((size_t) datasize > max_len) {
    csp_free(data);
    return CSPCL_ERR_NO_MEMORY;
  }

  /* Copy received bundle to output buffer */
  memcpy(bundle, data, (size_t) datasize);
  *len = (size_t) datasize;

  if (src_addr != NULL) {
    *src_addr = pkt_src_addr;
  }
  if (src_port != NULL) {
    *src_port = pkt_src_port;
  }

  /* Free SFP-allocated memory */
  csp_free(data);

  return CSPCL_OK;
}

void cspcl_conn_pool_get_stats(const cspcl_conn_pool_t *pool, cspcl_conn_pool_stats_t *stats)
{
  if (pool == NULL || stats == NULL) {
    return;
  }
  *stats = pool->stats;
}

/*===========================================================================*/
/* Address Translation Functions                                              */
/*===========================================================================*/

uint8_t cspcl_endpoint_to_addr(const char *endpoint_id)
{
  if (endpoint_id == NULL) {
    return 0;
  }

  /* Parse IPN scheme: ipn:X.Y → CSP address X */
  if (strncmp(endpoint_id, "ipn:", 4) == 0) {
    int node = 0;
    if (sscanf(endpoint_id + 4, "%d", &node) == 1) {
      if (node >= 0 && node <= 255) {
        return (uint8_t) node;
      }
    }
  }

  /* Parse DTN scheme variants used by the integration stack. */
  if (strncmp(endpoint_id, "dtn://node", 10) == 0) {
    int node = 0;
    if (sscanf(endpoint_id + 10, "%d", &node) == 1) {
      if (node >= 0 && node <= 255) {
        return (uint8_t) node;
      }
    }
  }

  if (strncmp(endpoint_id, "dtn://", 6) == 0) {
    const char *name = endpoint_id + 6;
    if (name[0] != '\0' && name[1] == '.' && strncmp(name + 2, "dtn/", 4) == 0) {
      char label = name[0];
      if (label >= 'a' && label <= 'z') {
        return (uint8_t) (label - 'a' + 1);
      }
    }
  }

  return 0;
}

cspcl_error_t cspcl_addr_to_endpoint(uint8_t addr, char *endpoint, size_t len)
{
  if (endpoint == NULL || len < 12) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  /* Generate IPN endpoint: CSP address X → ipn:X.0 */
  int written = snprintf(endpoint, len, "ipn:%d.0", addr);
  if (written < 0 || (size_t) written >= len) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  return CSPCL_OK;
}

/*===========================================================================*/
/* Utility Functions                                                          */
/*===========================================================================*/

const char *cspcl_strerror(cspcl_error_t err)
{
  switch (err) {
  case CSPCL_OK:
    return "Success";
  case CSPCL_ERR_INVALID_PARAM:
    return "Invalid parameter";
  case CSPCL_ERR_NO_MEMORY:
    return "Memory allocation failed";
  case CSPCL_ERR_BUNDLE_TOO_LARGE:
    return "Bundle exceeds maximum size";
  case CSPCL_ERR_CSP_SEND:
    return "CSP send failed";
  case CSPCL_ERR_CSP_RECV:
    return "CSP receive failed";
  case CSPCL_ERR_TIMEOUT:
    return "Operation timed out";
  case CSPCL_ERR_SFP:
    return "SFP fragmentation/reassembly failed";
  case CSPCL_ERR_NOT_INITIALIZED:
    return "CSPCL not initialized";
  case CSPCL_ERR_CONNECTION:
    return "CSP connection failed";
  case CSPCL_ERR_CSPINIT:
    return "CSP initialization failed";
  case CSPCL_ERR_CSP_STACK_INIT:
    return "CSP stack initialization failed";
  case CSPCL_ERR_CSP_ZMQHUB_INIT:
    return "CSP ZMQ hub interface initialization failed";
  case CSPCL_ERR_CSP_CAN_INIT:
    return "CSP CAN interface initialization failed";
  case CSPCL_ERR_CSP_CAN_NOT_SUPPORTED:
    return "CSP CAN interface not supported (rebuild libcsp with CAN driver)";
  case CSPCL_ERR_CSP_ROUTER:
    return "CSP router task start failed";
  case CSPCL_ERR_POOL_FULL:
    return "Connection pool full, LRU eviction was performed";
  default:
    return "Unknown error";
  }
}

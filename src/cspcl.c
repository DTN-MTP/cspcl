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

#include <stdbool.h>
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

static size_t cspcl_pool_find_free_or_evict_locked(cspcl_conn_pool_t *pool)
{
  for (size_t i = 0; i < CSPCL_CONN_POOL_SIZE; i++) {
    if (!pool->entries[i].used) {
      return i;
    }
  }

  size_t lru = 0;
  uint32_t oldest = pool->entries[0].last_used;

  for (size_t i = 1; i < CSPCL_CONN_POOL_SIZE; i++) {
    if (pool->entries[i].last_used < oldest) {
      oldest = pool->entries[i].last_used;
      lru = i;
    }
  }

  if (pool->entries[lru].conn != NULL) {
    csp_close(pool->entries[lru].conn);
    pool->entries[lru].conn = NULL;
  }

  pool->entries[lru].used = false;
  pool->stats.evictions++;

  return lru;
}

cspcl_error_t cspcl_conn_pool_add(cspcl_conn_pool_t *pool, uint8_t dest_addr, uint8_t dest_port,
                                  csp_conn_t *conn)
{
  if (pool == NULL || conn == NULL) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (!pool->initialized) {
    return CSPCL_ERR_NOT_INITIALIZED;
  }

  if (cspcl_pool_lock(pool) != 0) {
    return CSPCL_ERR_CONNECTION;
  }

  pool->tick++;

  size_t slot = cspcl_pool_find_free_or_evict_locked(pool);

  pool->entries[slot].used = true;
  pool->entries[slot].dest_addr = dest_addr;
  pool->entries[slot].dest_port = dest_port;
  pool->entries[slot].conn = conn;
  pool->entries[slot].last_used = pool->tick;

#ifndef FREERTOS
  pool->entries[slot].connected_at = time(NULL);
#endif

  (void) cspcl_pool_unlock(pool);
  return CSPCL_OK;
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

  /* LRU eviction when pool is full — close the LRU entry BEFORE calling
   * csp_connect() so that CSP has a free connection slot available. */
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
    CSPCL_LOG("pool full, evicted LRU entry (addr=%u port=%u)",
              (unsigned) pool->entries[free_slot].dest_addr,
              (unsigned) pool->entries[free_slot].dest_port);
    if (pool->entries[free_slot].conn != NULL) {
      csp_close(pool->entries[free_slot].conn);
      pool->entries[free_slot].conn = NULL;
    }
    pool->entries[free_slot].used = false;
    pool->stats.evictions++;
  }

  csp_conn_t *conn = csp_connect(CSP_PRIO_NORM, dest_addr, dest_port, CSPCL_CSP_TIMEOUT_MS,
                                 CSPCL_CSP_CONN_OPTIONS);
  if (conn == NULL) {
    pool->stats.connect_failures++;
    return NULL;
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

    memset(cspcl->rx_conns, 0, sizeof(cspcl->rx_conns));
    cspcl->rx_tick = 0;

#ifndef FREERTOS
    pthread_mutex_lock(&g_cspcl_global_lock);
#endif

    if (!g_cspcl_stack_initialized) {
      /* Configure CSP (global singleton) */
      csp_conf_t csp_conf;
      csp_conf_get_defaults(&csp_conf);
      csp_conf.address = cspcl->local_addr;
      csp_conf.hostname = "ud3tn";
      csp_conf.model = CSPCL_CSP_MODEL;
      csp_conf.revision = CSPCL_CSP_REVISION;
      /* Must cover outbound pool + held inbound connections */
      csp_conf.conn_max = CSPCL_CONN_POOL_SIZE + CSPCL_RX_CONN_TABLE_SIZE + 4;
      csp_conf.conn_queue_length = CSPCL_CSP_CONN_QUEUE_LENGTH;
      csp_conf.fifo_length = CSPCL_CSP_FIFO_LENGTH;
      csp_conf.port_max_bind = CSPCL_CSP_PORT_MAX_BIND;
      csp_conf.rdp_max_window = CSPCL_CSP_RDP_MAX_WINDOW;
      csp_conf.buffers = CSPCL_CSP_BUFFERS;
      csp_conf.buffer_data_size = CSPCL_CSP_BUFFER_DATA_SIZE;

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

  /* Close live inbound connections */
  for (size_t i = 0; i < CSPCL_RX_CONN_TABLE_SIZE; i++) {
    if (cspcl->rx_conns[i].used && cspcl->rx_conns[i].conn != NULL) {
      csp_close(cspcl->rx_conns[i].conn);
    }
    cspcl->rx_conns[i].conn = NULL;
    cspcl->rx_conns[i].used = false;
  }

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
/* Application-Level Delivery Acknowledgement                                 */
/*===========================================================================*/

/* csp_sfp_send()/csp_send() only report that data was queued locally (and,
 * on an RDP connection, accepted into the transmit window) - not that the
 * peer received it. cspcl confirms real delivery itself: the receiver sends
 * a small ack packet back on the same connection once it has successfully
 * reassembled a bundle, and the sender waits for it. This needs nothing
 * beyond stock CSP connection-oriented sockets (csp_send/csp_read), so it
 * works against an unmodified libcsp build. */

static int cspcl_send_ack(csp_conn_t *conn)
{
  csp_packet_t *packet = csp_buffer_get(1);
  if (packet == NULL) {
    return CSP_ERR_NOMEM;
  }

  packet->data[0] = CSPCL_ACK_MAGIC;
  packet->length = 1;

  /* csp_send() returns 1 on success, 0 on failure (unlike most other CSP
   * calls, which return a CSP_ERR_* code). */
  if (!csp_send(conn, packet, CSPCL_CSP_TIMEOUT_MS)) {
    csp_buffer_free(packet);
    return CSP_ERR_TIMEDOUT;
  }

  return CSP_ERR_NONE;
}

static int cspcl_wait_ack(csp_conn_t *conn, uint32_t timeout_ms)
{
  csp_packet_t *packet = csp_read(conn, timeout_ms);
  if (packet == NULL) {
    return CSP_ERR_TIMEDOUT;
  }

  csp_buffer_free(packet);
  return CSP_ERR_NONE;
}

static int cspcl_send_and_confirm(csp_conn_t *conn, const uint8_t *bundle, size_t len)
{
  int ret = csp_sfp_send(conn, bundle, (unsigned int) len, CSPCL_MAX_PAYLOAD, CSPCL_CSP_TIMEOUT_MS);
  if (ret != CSP_ERR_NONE) {
    return ret;
  }

  return cspcl_wait_ack(conn, CSPCL_ACK_TIMEOUT_MS);
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

  /* Use CSP's SFP to send the bundle, then wait for the receiver's
   * application-level ack to confirm it actually arrived. */
  int ret = cspcl_send_and_confirm(conn, bundle, len);

  if (ret != CSP_ERR_NONE) {
    /* The pooled connection may be stale (e.g. peer restarted). Reconnect
     * and retry once. */
    cspcl_pool_invalidate_locked(pool, dest_addr, dest_port);
    conn = cspcl_pool_get_or_create_locked(pool, dest_addr, dest_port);
    if (conn != NULL) {
      ret = cspcl_send_and_confirm(conn, bundle, len);
      if (ret != CSP_ERR_NONE) {
        cspcl_pool_invalidate_locked(pool, dest_addr, dest_port);
      }
    }
  }

  if (cspcl_pool_unlock(pool) != 0) {
    CSPCL_LOG("pool unlock failed after send");
  }

  if (ret == CSP_ERR_TIMEDOUT) {
    return CSPCL_ERR_TIMEOUT;
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

  /* Reuse the process-wide RX socket if it exists AND matches our port */
  if (g_cspcl_global_rx_socket != NULL && g_cspcl_global_rx_port == cspcl->csp_port) {
    cspcl->rx_socket = g_cspcl_global_rx_socket;
#ifndef FREERTOS
    pthread_mutex_unlock(&g_cspcl_global_lock);
#endif
    return CSPCL_OK;
  }

  /* Unlock global lock - we're creating instance-specific socket */
#ifndef FREERTOS
  pthread_mutex_unlock(&g_cspcl_global_lock);
#endif

  /* Create socket for connection-oriented mode */
  csp_socket_t *sock = csp_socket(CSPCL_CSP_SOCKET_OPTIONS);
  if (sock == NULL) {
    return CSPCL_ERR_NO_MEMORY;
  }

  /* Bind to receiving port */
  int bind_result = csp_bind(sock, cspcl->csp_port);
  if (bind_result != CSP_ERR_NONE) {
    csp_close(sock);
    return CSPCL_ERR_CSP_RECV;
  }

  /* Set socket to listen mode */
  int listen_result = csp_listen(sock, 5);
  if (listen_result != CSP_ERR_NONE) {
    csp_close(sock);
    return CSPCL_ERR_CSP_RECV;
  }

  cspcl->rx_socket = sock;

#ifndef FREERTOS
  pthread_mutex_lock(&g_cspcl_global_lock);
#endif
  g_cspcl_global_rx_socket = sock;
  g_cspcl_global_rx_port = cspcl->csp_port;
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

cspcl_error_t cspcl_accept_conn(cspcl_t *cspcl, csp_conn_t **conn, uint8_t *src_addr,
                                uint8_t *src_port, uint32_t timeout_ms)
{
  if (cspcl == NULL || conn == NULL || src_addr == NULL || src_port == NULL) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (!cspcl->initialized) {
    return CSPCL_ERR_NOT_INITIALIZED;
  }

  if (cspcl->rx_socket == NULL) {
    cspcl_error_t err = cspcl_open_rx_socket(cspcl);
    if (err != CSPCL_OK) {
      return err;
    }
  }

  csp_conn_t *accepted_conn = csp_accept((csp_socket_t *) cspcl->rx_socket,
                                         timeout_ms > 0 ? timeout_ms : CSPCL_CSP_TIMEOUT_MS);
  if (accepted_conn == NULL) {
    return CSPCL_ERR_TIMEOUT;
  }

  *conn = accepted_conn;
  *src_addr = csp_conn_src(accepted_conn);
  *src_port = csp_conn_sport(accepted_conn);

  return CSPCL_OK;
}

static cspcl_error_t cspcl_recv_from_conn_timeout(csp_conn_t *conn, uint8_t *bundle, size_t *len,
                                                  uint8_t *src_addr, uint8_t *src_port,
                                                  uint8_t pkt_src_addr, uint8_t pkt_src_port,
                                                  uint32_t timeout_ms)
{
  if (conn == NULL || bundle == NULL || len == NULL) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  size_t max_len = *len;
  *len = 0;

  /* Use CSP's SFP to receive the bundle with automatic reassembly */
  void *data = NULL;
  int datasize = 0;

  int ret = csp_sfp_recv(conn, &data, &datasize, timeout_ms);

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

  /* Confirm delivery to the sender. Best-effort: if this is lost, the
   * sender's wait for it will time out and retry, which is an accepted
   * trade-off rather than a reason to discard an already-received bundle. */
  if (cspcl_send_ack(conn) != CSP_ERR_NONE) {
    CSPCL_LOG("failed to send bundle-received ack");
  }

  return CSPCL_OK;
}

cspcl_error_t cspcl_recv_bundle_from_conn(csp_conn_t *conn, uint8_t *bundle, size_t *len,
                                          uint8_t *src_addr, uint8_t *src_port,
                                          uint8_t pkt_src_addr, uint8_t pkt_src_port)
{
  return cspcl_recv_from_conn_timeout(conn, bundle, len, src_addr, src_port, pkt_src_addr,
                                      pkt_src_port, CSPCL_SFP_TIMEOUT_MS);
}

/**
 * @brief Store a newly accepted inbound connection in the RX table
 *
 * A peer that reconnects (same address and port) replaces its previous
 * entry; the stale connection is closed. When the table is full the
 * least-recently-active entry is evicted.
 */
static void cspcl_rx_table_store(cspcl_t *cspcl, csp_conn_t *conn, uint8_t src_addr,
                                 uint8_t src_port)
{
  size_t slot = CSPCL_RX_CONN_TABLE_SIZE;

  for (size_t i = 0; i < CSPCL_RX_CONN_TABLE_SIZE; i++) {
    cspcl_rx_conn_entry_t *e = &cspcl->rx_conns[i];
    if (e->used && e->src_addr == src_addr && e->src_port == src_port) {
      /* Peer reconnected - its old connection is dead on their side */
      if (e->conn != NULL && e->conn != conn) {
        csp_close(e->conn);
      }
      slot = i;
      break;
    }
    if (!e->used && slot == CSPCL_RX_CONN_TABLE_SIZE) {
      slot = i;
    }
  }

  if (slot == CSPCL_RX_CONN_TABLE_SIZE) {
    /* Table full - evict least-recently-active entry */
    slot = 0;
    for (size_t i = 1; i < CSPCL_RX_CONN_TABLE_SIZE; i++) {
      if (cspcl->rx_conns[i].last_active < cspcl->rx_conns[slot].last_active) {
        slot = i;
      }
    }
    if (cspcl->rx_conns[slot].conn != NULL) {
      csp_close(cspcl->rx_conns[slot].conn);
    }
  }

  cspcl->rx_conns[slot].used = true;
  cspcl->rx_conns[slot].conn = conn;
  cspcl->rx_conns[slot].src_addr = src_addr;
  cspcl->rx_conns[slot].src_port = src_port;
  cspcl->rx_conns[slot].last_active = ++cspcl->rx_tick;
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

  if (timeout_ms == 0) {
    timeout_ms = CSPCL_CSP_TIMEOUT_MS;
  }

  size_t max_len = *len;

  size_t live = 0;
  for (size_t i = 0; i < CSPCL_RX_CONN_TABLE_SIZE; i++) {
    if (cspcl->rx_conns[i].used) {
      live++;
    }
  }

  /* Poll for new inbound connections. With live connections to service we
   * only poll briefly; otherwise accept() gets the whole timeout. Senders
   * pool and reuse their outbound connection, so most bundles arrive on an
   * already-accepted connection rather than a new one. */
  uint32_t accept_timeout = timeout_ms;
  if (live > 0) {
    accept_timeout =
        timeout_ms / 2 < CSPCL_RX_ACCEPT_POLL_MS ? timeout_ms / 2 : CSPCL_RX_ACCEPT_POLL_MS;
  }

  csp_conn_t *new_conn = NULL;
  uint8_t new_src_addr = 0;
  uint8_t new_src_port = 0;
  cspcl_error_t accept_err =
      cspcl_accept_conn(cspcl, &new_conn, &new_src_addr, &new_src_port, accept_timeout);
  if (accept_err == CSPCL_OK) {
    cspcl_rx_table_store(cspcl, new_conn, new_src_addr, new_src_port);
    live = 0;
    for (size_t i = 0; i < CSPCL_RX_CONN_TABLE_SIZE; i++) {
      if (cspcl->rx_conns[i].used) {
        live++;
      }
    }
  } else if (accept_err != CSPCL_ERR_TIMEOUT) {
    *len = 0;
    return accept_err;
  }

  if (live == 0) {
    *len = 0;
    return CSPCL_ERR_TIMEOUT;
  }

  /* Poll live connections for a bundle, splitting the remaining time
   * between them. Round-robin start index keeps one busy peer from
   * starving the others. */
  uint32_t read_timeout = (timeout_ms - accept_timeout) / (uint32_t) live;
  if (read_timeout < 50) {
    read_timeout = 50;
  }

  size_t start = cspcl->rx_tick % CSPCL_RX_CONN_TABLE_SIZE;
  for (size_t n = 0; n < CSPCL_RX_CONN_TABLE_SIZE; n++) {
    size_t i = (start + n) % CSPCL_RX_CONN_TABLE_SIZE;
    cspcl_rx_conn_entry_t *e = &cspcl->rx_conns[i];
    if (!e->used) {
      continue;
    }

    *len = max_len;
    cspcl_error_t recv_err = cspcl_recv_from_conn_timeout(
        e->conn, bundle, len, src_addr, src_port, e->src_addr, e->src_port, read_timeout);

    if (recv_err == CSPCL_OK) {
      e->last_active = ++cspcl->rx_tick;
      return CSPCL_OK;
    }

    if (recv_err != CSPCL_ERR_TIMEOUT) {
      /* Connection is broken - drop it; the peer will reconnect */
      csp_close(e->conn);
      e->conn = NULL;
      e->used = false;
    }
  }

  *len = 0;
  return CSPCL_ERR_TIMEOUT;
}

/**
 * @brief Debug: Dump connection pool status
 */
static void cspcl_debug_pool_status(cspcl_conn_pool_t *pool)
{
  if (pool == NULL || !pool->initialized) {
    return;
  }

  for (size_t i = 0; i < CSPCL_CONN_POOL_SIZE; i++) {
    if (pool->entries[i].used && pool->entries[i].conn != NULL) {
      /* Pool entry exists: addr=%u, port=%u */
    }
  }
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
/* PHASE 2: Unified Address Parsing Implementation                           */
/*===========================================================================*/

uint8_t cspcl_parse_address(const char *addr_string, uint8_t *dest_port)
{
  if (!addr_string) {
    return 0;
  }

  /* Try CSP scheme first: "csp:X" or "csp:X,Y" */
  if (strncmp(addr_string, "csp:", 4) == 0) {
    const char *addr_str = addr_string + 4;
    int addr = atoi(addr_str);
    if (addr >= 0 && addr <= 255) {
      if (dest_port) {
        uint8_t port = cspcl_parse_port(addr_string);
        *dest_port = port;
      }
      return (uint8_t) addr;
    }
  }

  /* Try IPN scheme: "ipn:X.Y" */
  if (strncmp(addr_string, "ipn:", 4) == 0) {
    int node = 0;
    if (sscanf(addr_string + 4, "%d", &node) == 1 && node >= 0 && node <= 255) {
      if (dest_port)
        *dest_port = CSPCL_PORT_BP;
      return (uint8_t) node;
    }
  }

  /* Try DTN schemes */
  if (strncmp(addr_string, "dtn://node", 10) == 0) {
    int node = 0;
    if (sscanf(addr_string + 10, "%d", &node) == 1 && node >= 0 && node <= 255) {
      if (dest_port)
        *dest_port = CSPCL_PORT_BP;
      return (uint8_t) node;
    }
  }

  if (strncmp(addr_string, "dtn://", 6) == 0) {
    const char *name = addr_string + 6;
    if (name[0] != '\0' && name[1] == '.' && strncmp(name + 2, "dtn/", 4) == 0) {
      char label = name[0];
      if (label >= 'a' && label <= 'z') {
        if (dest_port)
          *dest_port = CSPCL_PORT_BP;
        return (uint8_t) (label - 'a' + 1);
      }
    }
  }

  /* Try bare integer: "42" */
  int addr = atoi(addr_string);
  if (addr >= 0 && addr <= 255) {
    if (dest_port)
      *dest_port = CSPCL_PORT_BP;
    return (uint8_t) addr;
  }

  return 0;
}

bool cspcl_is_valid_address_string(const char *addr_string, uint8_t parsed_addr)
{
  if (parsed_addr != 0 || !addr_string) {
    return parsed_addr != 0;
  }

  /* Check if string is literally "0" or "csp:0" or "ipn:0.X" etc */
  if (strcmp(addr_string, "0") == 0)
    return true;
  if (strcmp(addr_string, "csp:0") == 0)
    return true;
  if (strncmp(addr_string, "ipn:0.", 6) == 0)
    return true;
  if (strcmp(addr_string, "dtn://node0") == 0)
    return true;

  return false;
}

uint8_t cspcl_parse_port(const char *addr_string)
{
  if (!addr_string) {
    return CSPCL_PORT_BP;
  }

  if (strncmp(addr_string, "csp:", 4) == 0) {
    const char *comma = strchr(addr_string + 4, ',');
    if (comma) {
      int port = atoi(comma + 1);
      if (port >= 0 && port <= 31) {
        return (uint8_t) port;
      }
    }
  }

  return CSPCL_PORT_BP;
}

cspcl_error_t cspcl_identify_address_scheme(const char *addr_string, char *scheme_out,
                                            size_t scheme_len)
{
  if (!addr_string || !scheme_out || scheme_len < 5) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (strncmp(addr_string, "ipn:", 4) == 0) {
    strncpy(scheme_out, "ipn", scheme_len - 1);
    scheme_out[scheme_len - 1] = '\0';
    return CSPCL_OK;
  }

  if (strncmp(addr_string, "dtn://", 6) == 0) {
    strncpy(scheme_out, "dtn", scheme_len - 1);
    scheme_out[scheme_len - 1] = '\0';
    return CSPCL_OK;
  }

  if (strncmp(addr_string, "csp:", 4) == 0) {
    strncpy(scheme_out, "csp", scheme_len - 1);
    scheme_out[scheme_len - 1] = '\0';
    return CSPCL_OK;
  }

  /* Check if it's a bare integer */
  if (addr_string[0] >= '0' && addr_string[0] <= '9') {
    strncpy(scheme_out, "bare", scheme_len - 1);
    scheme_out[scheme_len - 1] = '\0';
    return CSPCL_OK;
  }

  return CSPCL_ERR_INVALID_PARAM;
}

/*===========================================================================*/
/* PHASE 3: Error Categorization Implementation                              */
/*===========================================================================*/

cspcl_error_category_t cspcl_categorize_error(cspcl_error_t err)
{
  switch (err) {
  case CSPCL_OK:
    return CSPCL_ERRCATEGORY_OK;

  case CSPCL_ERR_INVALID_PARAM:
    return CSPCL_ERRCATEGORY_PARAM;

  case CSPCL_ERR_NO_MEMORY:
  case CSPCL_ERR_POOL_FULL:
    return CSPCL_ERRCATEGORY_RESOURCE;

  case CSPCL_ERR_TIMEOUT:
    return CSPCL_ERRCATEGORY_TIMEOUT;

  case CSPCL_ERR_CSP_SEND:
  case CSPCL_ERR_CSP_RECV:
  case CSPCL_ERR_CONNECTION:
  case CSPCL_ERR_SFP:
    return CSPCL_ERRCATEGORY_CSP;

  case CSPCL_ERR_BUNDLE_TOO_LARGE:
  case CSPCL_ERR_NOT_INITIALIZED:
  case CSPCL_ERR_CSPINIT:
  case CSPCL_ERR_CSP_STACK_INIT:
  case CSPCL_ERR_CSP_ZMQHUB_INIT:
  case CSPCL_ERR_CSP_CAN_INIT:
  case CSPCL_ERR_CSP_CAN_NOT_SUPPORTED:
  case CSPCL_ERR_CSP_ROUTER:
  default:
    return CSPCL_ERRCATEGORY_FATAL;
  }
}

bool cspcl_error_is_retryable(cspcl_error_t err)
{
  return err == CSPCL_ERR_TIMEOUT || err == CSPCL_ERR_POOL_FULL || err == CSPCL_ERR_CSP_SEND ||
         err == CSPCL_ERR_CSP_RECV;
}

/*===========================================================================*/
/* PHASE 4: Interface Parsing Implementation                                 */
/*===========================================================================*/

cspcl_error_t cspcl_parse_interface_spec(const char *iface_spec, cspcl_t *cspcl)
{
  if (!iface_spec || !cspcl) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (strncmp(iface_spec, "zmqhub:", 7) == 0) {
    cspcl->iface_type = CSP_IFACE_ZMQHUB;
    strncpy(cspcl->zmqhub_addr, iface_spec + 7, CSPCL_IFACE_PARAM_MAX - 1);
    cspcl->zmqhub_addr[CSPCL_IFACE_PARAM_MAX - 1] = '\0';
    return CSPCL_OK;
  }

  if (strcmp(iface_spec, "zmqhub") == 0) {
    cspcl->iface_type = CSP_IFACE_ZMQHUB;
    strncpy(cspcl->zmqhub_addr, CSPCL_ZMQHUB_ADDR_DEFAULT, CSPCL_IFACE_PARAM_MAX - 1);
    cspcl->zmqhub_addr[CSPCL_IFACE_PARAM_MAX - 1] = '\0';
    return CSPCL_OK;
  }

  if (strncmp(iface_spec, "can:", 4) == 0) {
    cspcl->iface_type = CSP_IFACE_CAN;
    strncpy(cspcl->can_iface, iface_spec + 4, CSPCL_IFACE_PARAM_MAX - 1);
    cspcl->can_iface[CSPCL_IFACE_PARAM_MAX - 1] = '\0';
    return CSPCL_OK;
  }

  if (strcmp(iface_spec, "can") == 0) {
    cspcl->iface_type = CSP_IFACE_CAN;
    strncpy(cspcl->can_iface, CSPCL_CAN_IFACE_DEFAULT, CSPCL_IFACE_PARAM_MAX - 1);
    cspcl->can_iface[CSPCL_IFACE_PARAM_MAX - 1] = '\0';
    return CSPCL_OK;
  }

  if (strcmp(iface_spec, "loopback") == 0) {
    cspcl->iface_type = CSP_IFACE_LOOPBACK;
    return CSPCL_OK;
  }

  return CSPCL_ERR_INVALID_PARAM;
}

cspcl_error_t cspcl_interface_type_to_string(const cspcl_t *cspcl, char *buf, size_t len)
{
  if (!cspcl || !buf || len < 10) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  switch (cspcl->iface_type) {
  case CSP_IFACE_ZMQHUB:
    snprintf(buf, len, "zmqhub:%s", cspcl->zmqhub_addr);
    return CSPCL_OK;
  case CSP_IFACE_CAN:
    snprintf(buf, len, "can:%s", cspcl->can_iface);
    return CSPCL_OK;
  case CSP_IFACE_LOOPBACK:
    snprintf(buf, len, "loopback");
    return CSPCL_OK;
  default:
    snprintf(buf, len, "unknown");
    return CSPCL_ERR_INVALID_PARAM;
  }
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

/**
 * @file csp_stub.c
 * @brief Minimal CSP stub implementation for testing
 *
 * This provides minimal stub implementations for CSP functions
 * to allow testing of CSPCL without a full libcsp installation.
 */

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "csp/csp.h"
#include "csp/csp_rtable.h"
#include "csp/interfaces/csp_if_zmqhub.h"

/*===========================================================================*/
/* Internal Structures                                                        */
/*===========================================================================*/

struct csp_socket_s {
  uint8_t port;
  uint32_t opts;
};

struct csp_conn_s {
  uint8_t src;
  uint8_t dst;
  uint8_t sport;
  uint8_t dport;
};

/* Simple loopback queue for testing */
#define LOOPBACK_QUEUE_SIZE 16

static struct {
  csp_packet_t *packets[LOOPBACK_QUEUE_SIZE];
  uint8_t src_addrs[LOOPBACK_QUEUE_SIZE];
  int head;
  int tail;
  int count;
} loopback_queue = {0};

/*===========================================================================*/
/* Test helpers                                                               */
/*===========================================================================*/

/**
 * Set to non-zero to make csp_sfp_send_own_memcpy return CSP_ERR_TIMEDOUT.
 * Reset to 0 to restore normal behaviour. Used for failure-injection tests.
 */
int g_csp_sfp_send_fail = 0;

/*===========================================================================*/
/* Core stack API                                                            */
/*===========================================================================*/

void csp_conf_get_defaults(csp_conf_t *conf)
{
  if (conf == NULL) {
    return;
  }
  memset(conf, 0, sizeof(*conf));
}

int csp_init(const csp_conf_t *conf)
{
  (void) conf;
  return CSP_ERR_NONE;
}

int csp_route_start_task(unsigned int stack_size, unsigned int prio)
{
  (void) stack_size;
  (void) prio;
  return CSP_ERR_NONE;
}

void csp_rdp_set_opt(unsigned int window_size, unsigned int conn_timeout_ms,
                     unsigned int packet_timeout_ms, int delayed_acks, unsigned int ack_timeout,
                     int ack_delay_count)
{
  (void) window_size;
  (void) conn_timeout_ms;
  (void) packet_timeout_ms;
  (void) delayed_acks;
  (void) ack_timeout;
  (void) ack_delay_count;
}

int csp_rtable_set(uint8_t addr, uint8_t netmask, csp_iface_t *ifc, uint16_t via)
{
  (void) addr;
  (void) netmask;
  (void) ifc;
  (void) via;
  return CSP_ERR_NONE;
}

int csp_zmqhub_init(uint8_t addr, const char *server, uint16_t rx_filter, csp_iface_t **iface)
{
  static csp_iface_t iface_static;

  (void) server;
  (void) rx_filter;

  iface_static.addr = addr;
  if (iface != NULL) {
    *iface = &iface_static;
  }

  return CSP_ERR_NONE;
}

/*===========================================================================*/
/* Endian / CRC helpers                                                       */
/*===========================================================================*/

/* Declared by the real libcsp headers (csp/csp_endian.h, csp/csp_crc32.h),
 * which are picked up transitively since this stub tree does not shadow
 * them. Only definitions are needed here. */

uint32_t csp_hton32(uint32_t h32)
{
  return ((h32 & 0x000000FFu) << 24) | ((h32 & 0x0000FF00u) << 8) |
         ((h32 & 0x00FF0000u) >> 8) | ((h32 & 0xFF000000u) >> 24);
}

uint32_t csp_ntoh32(uint32_t n32)
{
  return csp_hton32(n32);
}

uint32_t csp_crc32_memory(const uint8_t *addr, uint32_t length)
{
  uint32_t crc = 0xFFFFFFFFu;

  if (addr == NULL) {
    return 0;
  }

  for (uint32_t i = 0; i < length; i++) {
    crc ^= addr[i];
    for (int bit = 0; bit < 8; bit++) {
      uint32_t mask = (uint32_t) (-(int32_t) (crc & 1u));
      crc = (crc >> 1) ^ (0xEDB88320u & mask);
    }
  }

  return crc ^ 0xFFFFFFFFu;
}

/*===========================================================================*/
/* Buffer API                                                                 */
/*===========================================================================*/

csp_packet_t *csp_buffer_get(size_t size)
{
  (void) size;
  csp_packet_t *packet = (csp_packet_t *) calloc(1, sizeof(csp_packet_t));
  return packet;
}

void csp_buffer_free(csp_packet_t *packet)
{
  if (packet != NULL) {
    free(packet);
  }
}

/*===========================================================================*/
/* Socket API                                                                 */
/*===========================================================================*/

csp_socket_t *csp_socket(uint32_t opts)
{
  csp_socket_t *sock = (csp_socket_t *) calloc(1, sizeof(csp_socket_t));
  if (sock != NULL) {
    sock->opts = opts;
  }
  return sock;
}

int csp_bind(csp_socket_t *socket, uint8_t port)
{
  if (socket == NULL) {
    return CSP_ERR_INVAL;
  }
  socket->port = port;
  return CSP_ERR_NONE;
}

int csp_listen(csp_socket_t *socket, size_t backlog)
{
  (void) socket;
  (void) backlog;
  return CSP_ERR_NONE;
}

csp_conn_t *csp_accept(csp_socket_t *socket, uint32_t timeout)
{
  (void) socket;
  (void) timeout;

  /* Return connection if there's a packet in the loopback queue */
  if (loopback_queue.count > 0) {
    csp_conn_t *conn = (csp_conn_t *) calloc(1, sizeof(csp_conn_t));
    if (conn != NULL) {
      conn->src = loopback_queue.src_addrs[loopback_queue.tail];
    }
    return conn;
  }

  return NULL;
}

int csp_close(void *conn)
{
  if (conn != NULL) {
    free(conn);
  }
  return CSP_ERR_NONE;
}

/*===========================================================================*/
/* Connection API                                                             */
/*===========================================================================*/

csp_conn_t *csp_connect(uint8_t prio, uint8_t dest, uint8_t dport, uint32_t timeout, uint32_t opts)
{
  (void) prio;
  (void) timeout;
  (void) opts;

  csp_conn_t *conn = (csp_conn_t *) calloc(1, sizeof(csp_conn_t));
  if (conn != NULL) {
    conn->dst = dest;
    conn->dport = dport;
  }
  return conn;
}

int csp_send(csp_conn_t *conn, csp_packet_t *packet, uint32_t timeout)
{
  (void) conn;
  (void) packet;
  (void) timeout;
  return CSP_ERR_NONE;
}

csp_packet_t *csp_read(csp_conn_t *conn, uint32_t timeout)
{
  (void) conn;
  (void) timeout;

  /* Return packet from loopback queue */
  if (loopback_queue.count > 0) {
    csp_packet_t *packet = loopback_queue.packets[loopback_queue.tail];
    loopback_queue.tail = (loopback_queue.tail + 1) % LOOPBACK_QUEUE_SIZE;
    loopback_queue.count--;
    return packet;
  }

  return NULL;
}

/*===========================================================================*/
/* Connectionless API                                                         */
/*===========================================================================*/

int csp_sendto(uint8_t prio, uint8_t dest, uint8_t dport, uint8_t sport, uint32_t opts,
               csp_packet_t *packet, uint32_t timeout)
{
  (void) prio;
  (void) dport;
  (void) sport;
  (void) opts;
  (void) timeout;

  /* Add to loopback queue for testing */
  if (loopback_queue.count < LOOPBACK_QUEUE_SIZE && packet != NULL) {
    /* Copy packet for loopback */
    csp_packet_t *copy = csp_buffer_get(packet->length);
    if (copy != NULL) {
      memcpy(copy, packet, sizeof(csp_packet_t));
      loopback_queue.packets[loopback_queue.head] = copy;
      loopback_queue.src_addrs[loopback_queue.head] = dest; /* Loopback */
      loopback_queue.head = (loopback_queue.head + 1) % LOOPBACK_QUEUE_SIZE;
      loopback_queue.count++;
    }
  }

  /* Free original packet (CSP takes ownership) */
  csp_buffer_free(packet);

  return CSP_ERR_NONE;
}

csp_packet_t *csp_recvfrom(csp_socket_t *socket, uint32_t timeout)
{
  (void) socket;
  (void) timeout;

  if (loopback_queue.count > 0) {
    csp_packet_t *packet = loopback_queue.packets[loopback_queue.tail];
    loopback_queue.tail = (loopback_queue.tail + 1) % LOOPBACK_QUEUE_SIZE;
    loopback_queue.count--;
    return packet;
  }

  return NULL;
}

/*===========================================================================*/
/* Connection Info                                                            */
/*===========================================================================*/

uint8_t csp_conn_src(csp_conn_t *conn)
{
  return conn != NULL ? conn->src : 0;
}

uint8_t csp_conn_dst(csp_conn_t *conn)
{
  return conn != NULL ? conn->dst : 0;
}

uint8_t csp_conn_sport(csp_conn_t *conn)
{
  return conn != NULL ? conn->sport : 0;
}

uint8_t csp_conn_dport(csp_conn_t *conn)
{
  return conn != NULL ? conn->dport : 0;
}

/*===========================================================================*/
/* SFP (Simple Fragmentation Protocol) API                                    */
/*===========================================================================*/

/* SFP loopback buffer for testing */
static struct {
  void *data;
  unsigned int size;
  uint8_t src_addr;
  int pending;
} sfp_loopback = {0};

int csp_sfp_send_own_memcpy(csp_conn_t *conn, const void *data, unsigned int datasize,
                            unsigned int mtu, uint32_t timeout, csp_memcpy_fnc_t memcpyfcn)
{
  (void) mtu;
  (void) timeout;

  if (conn == NULL || data == NULL || datasize == 0 || memcpyfcn == NULL) {
    return CSP_ERR_INVAL;
  }

  if (g_csp_sfp_send_fail) {
    return CSP_ERR_TIMEDOUT;
  }

  /* For testing: store data in loopback buffer */
  if (sfp_loopback.data != NULL) {
    free(sfp_loopback.data);
  }

  sfp_loopback.data = malloc(datasize);
  if (sfp_loopback.data == NULL) {
    return CSP_ERR_NOMEM;
  }

  memcpyfcn((csp_memptr_t) sfp_loopback.data, (csp_memptr_t) (uintptr_t) data, datasize);
  sfp_loopback.size = datasize;
  sfp_loopback.src_addr = conn->dst; /* For loopback testing */
  sfp_loopback.pending = 1;

  return CSP_ERR_NONE;
}

int csp_sfp_recv_fp(csp_conn_t *conn, void **dataout, int *datasize, uint32_t timeout,
                    csp_packet_t *first_packet)
{
  (void) conn;
  (void) timeout;
  (void) first_packet;

  *dataout = NULL;
  *datasize = 0;

  if (!sfp_loopback.pending || sfp_loopback.data == NULL) {
    return CSP_ERR_TIMEDOUT;
  }

  /* Return the loopback data */
  *dataout = sfp_loopback.data;
  *datasize = (int) sfp_loopback.size;

  /* Clear loopback state (data ownership transferred to caller) */
  sfp_loopback.data = NULL;
  sfp_loopback.size = 0;
  sfp_loopback.pending = 0;

  return CSP_ERR_NONE;
}

void csp_free(void *ptr)
{
  if (ptr != NULL) {
    free(ptr);
  }
}

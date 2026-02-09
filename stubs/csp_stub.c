/**
 * @file csp_stub.c
 * @brief Minimal CSP stub implementation for testing
 *
 * This provides minimal stub implementations for CSP functions
 * to allow testing of CSPCL without a full libcsp installation.
 */

#include "csp/csp.h"
#include <stdlib.h>
#include <string.h>

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
/* Buffer API                                                                 */
/*===========================================================================*/

csp_packet_t *csp_buffer_get(size_t size)
{
    (void)size;
    csp_packet_t *packet = (csp_packet_t *)calloc(1, sizeof(csp_packet_t));
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
    csp_socket_t *sock = (csp_socket_t *)calloc(1, sizeof(csp_socket_t));
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
    (void)socket;
    (void)backlog;
    return CSP_ERR_NONE;
}

csp_conn_t *csp_accept(csp_socket_t *socket, uint32_t timeout)
{
    (void)socket;
    (void)timeout;

    /* Return connection if there's a packet in the loopback queue */
    if (loopback_queue.count > 0) {
        csp_conn_t *conn = (csp_conn_t *)calloc(1, sizeof(csp_conn_t));
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

csp_conn_t *csp_connect(uint8_t prio, uint8_t dest, uint8_t dport,
                        uint32_t timeout, uint32_t opts)
{
    (void)prio;
    (void)timeout;
    (void)opts;

    csp_conn_t *conn = (csp_conn_t *)calloc(1, sizeof(csp_conn_t));
    if (conn != NULL) {
        conn->dst = dest;
        conn->dport = dport;
    }
    return conn;
}

int csp_send(csp_conn_t *conn, csp_packet_t *packet, uint32_t timeout)
{
    (void)conn;
    (void)packet;
    (void)timeout;
    return CSP_ERR_NONE;
}

csp_packet_t *csp_read(csp_conn_t *conn, uint32_t timeout)
{
    (void)conn;
    (void)timeout;

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

int csp_sendto(uint8_t prio, uint8_t dest, uint8_t dport, uint8_t sport,
               uint32_t opts, csp_packet_t *packet, uint32_t timeout)
{
    (void)prio;
    (void)dport;
    (void)sport;
    (void)opts;
    (void)timeout;

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
    (void)socket;
    (void)timeout;

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


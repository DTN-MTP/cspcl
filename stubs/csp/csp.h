/**
 * @file csp.h
 * @brief Minimal CSP v1.6 stub for CSPCL compilation
 *
 * This is a stub header that provides the minimal CSP API definitions
 * needed to compile CSPCL. Replace with actual libcsp headers for
 * real deployment.
 *
 * @note This is NOT a complete CSP implementation!
 */

#ifndef CSP_CSP_H
#define CSP_CSP_H

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "csp_types.h"

#ifdef __cplusplus
extern "C" {
#endif

/*===========================================================================*/
/* CSP Types                                                                  */
/*===========================================================================*/

/** CSP packet identifier - contains routing information */
typedef struct {
  uint8_t pri;   /**< Priority */
  uint8_t flags; /**< Flags */
  uint8_t src;   /**< Source address */
  uint8_t dst;   /**< Destination address */
  uint8_t dport; /**< Destination port */
  uint8_t sport; /**< Source port */
} csp_id_t;

/** CSP packet structure */
typedef struct {
  uint16_t length;
  csp_id_t id; /**< Packet identifier with routing info */
  uint8_t data[256];
} csp_packet_t;

/** CSP socket handle */
typedef struct csp_socket_s csp_socket_t;

/** CSP connection handle */
typedef struct csp_conn_s csp_conn_t;

/*===========================================================================*/
/* CSP Constants                                                              */
/*===========================================================================*/

/** CSP error codes */
#define CSP_ERR_NONE 0
#define CSP_ERR_NOMEM -1
#define CSP_ERR_INVAL -2
#define CSP_ERR_TIMEDOUT -3

/** CSP priorities */
#define CSP_PRIO_CRITICAL 0
#define CSP_PRIO_HIGH 1
#define CSP_PRIO_NORM 2
#define CSP_PRIO_LOW 3

/** CSP socket options */
#define CSP_SO_NONE 0x0000
#define CSP_SO_RDPREQ 0x0001
#define CSP_SO_RDPPROHIB 0x0002
#define CSP_SO_HMACREQ 0x0004
#define CSP_SO_HMACPROHIB 0x0008
#define CSP_SO_XTEAREQ 0x0010
#define CSP_SO_XTEAPROHIB 0x0020
#define CSP_SO_CRC32REQ 0x0040
#define CSP_SO_CRC32PROHIB 0x0080
#define CSP_SO_CONN_LESS 0x0100

/** CSP connection options */
#define CSP_O_NONE 0x0000
#define CSP_O_RDP 0x0001
#define CSP_O_HMAC 0x0002
#define CSP_O_XTEA 0x0004
#define CSP_O_CRC32 0x0008

/* Router init/task API (stubbed) */
void csp_conf_get_defaults(csp_conf_t *conf);
int csp_init(const csp_conf_t *conf);
int csp_route_start_task(unsigned int stack_size, unsigned int prio);

/*===========================================================================*/
/* CSP Buffer API                                                             */
/*===========================================================================*/

/**
 * Get a CSP buffer
 * @param size Requested data size
 * @return Pointer to packet, or NULL on error
 */
csp_packet_t *csp_buffer_get(size_t size);

/**
 * Free a CSP buffer
 * @param packet Packet to free
 */
void csp_buffer_free(csp_packet_t *packet);

/*===========================================================================*/
/* CSP Socket API                                                             */
/*===========================================================================*/

/**
 * Create a CSP socket
 * @param opts Socket options
 * @return Socket handle, or NULL on error
 */
csp_socket_t *csp_socket(uint32_t opts);

/**
 * Bind socket to port
 * @param socket Socket handle
 * @param port Port number
 * @return CSP_ERR_NONE on success
 */
int csp_bind(csp_socket_t *socket, uint8_t port);

/**
 * Set socket to listen mode
 * @param socket Socket handle
 * @param backlog Connection backlog
 * @return CSP_ERR_NONE on success
 */
int csp_listen(csp_socket_t *socket, size_t backlog);

/**
 * Accept incoming connection
 * @param socket Socket handle
 * @param timeout Timeout in ms
 * @return Connection handle, or NULL on timeout/error
 */
csp_conn_t *csp_accept(csp_socket_t *socket, uint32_t timeout);

/**
 * Close socket or connection
 * @param conn Socket or connection handle
 * @return CSP_ERR_NONE on success
 */
int csp_close(void *conn);

/*===========================================================================*/
/* CSP Connection API                                                         */
/*===========================================================================*/

/**
 * Connect to remote host
 * @param prio Priority
 * @param dest Destination address
 * @param dport Destination port
 * @param timeout Timeout in ms
 * @param opts Connection options
 * @return Connection handle, or NULL on error
 */
csp_conn_t *csp_connect(uint8_t prio, uint8_t dest, uint8_t dport, uint32_t timeout, uint32_t opts);

/**
 * Send packet on connection
 * @param conn Connection handle
 * @param packet Packet to send
 * @param timeout Timeout in ms
 * @return CSP_ERR_NONE on success
 */
int csp_send(csp_conn_t *conn, csp_packet_t *packet, uint32_t timeout);

/**
 * Read packet from connection
 * @param conn Connection handle
 * @param timeout Timeout in ms
 * @return Packet, or NULL on timeout/error
 */
csp_packet_t *csp_read(csp_conn_t *conn, uint32_t timeout);

/*===========================================================================*/
/* CSP Connectionless API (UDP-like)                                          */
/*===========================================================================*/

/**
 * Send packet without connection (UDP-like)
 * @param prio Priority
 * @param dest Destination address
 * @param dport Destination port
 * @param sport Source port
 * @param opts Options
 * @param packet Packet to send
 * @param timeout Timeout in ms
 * @return CSP_ERR_NONE on success
 */
int csp_sendto(uint8_t prio, uint8_t dest, uint8_t dport, uint8_t sport, uint32_t opts,
               csp_packet_t *packet, uint32_t timeout);

/**
 * Receive packet without connection (UDP-like)
 * @param socket Socket handle
 * @param timeout Timeout in ms
 * @return Packet, or NULL on timeout/error
 */
csp_packet_t *csp_recvfrom(csp_socket_t *socket, uint32_t timeout);

/*===========================================================================*/
/* CSP Connection Info                                                        */
/*===========================================================================*/

/**
 * Get source address from connection
 * @param conn Connection handle
 * @return Source address
 */
uint8_t csp_conn_src(csp_conn_t *conn);

/**
 * Get destination address from connection
 * @param conn Connection handle
 * @return Destination address
 */
uint8_t csp_conn_dst(csp_conn_t *conn);

/**
 * Get source port from connection
 * @param conn Connection handle
 * @return Source port
 */
uint8_t csp_conn_sport(csp_conn_t *conn);

/**
 * Get destination port from connection
 * @param conn Connection handle
 * @return Destination port
 */
uint8_t csp_conn_dport(csp_conn_t *conn);

/*===========================================================================*/
/* CSP SFP (Simple Fragmentation Protocol) API                                */
/*===========================================================================*/

/** Memory copy function type for SFP */
typedef void *csp_memptr_t;
typedef void *(*csp_memcpy_fnc_t)(csp_memptr_t dst, csp_memptr_t src, size_t len);

/**
 * Send data using SFP (handles fragmentation automatically)
 * @param conn Connection handle
 * @param data Data to send
 * @param datasize Size of data
 * @param mtu Maximum transfer unit per packet
 * @param timeout Timeout in ms (unused in CSP 1.6)
 * @param memcpyfcn Memory copy function
 * @return CSP_ERR_NONE on success
 */
int csp_sfp_send_own_memcpy(csp_conn_t *conn, const void *data, unsigned int datasize,
                            unsigned int mtu, uint32_t timeout, csp_memcpy_fnc_t memcpyfcn);

/**
 * Send data using SFP with standard memcpy
 * @param conn Connection handle
 * @param data Data to send
 * @param datasize Size of data
 * @param mtu Maximum transfer unit per packet
 * @param timeout Timeout in ms (unused in CSP 1.6)
 * @return CSP_ERR_NONE on success
 */
static inline int csp_sfp_send(csp_conn_t *conn, const void *data, unsigned int datasize,
                               unsigned int mtu, uint32_t timeout)
{
  return csp_sfp_send_own_memcpy(conn, data, datasize, mtu, timeout, (csp_memcpy_fnc_t) &memcpy);
}

/**
 * Receive data using SFP (handles reassembly automatically)
 * @param conn Connection handle
 * @param dataout Output pointer to received data (allocated by SFP, free with
 * csp_free)
 * @param datasize Output size of received data
 * @param timeout Timeout in ms
 * @param first_packet First packet if already received, or NULL
 * @return CSP_ERR_NONE on success
 */
int csp_sfp_recv_fp(csp_conn_t *conn, void **dataout, int *datasize, uint32_t timeout,
                    csp_packet_t *first_packet);

/**
 * Receive data using SFP (convenience wrapper)
 * @param conn Connection handle
 * @param dataout Output pointer to received data (allocated by SFP, free with
 * csp_free)
 * @param datasize Output size of received data
 * @param timeout Timeout in ms
 * @return CSP_ERR_NONE on success
 */
static inline int csp_sfp_recv(csp_conn_t *conn, void **dataout, int *datasize, uint32_t timeout)
{
  return csp_sfp_recv_fp(conn, dataout, datasize, timeout, NULL);
}

/**
 * Free memory allocated by CSP (e.g., from csp_sfp_recv)
 * @param ptr Pointer to free
 */
void csp_free(void *ptr);

#ifdef __cplusplus
}
#endif

#endif /* CSP_CSP_H */

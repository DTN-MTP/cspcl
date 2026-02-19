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
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

/* CSP library headers */
#include <csp/csp.h>
#include <csp/arch/csp_malloc.h>

/*===========================================================================*/
/* Initialization Functions                                                   */
/*===========================================================================*/

cspcl_error_t cspcl_init(cspcl_t *cspcl, uint8_t local_addr)
{
    if (cspcl == NULL) {
        return CSPCL_ERR_INVALID_PARAM;
    }

    memset(cspcl, 0, sizeof(cspcl_t));
    cspcl->local_addr = local_addr;
    cspcl->rx_socket = NULL;
    cspcl->initialized = true;

    return CSPCL_OK;
}

void cspcl_cleanup(cspcl_t *cspcl)
{
    if (cspcl == NULL) {
        return;
    }

    /* Close server socket */
    cspcl_close_rx_socket(cspcl);

    cspcl->initialized = false;
}

/*===========================================================================*/
/* Bundle Transmission Functions                                              */
/*===========================================================================*/

cspcl_error_t cspcl_send_bundle(cspcl_t *cspcl,
                                 const uint8_t *bundle,
                                 size_t len,
                                 uint8_t dest_addr)
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

    /* Open connection to destination */
    csp_conn_t *conn = csp_connect(CSP_PRIO_NORM,
                                    dest_addr,
                                    CSPCL_PORT_BP,
                                    CSPCL_CSP_TIMEOUT_MS,
                                    CSP_O_NONE);
    if (conn == NULL) {
        return CSPCL_ERR_CONNECTION;
    }

    /* Use CSP's SFP to send the bundle with automatic fragmentation */
    int ret = csp_sfp_send(conn,
                           bundle,
                           (unsigned int)len,
                           CSPCL_MAX_PAYLOAD,
                           CSPCL_CSP_TIMEOUT_MS);

    /* Close connection */
    csp_close(conn);

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
        return CSPCL_OK;  /* Already open */
    }

    /* Create socket for connection-oriented mode */
    csp_socket_t *sock = csp_socket(CSP_SO_NONE);
    if (sock == NULL) {
        return CSPCL_ERR_NO_MEMORY;
    }

    /* Bind to BP port */
    int bind_result = csp_bind(sock, CSPCL_PORT_BP);
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
    return CSPCL_OK;
}

/**
 * @brief Close the server socket
 *
 * @param cspcl CSPCL instance
 */
void cspcl_close_rx_socket(cspcl_t *cspcl)
{
    if (cspcl != NULL && cspcl->rx_socket != NULL) {
        csp_close((csp_socket_t *)cspcl->rx_socket);
        cspcl->rx_socket = NULL;
    }
}

cspcl_error_t cspcl_recv_bundle(cspcl_t *cspcl,
                                 uint8_t *bundle,
                                 size_t *len,
                                 uint8_t *src_addr,
                                 uint32_t timeout_ms)
{
    if (cspcl == NULL || bundle == NULL || len == NULL) {
        return CSPCL_ERR_INVALID_PARAM;
    }

    if (!cspcl->initialized) {
        return CSPCL_ERR_NOT_INITIALIZED;
    }

    size_t max_len = *len;
    *len = 0;

    /* RX socket should already be open from initialization */
    if (cspcl->rx_socket == NULL) {
        return CSPCL_ERR_NOT_INITIALIZED;
    }

    /* Accept incoming connection */
    csp_conn_t *conn = csp_accept((csp_socket_t *)cspcl->rx_socket,
                                   timeout_ms > 0 ? timeout_ms : CSPCL_CSP_TIMEOUT_MS);
    if (conn == NULL) {
        return CSPCL_ERR_TIMEOUT;
    }

    /* Get source address from connection */
    uint8_t pkt_src_addr = csp_conn_src(conn);

    /* Use CSP's SFP to receive the bundle with automatic reassembly */
    void *data = NULL;
    int datasize = 0;

    int ret = csp_sfp_recv(conn,
                           &data,
                           &datasize,
                           CSPCL_SFP_TIMEOUT_MS);

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
    if ((size_t)datasize > max_len) {
        csp_free(data);
        return CSPCL_ERR_NO_MEMORY;
    }

    /* Copy received bundle to output buffer */
    memcpy(bundle, data, (size_t)datasize);
    *len = (size_t)datasize;

    if (src_addr != NULL) {
        *src_addr = pkt_src_addr;
    }

    /* Free SFP-allocated memory */
    csp_free(data);

    return CSPCL_OK;
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
                return (uint8_t)node;
            }
        }
    }

    /* Parse DTN scheme: dtn://nodeX/... → CSP address X */
    if (strncmp(endpoint_id, "dtn://node", 10) == 0) {
        int node = 0;
        if (sscanf(endpoint_id + 10, "%d", &node) == 1) {
            if (node >= 0 && node <= 255) {
                return (uint8_t)node;
            }
        }
    }

    return 0;
}

cspcl_error_t cspcl_addr_to_endpoint(uint8_t addr,
                                      char *endpoint,
                                      size_t len)
{
    if (endpoint == NULL || len < 12) {
        return CSPCL_ERR_INVALID_PARAM;
    }

    /* Generate IPN endpoint: CSP address X → ipn:X.0 */
    int written = snprintf(endpoint, len, "ipn:%d.0", addr);
    if (written < 0 || (size_t)written >= len) {
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
        case CSPCL_OK:                  return "Success";
        case CSPCL_ERR_INVALID_PARAM:   return "Invalid parameter";
        case CSPCL_ERR_NO_MEMORY:       return "Memory allocation failed";
        case CSPCL_ERR_BUNDLE_TOO_LARGE:return "Bundle exceeds maximum size";
        case CSPCL_ERR_CSP_SEND:        return "CSP send failed";
        case CSPCL_ERR_CSP_RECV:        return "CSP receive failed";
        case CSPCL_ERR_TIMEOUT:         return "Operation timed out";
        case CSPCL_ERR_SFP:             return "SFP fragmentation/reassembly failed";
        case CSPCL_ERR_NOT_INITIALIZED: return "CSPCL not initialized";
        case CSPCL_ERR_CONNECTION:      return "CSP connection failed";
        default:                        return "Unknown error";
    }
}


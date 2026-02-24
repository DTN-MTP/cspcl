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
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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
/* Initialization Functions                                                   */
/*===========================================================================*/

cspcl_error_t cspcl_init(cspcl_t *cspcl) {
  if (cspcl == NULL) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (!cspcl->initialized) {

    /* Configure CSP */
    csp_conf_t csp_conf;
    csp_conf_get_defaults(&csp_conf);
    csp_conf.address = cspcl->local_addr;
    csp_conf.hostname = "ud3tn";
    csp_conf.model = "csp-cla";
    csp_conf.revision = "1.0";
    csp_conf.conn_max = 10;
    csp_conf.conn_queue_length = 100;
    csp_conf.fifo_length = 25;
    csp_conf.port_max_bind = 31;
    csp_conf.rdp_max_window = 20;
    csp_conf.buffers = 100;
    csp_conf.buffer_data_size = 256;

    int ret = csp_init(&csp_conf);
    /* Initialize the CSP stack */
    if (ret != CSP_ERR_NONE) {
      return CSPCL_ERR_CSP_STACK_INIT;
    }

    /* Initialize the selected interface */
    switch (cspcl->iface_type) {
    case CSP_IFACE_ZMQHUB:
      ret = csp_zmqhub_init(cspcl->local_addr, cspcl->zmqhub_addr, 0,
                            &cspcl->active_iface);
      if (ret != CSP_ERR_NONE) {
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
        return CSPCL_ERR_CSP_CAN_INIT;
      }
#else
      return CSPCL_ERR_CSP_CAN_NOT_SUPPORTED;
#endif
      break;

    case CSP_IFACE_LOOPBACK:
      cspcl->active_iface = NULL; /* Will use default loopback */
      break;
    }

    /* Set default route via active interface */
    if (cspcl->active_iface != NULL) {
      csp_rtable_set(CSP_DEFAULT_ROUTE, 0, cspcl->active_iface, CSP_NODE_MAC);
    }

    /* Start the CSP router task */
    ret = csp_route_start_task(500, 0);
    if (ret != CSP_ERR_NONE) {
      return CSPCL_ERR_CSP_ROUTER;
    }

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

void cspcl_cleanup(cspcl_t *cspcl) {
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

cspcl_error_t cspcl_send_bundle(cspcl_t *cspcl, const uint8_t *bundle,
                                size_t len, uint8_t dest_addr) {
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
  csp_conn_t *conn = csp_connect(CSP_PRIO_NORM, dest_addr, CSPCL_PORT_BP,
                                 CSPCL_CSP_TIMEOUT_MS, CSP_O_NONE);
  if (conn == NULL) {
    return CSPCL_ERR_CONNECTION;
  }

  /* Use CSP's SFP to send the bundle with automatic fragmentation */
  int ret = csp_sfp_send(conn, bundle, (unsigned int)len, CSPCL_MAX_PAYLOAD,
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
cspcl_error_t cspcl_open_rx_socket(cspcl_t *cspcl) {
  if (cspcl == NULL || !cspcl->initialized) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (cspcl->rx_socket != NULL) {
    return CSPCL_OK; /* Already open */
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
void cspcl_close_rx_socket(cspcl_t *cspcl) {
  if (cspcl != NULL && cspcl->rx_socket != NULL) {
    csp_close((csp_socket_t *)cspcl->rx_socket);
    cspcl->rx_socket = NULL;
  }
}

cspcl_error_t cspcl_recv_bundle(cspcl_t *cspcl, uint8_t *bundle, size_t *len,
                                uint8_t *src_addr, uint32_t timeout_ms) {
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
  csp_conn_t *conn =
      csp_accept((csp_socket_t *)cspcl->rx_socket,
                 timeout_ms > 0 ? timeout_ms : CSPCL_CSP_TIMEOUT_MS);
  if (conn == NULL) {
    return CSPCL_ERR_TIMEOUT;
  }

  /* Get source address from connection */
  uint8_t pkt_src_addr = csp_conn_src(conn);

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

uint8_t cspcl_endpoint_to_addr(const char *endpoint_id) {
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

cspcl_error_t cspcl_addr_to_endpoint(uint8_t addr, char *endpoint, size_t len) {
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

const char *cspcl_strerror(cspcl_error_t err) {
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
  default:
    return "Unknown error";
  }
}

/**
 * @file cspcl.c
 * @brief CSPCL - CubeSat Space Protocol Convergence Layer Implementation
 *
 * @version 1.0
 */

#include "cspcl.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

/* CSP library headers */
#include <csp/csp.h>

/*===========================================================================*/
/* Internal Helper Functions                                                  */
/*===========================================================================*/

/**
 * @brief Serialize CSPCL header to network byte order
 */
static void cspcl_header_serialize(const cspcl_header_t *header, uint8_t *buf)
{
    buf[0] = header->version;
    buf[1] = header->flags;
    buf[2] = (header->fragment_id >> 8) & 0xFF;
    buf[3] = header->fragment_id & 0xFF;
    buf[4] = (header->fragment_offset >> 8) & 0xFF;
    buf[5] = header->fragment_offset & 0xFF;
    buf[6] = (header->bundle_size >> 24) & 0xFF;
    buf[7] = (header->bundle_size >> 16) & 0xFF;
    buf[8] = (header->bundle_size >> 8) & 0xFF;
    buf[9] = header->bundle_size & 0xFF;
}

/**
 * @brief Deserialize CSPCL header from network byte order
 */
static void cspcl_header_deserialize(const uint8_t *buf, cspcl_header_t *header)
{
    header->version = buf[0];
    header->flags = buf[1];
    header->fragment_id = ((uint16_t)buf[2] << 8) | buf[3];
    header->fragment_offset = ((uint16_t)buf[4] << 8) | buf[5];
    header->bundle_size = ((uint32_t)buf[6] << 24) |
                          ((uint32_t)buf[7] << 16) |
                          ((uint32_t)buf[8] << 8) |
                          buf[9];
}

/**
 * @brief Find or create a reassembly context
 */
static cspcl_reassembly_ctx_t *cspcl_find_reassembly_ctx(cspcl_t *cspcl,
                                                          uint8_t src_addr,
                                                          uint16_t fragment_id,
                                                          uint32_t bundle_size)
{
    cspcl_reassembly_ctx_t *ctx = NULL;
    cspcl_reassembly_ctx_t *free_ctx = NULL;

    /* Look for existing context or free slot */
    for (int i = 0; i < CSPCL_MAX_REASSEMBLY_CTX; i++) {
        if (cspcl->reassembly[i].in_use) {
            if (cspcl->reassembly[i].src_addr == src_addr &&
                cspcl->reassembly[i].fragment_id == fragment_id) {
                return &cspcl->reassembly[i];
            }
        } else if (free_ctx == NULL) {
            free_ctx = &cspcl->reassembly[i];
        }
    }

    /* Create new context if we have a free slot */
    if (free_ctx != NULL) {
        free_ctx->in_use = true;
        free_ctx->src_addr = src_addr;
        free_ctx->fragment_id = fragment_id;
        free_ctx->bundle_size = bundle_size;
        free_ctx->received_bytes = 0;
        free_ctx->timestamp_ms = cspcl_get_time_ms();

        /* Allocate reassembly buffer */
        free_ctx->buffer = (uint8_t *)malloc(bundle_size);
        if (free_ctx->buffer == NULL) {
            free_ctx->in_use = false;
            return NULL;
        }

        /* Clear received mask */
        memset(free_ctx->received_mask, 0, sizeof(free_ctx->received_mask));

        ctx = free_ctx;
    }

    return ctx;
}

/**
 * @brief Free a reassembly context
 */
static void cspcl_free_reassembly_ctx(cspcl_reassembly_ctx_t *ctx)
{
    if (ctx != NULL && ctx->in_use) {
        if (ctx->buffer != NULL) {
            free(ctx->buffer);
            ctx->buffer = NULL;
        }
        ctx->in_use = false;
    }
}

/**
 * @brief Check if a fragment has been received
 */
static bool cspcl_is_fragment_received(cspcl_reassembly_ctx_t *ctx,
                                        uint16_t offset,
                                        uint16_t len)
{
    (void)len; /* Reserved for future use */

    /* Simple check - mark fragment as received using offset index */
    uint16_t fragment_idx = offset / CSPCL_MAX_PAYLOAD;
    if (fragment_idx < 256 * 32) {
        uint32_t word_idx = fragment_idx / 32;
        uint32_t bit_idx = fragment_idx % 32;
        return (ctx->received_mask[word_idx] & (1U << bit_idx)) != 0;
    }
    return false;
}

/**
 * @brief Mark a fragment as received
 */
static void cspcl_mark_fragment_received(cspcl_reassembly_ctx_t *ctx,
                                          uint16_t offset,
                                          uint16_t len)
{
    (void)len; /* Reserved for future use */

    uint16_t fragment_idx = offset / CSPCL_MAX_PAYLOAD;
    if (fragment_idx < 256 * 32) {
        uint32_t word_idx = fragment_idx / 32;
        uint32_t bit_idx = fragment_idx % 32;
        ctx->received_mask[word_idx] |= (1U << bit_idx);
    }
}

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
    cspcl->next_fragment_id = 1;
    cspcl->rx_socket = NULL;
    cspcl->initialized = true;

    return CSPCL_OK;
}

void cspcl_cleanup(cspcl_t *cspcl)
{
    if (cspcl == NULL) {
        return;
    }

    /* Close RX socket */
    cspcl_close_rx_socket(cspcl);

    /* Free all reassembly contexts */
    for (int i = 0; i < CSPCL_MAX_REASSEMBLY_CTX; i++) {
        cspcl_free_reassembly_ctx(&cspcl->reassembly[i]);
    }

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

    /* Get a new fragment ID for this bundle */
    uint16_t fragment_id = cspcl->next_fragment_id++;

    /* Calculate number of fragments needed */
    size_t num_fragments = (len + CSPCL_MAX_PAYLOAD - 1) / CSPCL_MAX_PAYLOAD;

    /* Send each fragment */
    size_t offset = 0;
    for (size_t frag = 0; frag < num_fragments; frag++) {
        /* Calculate payload size for this fragment */
        size_t payload_size = len - offset;
        if (payload_size > CSPCL_MAX_PAYLOAD) {
            payload_size = CSPCL_MAX_PAYLOAD;
        }

        /* Build CSPCL header */
        cspcl_header_t header;
        header.version = CSPCL_VERSION;
        header.flags = 0;

        if (frag == 0) {
            header.flags |= CSPCL_FLAG_FIRST;
        }
        if (frag == num_fragments - 1) {
            header.flags |= CSPCL_FLAG_LAST;
        } else {
            header.flags |= CSPCL_FLAG_MORE;
        }

        header.fragment_id = fragment_id;
        header.fragment_offset = (uint16_t)offset;
        header.bundle_size = (uint32_t)len;

        /* Allocate CSP packet buffer */
        csp_packet_t *packet = csp_buffer_get(CSPCL_HEADER_SIZE + payload_size);
        if (packet == NULL) {
            return CSPCL_ERR_NO_MEMORY;
        }

        /* Serialize header into packet */
        cspcl_header_serialize(&header, packet->data);

        /* Copy payload */
        memcpy(packet->data + CSPCL_HEADER_SIZE, bundle + offset, payload_size);
        packet->length = CSPCL_HEADER_SIZE + payload_size;

        /* Send packet via CSP UDP */
        int ret = csp_sendto(CSP_PRIO_NORM,
                             dest_addr,
                             CSPCL_PORT_BP,
                             CSPCL_PORT_BP,
                             CSP_O_NONE,
                             packet,
                             CSPCL_CSP_TIMEOUT_MS);

        if (ret != CSP_ERR_NONE) {
            csp_buffer_free(packet);
            return CSPCL_ERR_CSP_SEND;
        }

        offset += payload_size;
    }

    return CSPCL_OK;
}

/**
 * @brief Open and bind a persistent receive socket
 *
 * This should be called once during initialization to create
 * a socket bound to the BP port for receiving bundles.
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

    /* Create connectionless socket */
    csp_socket_t *sock = csp_socket(CSP_SO_CONN_LESS);
    if (sock == NULL) {
        return CSPCL_ERR_NO_MEMORY;
    }

    /* Bind to BP port */
    int bind_result = csp_bind(sock, CSPCL_PORT_BP);
    if (bind_result != CSP_ERR_NONE) {
        csp_close(sock);
        return CSPCL_ERR_CSP_RECV;
    }

    cspcl->rx_socket = sock;
    return CSPCL_OK;
}

/**
 * @brief Close the receive socket
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

    /* RX socket should already be open from initialization.
     * If it's NULL here, something went wrong. */
    if (cspcl->rx_socket == NULL) {
        /* This should not happen if cspcl_open_rx_socket was called during init */
        return CSPCL_ERR_NOT_INITIALIZED;
    }

    uint64_t start_time = cspcl_get_time_ms();
    cspcl_error_t result = CSPCL_ERR_TIMEOUT;

    while (1) {
        /* Check timeout */
        if (timeout_ms > 0) {
            uint64_t elapsed = cspcl_get_time_ms() - start_time;
            if (elapsed >= timeout_ms) {
                break;
            }
        }

        /* Clean up expired reassembly contexts */
        cspcl_cleanup_expired(cspcl);

        /* Receive packet using connectionless API (matches csp_sendto) */
        csp_packet_t *packet = csp_recvfrom((csp_socket_t *)cspcl->rx_socket, CSPCL_CSP_TIMEOUT_MS);
        if (packet == NULL) {
            continue;
        }

        /* Validate packet size */
        if (packet->length < CSPCL_HEADER_SIZE) {
            csp_buffer_free(packet);
            continue;
        }

        /* Parse header */
        cspcl_header_t header;
        cspcl_header_deserialize(packet->data, &header);

        /* Validate header */
        if (header.version != CSPCL_VERSION) {
            csp_buffer_free(packet);
            result = CSPCL_ERR_VERSION_MISMATCH;
            continue;
        }

        /* Extract source address from packet header
         * In CSP, the packet structure contains id with source/dest info */
        uint8_t pkt_src_addr = (packet->id.src);
        uint16_t payload_len = packet->length - CSPCL_HEADER_SIZE;

        /* Handle single-packet bundle (no fragmentation) */
        if ((header.flags & CSPCL_FLAG_FIRST) && (header.flags & CSPCL_FLAG_LAST)) {
            if (header.bundle_size > max_len) {
                csp_buffer_free(packet);
                result = CSPCL_ERR_NO_MEMORY;
                continue;
            }

            memcpy(bundle, packet->data + CSPCL_HEADER_SIZE, payload_len);
            *len = payload_len;
            if (src_addr != NULL) {
                *src_addr = pkt_src_addr;
            }

            csp_buffer_free(packet);
            return CSPCL_OK;
        }

        /* Handle fragmented bundle */
        cspcl_reassembly_ctx_t *ctx = cspcl_find_reassembly_ctx(
            cspcl, pkt_src_addr, header.fragment_id, header.bundle_size);

        if (ctx == NULL) {
            csp_buffer_free(packet);
            result = CSPCL_ERR_NO_MEMORY;
            continue;
        }

        /* Check if fragment already received */
        if (!cspcl_is_fragment_received(ctx, header.fragment_offset, payload_len)) {
            /* Validate offset and size */
            if (header.fragment_offset + payload_len <= ctx->bundle_size) {
                /* Copy fragment data */
                memcpy(ctx->buffer + header.fragment_offset,
                       packet->data + CSPCL_HEADER_SIZE,
                       payload_len);

                ctx->received_bytes += payload_len;
                cspcl_mark_fragment_received(ctx, header.fragment_offset, payload_len);
            }
        }

        csp_buffer_free(packet);

        /* Check if bundle is complete */
        if (ctx->received_bytes >= ctx->bundle_size) {
            if (ctx->bundle_size > max_len) {
                cspcl_free_reassembly_ctx(ctx);
                result = CSPCL_ERR_NO_MEMORY;
                continue;
            }

            memcpy(bundle, ctx->buffer, ctx->bundle_size);
            *len = ctx->bundle_size;
            if (src_addr != NULL) {
                *src_addr = ctx->src_addr;
            }

            cspcl_free_reassembly_ctx(ctx);
            return CSPCL_OK;
        }
    }

    return result;
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

#ifdef __linux__
#include <time.h>

uint64_t cspcl_get_time_ms(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000 + (uint64_t)ts.tv_nsec / 1000000;
}

#elif defined(FREERTOS)
#include "FreeRTOS.h"
#include "task.h"

uint64_t cspcl_get_time_ms(void)
{
    return (uint64_t)xTaskGetTickCount() * portTICK_PERIOD_MS;
}

#else
/* Fallback - user must provide implementation */
__attribute__((weak))
uint64_t cspcl_get_time_ms(void)
{
    return 0;
}
#endif

void cspcl_cleanup_expired(cspcl_t *cspcl)
{
    if (cspcl == NULL) {
        return;
    }

    uint64_t now = cspcl_get_time_ms();

    for (int i = 0; i < CSPCL_MAX_REASSEMBLY_CTX; i++) {
        cspcl_reassembly_ctx_t *ctx = &cspcl->reassembly[i];
        if (ctx->in_use) {
            if (now - ctx->timestamp_ms > CSPCL_REASSEMBLY_TIMEOUT_MS) {
                cspcl_free_reassembly_ctx(ctx);
            }
        }
    }
}

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
        case CSPCL_ERR_REASSEMBLY:      return "Reassembly failed";
        case CSPCL_ERR_VERSION_MISMATCH:return "Protocol version mismatch";
        case CSPCL_ERR_NOT_INITIALIZED: return "CSPCL not initialized";
        case CSPCL_ERR_INVALID_HEADER:  return "Invalid CSPCL header";
        default:                        return "Unknown error";
    }
}


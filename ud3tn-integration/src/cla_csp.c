// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
/**
 * @file cla_csp.c
 * @brief CSP Convergence Layer Adapter for uD3TN
 *
 * This CLA integrates CSPCL (CubeSat Space Protocol Convergence Layer)
 * with uD3TN's bundle processor to enable BP7 over CSP.
 *
 * @version 1.0
 */

#include "cla/cla_csp.h"

#include "bundle6/parser.h"
#include "bundle7/parser.h"
#include "cla/cla.h"
#include "cla/cla_contact_tx_task.h"
#include "platform/hal_io.h"
#include "platform/hal_queue.h"
#include "platform/hal_semaphore.h"
#include "platform/hal_task.h"
#include "platform/hal_time.h"
#include "platform/hal_types.h"
#include "ud3tn/bundle_processor.h"
#include "ud3tn/cmdline.h"
#include "ud3tn/common.h"
#include "ud3tn/result.h"
#include "ud3tn/simplehtab.h"

/* CSPCL library */
#include "cspcl.h"

/* Configuration defaults (fallback if cspcl_config.h not available in build) */
#ifndef CSPCL_TX_BUFFER_SIZE
#define CSPCL_TX_BUFFER_SIZE 65536
#endif
#ifndef CSPCL_RX_BUFFER_SIZE
#define CSPCL_RX_BUFFER_SIZE 65536
#endif
#ifndef CSPCL_IFACE_PARAM_MAX
#define CSPCL_IFACE_PARAM_MAX 64
#endif
#ifndef CSPCL_ZMQHUB_ADDR_DEFAULT
#define CSPCL_ZMQHUB_ADDR_DEFAULT "localhost"
#endif
#ifndef CSPCL_CAN_IFACE_DEFAULT
#define CSPCL_CAN_IFACE_DEFAULT "vcan0"
#endif

/* libcsp headers for direct CSP stack control */
#include <csp/csp.h>
#include <csp/csp_rtable.h>
#include <csp/interfaces/csp_if_zmqhub.h>

/* CAN interface support - requires libcsp built with CAN driver */
#ifdef CSP_HAVE_LIBSOCKETCAN
#include <csp/drivers/can_socketcan.h>
#include <csp/interfaces/csp_if_can.h>
#endif

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/*===========================================================================*/
/* Constants                                                                  */
/*===========================================================================*/

static const char *CLA_NAME = "csp";

/** Hash table slots for parameter storage */
#define CSP_PARAM_HTAB_SLOT_COUNT 8

/* All other constants now defined in cspcl_config.h:
 * - CSPCL_MAX_LINKS = CSPCL_MAX_LINKS
 * - CSP_DEFAULT_BP_PORT = CSPCL_PORT_BP
 * - CSP_RX_BUFFER_SIZE = CSPCL_RX_BUFFER_SIZE
 * - CSP_TX_BUFFER_SIZE = CSPCL_TX_BUFFER_SIZE
 * - CSP_ZMQHUB_ADDR_DEFAULT = CSPCL_ZMQHUB_ADDR_DEFAULT
 * - CSP_CAN_IFACE_DEFAULT = CSPCL_CAN_IFACE_DEFAULT
 * - CSP_IFACE_PARAM_MAX = CSPCL_IFACE_PARAM_MAX
 */

/** Flag to track if CSP has been initialized globally */
static bool csp_initialized = false;

/** Active CSP interface */
static csp_iface_t *csp_active_iface = NULL;

/*===========================================================================*/
/* Data Structures                                                            */
/*===========================================================================*/

/**
 * @brief CSP CLA configuration
 */
struct csp_cla_config {
  struct cla_config base;

  /* CSPCL instance */
  cspcl_t cspcl;

  /* Link management */
  struct htab_entrylist *param_htab_elem[CSP_PARAM_HTAB_SLOT_COUNT];
  struct htab param_htab;
  Semaphore_t param_htab_sem;

  /* RX task control */
  bool rx_running;
  Semaphore_t rx_task_sem;
};

/**
 * @brief CSP link structure
 */
struct csp_link {
  struct cla_link base;

  /* CSPCL-specific protocol parser */
  struct parser csp_parser;

  /* Destination CSP address for this link */
  uint8_t dest_addr;

  uint8_t dest_port;

  /* TX buffer for accumulating bundle data before sending */
  uint8_t *tx_buffer;
  size_t tx_buffer_len;
  size_t tx_buffer_capacity;

  /* RX buffer for incoming bundle data */
  uint8_t *rx_buffer;
  size_t rx_buffer_len;
  size_t rx_buffer_pos;
};

/**
 * @brief CSP contact parameters for link management
 */
struct csp_contact_parameters {
  struct csp_link link;
  struct csp_cla_config *config;

  Semaphore_t param_semphr;

  char *cla_addr;
  uint8_t dest_addr;
  uint8_t dest_port;

  bool in_contact;
  bool is_outgoing;
};

/*===========================================================================*/
/* Forward Declarations                                                       */
/*===========================================================================*/

static const struct cla_vtable csp_vtable;

/*===========================================================================*/
/* Parser Implementation                                                      */
/*===========================================================================*/

/**
 * @brief Reset CSP parser state
 *
 * CSP bundles arrive as complete units via CSPCL (which handles
 * fragmentation/reassembly), so the parser is straightforward.
 */
static void csp_parser_reset(struct parser *parser)
{
  parser->status = PARSER_STATUS_GOOD;
  parser->next_bytes = 0;
  parser->flags = PARSER_FLAG_NONE;
}

/**
 * @brief Parse CSP data
 *
 * CSPCL delivers complete bundles, so we just signal that all data
 * should be forwarded to the bundle parser.
 */
static size_t csp_parser_parse(struct parser *parser, const uint8_t *buffer, size_t length)
{
  (void) buffer;

  /* Signal that data should be forwarded to bundle subparser */
  parser->flags = PARSER_FLAG_DATA_SUBPARSER;
  parser->next_bytes = length;

  return 0; /* Header bytes consumed (none for CSP - data is raw bundle) */
}

/*===========================================================================*/
/* Helper Functions                                                           */
/*===========================================================================*/

/* PHASE 2: Address parsing from cspcl library
 * Uses cspcl_parse_address(), cspcl_parse_port(), and cspcl_is_valid_address_string()
 * for standardized address parsing across all integrations
 */

static char *create_cla_addr(uint8_t csp_addr, uint8_t csp_port)
{
  char *addr = malloc(16);
  if (addr) {
    snprintf(addr, 16, "%u,%u", csp_addr, csp_port);
  }
  return addr;
}

/*===========================================================================*/
/* Link Management                                                            */
/*===========================================================================*/

static enum ud3tn_result csp_link_init(struct csp_link *link, struct csp_cla_config *config,
                                       uint8_t dest_addr, uint8_t dest_port, const char *cla_addr)
{
  /* Initialize base link */
  if (cla_link_init(&link->base, &config->base, cla_addr, true, true) != UD3TN_OK) {
    return UD3TN_FAIL;
  }

  link->dest_addr = dest_addr;
  link->dest_port = dest_port;

  /* Allocate TX buffer */
  link->tx_buffer = malloc(CSPCL_TX_BUFFER_SIZE);
  if (!link->tx_buffer) {
    LOG_ERROR("CSP: Failed to allocate TX buffer");
    cla_link_cleanup(&link->base);
    return UD3TN_FAIL;
  }
  link->tx_buffer_len = 0;
  link->tx_buffer_capacity = CSPCL_TX_BUFFER_SIZE;

  /* Allocate RX buffer */
  link->rx_buffer = malloc(CSPCL_RX_BUFFER_SIZE);
  if (!link->rx_buffer) {
    LOG_ERROR("CSP: Failed to allocate RX buffer");
    free(link->tx_buffer);
    cla_link_cleanup(&link->base);
    return UD3TN_FAIL;
  }
  link->rx_buffer_len = 0;
  link->rx_buffer_pos = 0;

  /* Initialize parser */
  csp_parser_reset(&link->csp_parser);

  return UD3TN_OK;
}

static void csp_link_cleanup(struct csp_link *link)
{
  if (link->tx_buffer) {
    free(link->tx_buffer);
    link->tx_buffer = NULL;
  }
  if (link->rx_buffer) {
    free(link->rx_buffer);
    link->rx_buffer = NULL;
  }

  cla_link_cleanup(&link->base);
}

/*===========================================================================*/
/* Contact Management                                                         */
/*===========================================================================*/

static void csp_link_management_task(void *p)
{
  struct csp_contact_parameters *const param = p;
  struct csp_cla_config *const config = param->config;

  LOGF_INFO("CSP: Starting link management for csp:%u", param->dest_addr);

  /* Initialize the link */
  if (csp_link_init(&param->link, config, param->dest_addr, param->dest_port, param->cla_addr) !=
      UD3TN_OK) {
    LOG_ERROR("CSP: Failed to initialize link");
    goto cleanup;
  }

  /* Release semaphore for TX/RX tasks */
  hal_semaphore_release(param->param_semphr);

  /* Wait for link tasks to complete */
  cla_link_wait(&param->link.base);

  /* Reacquire semaphore */
  hal_semaphore_take_blocking(param->config->param_htab_sem);
  hal_semaphore_take_blocking(param->param_semphr);
  csp_link_cleanup(&param->link);
  hal_semaphore_release(param->config->param_htab_sem);

cleanup:
  LOGF_INFO("CSP: Terminating link manager for csp:%u", param->dest_addr);

  /* Remove from hash table */
  hal_semaphore_take_blocking(param->config->param_htab_sem);
  if (htab_get(&param->config->param_htab, param->cla_addr) == param)
    htab_remove(&param->config->param_htab, param->cla_addr);
  hal_semaphore_release(param->config->param_htab_sem);

  /* Cleanup */
  hal_semaphore_take_blocking(param->param_semphr);
  free(param->cla_addr);
  hal_semaphore_delete(param->param_semphr);
  free(param);
}

static struct csp_contact_parameters *create_contact_params(struct csp_cla_config *config,
                                                            uint8_t dest_addr, uint8_t dest_port,
                                                            bool is_outgoing)
{
  struct csp_contact_parameters *param = malloc(sizeof(*param));
  if (!param) {
    LOG_ERROR("CSP: Failed to allocate contact parameters");
    return NULL;
  }

  memset(param, 0, sizeof(*param));
  param->config = config;
  param->dest_addr = dest_addr;
  param->dest_port = dest_port;
  param->cla_addr = create_cla_addr(dest_addr, dest_port);
  param->in_contact = true;
  param->is_outgoing = is_outgoing;

  if (!param->cla_addr) {
    free(param);
    return NULL;
  }

  param->param_semphr = hal_semaphore_init_binary();
  if (!param->param_semphr) {
    free(param->cla_addr);
    free(param);
    return NULL;
  }

  return param;
}

static void launch_connection_management(struct csp_cla_config *config, uint8_t dest_addr,
                                         uint8_t dest_port)
{
  struct csp_contact_parameters *param = create_contact_params(config, dest_addr, dest_port, true);

  if (!param)
    return;

  /* Add to hash table */
  struct htab_entrylist *entry = htab_add(&config->param_htab, param->cla_addr, param);

  if (!entry) {
    LOG_ERROR("CSP: Failed to add to hash table");
    hal_semaphore_delete(param->param_semphr);
    free(param->cla_addr);
    free(param);
    return;
  }

  /* Launch management task */
  if (hal_task_create(csp_link_management_task, param, true, NULL) != UD3TN_OK) {
    LOG_ERROR("CSP: Failed to create management task");
    htab_remove(&config->param_htab, param->cla_addr);
    hal_semaphore_delete(param->param_semphr);
    free(param->cla_addr);
    free(param);
  }
}

/*===========================================================================*/
/* RX Task                                                                    */
/*===========================================================================*/

static void csp_rx_task(void *p)
{
  struct csp_cla_config *const config = p;
  uint8_t bundle_buffer[CSPCL_RX_BUFFER_SIZE];
  size_t bundle_len;
  uint8_t src_addr;
  uint8_t src_port;

  LOG_INFO("CSP: RX task started");

  while (config->rx_running) {
    bundle_len = sizeof(bundle_buffer);
    /* Receive bundle via CSPCL */

    cspcl_error_t err = cspcl_recv_bundle(&config->cspcl, bundle_buffer, &bundle_len, &src_addr,
                                          &src_port, 1000 /* 1 second timeout */
    );

    /* Log every 10th iteration to confirm loop is running */
    static int loop_count = 0;
    if (++loop_count % 10 == 0) {
      LOGF_INFO("CSP: RX loop iteration %d, last_err=%d", loop_count, err);
    }

    if (err == CSPCL_ERR_TIMEOUT) {
      /* Expected on idle links - no incoming connection */
      static int timeout_count = 0;
      if (++timeout_count % 10 == 0) {
        LOGF_INFO("CSP: RX waiting - socket=%p, init=%d", config->cspcl.rx_socket,
                  config->cspcl.initialized);
      }
      continue;
    }

    if (err != CSPCL_OK) {
      /* Enhanced error logging for diagnosis */
      if (err == CSPCL_ERR_SFP) {
        LOGF_WARN("CSP: RX SFP ERROR - Incomplete bundle from csp:%u:%u (fragments missing, "
                  "discarding)",
                  src_addr, src_port);
      } else if (err == CSPCL_ERR_CSP_RECV) {
        LOGF_WARN("CSP: RX CSP_RECV error - Link issue from csp:%u:%u", src_addr, src_port);
      } else {
        LOGF_WARN("CSP: RX error code %d from csp:%u:%u", err, src_addr, src_port);
      }
      continue;
    }

    LOGF_INFO("CSP: RX COMPLETE - Got %zu bytes from csp:%u:%u (all fragments ACK'd)",
              bundle_len, src_addr, src_port);

    /* Look up or create link for this source */
    char *cla_addr = create_cla_addr(src_addr, src_port);
    if (!cla_addr)
      continue;

    hal_semaphore_take_blocking(config->param_htab_sem);
    struct csp_contact_parameters *param = htab_get(&config->param_htab, cla_addr);

    if (!param) {
      /* Create opportunistic link */
      launch_connection_management(config, src_addr, src_port);
      param = htab_get(&config->param_htab, cla_addr);
    }
    hal_semaphore_release(config->param_htab_sem);
    free(cla_addr);

    if (!param)
      continue;

    /* Store received data in link's RX buffer */
    hal_semaphore_take_blocking(param->param_semphr);
    if (bundle_len <= CSPCL_RX_BUFFER_SIZE - param->link.rx_buffer_len) {
      memcpy(param->link.rx_buffer + param->link.rx_buffer_len, bundle_buffer, bundle_len);
      param->link.rx_buffer_len += bundle_len;
    }
    hal_semaphore_release(param->param_semphr);
  }

  LOG_INFO("CSP: RX task terminated");
  hal_semaphore_release(config->rx_task_sem);
}

/*===========================================================================*/
/* CLA VTable Implementation                                                  */
/*===========================================================================*/

static const char *csp_cla_name_get(void)
{
  return CLA_NAME;
}

static enum ud3tn_result csp_cla_launch(struct cla_config *const config)
{
  struct csp_cla_config *const csp_config = (struct csp_cla_config *) config;

  csp_config->rx_running = true;

  return hal_task_create(csp_rx_task, csp_config, true, NULL);
}

static enum ud3tn_result csp_cla_terminate(struct cla_config *const config)
{
  struct csp_cla_config *const csp_config = (struct csp_cla_config *) config;

  csp_config->rx_running = false;

  /* Wait for RX task to finish */
  hal_semaphore_take_blocking(csp_config->rx_task_sem);

  LOG_DEBUG("CSP: CLA terminated gracefully");
  return UD3TN_OK;
}

size_t csp_cla_mbs_get(struct cla_config *const config)
{
  (void) config;
  /* CSPCL supports bundles up to CSPCL_MAX_BUNDLE_SIZE */
  return CSPCL_MAX_BUNDLE_SIZE;
}

static struct cla_tx_queue csp_cla_get_tx_queue(struct cla_config *config, const char *eid,
                                                const char *cla_addr)
{
  (void) eid;
  struct csp_cla_config *const csp_config = (struct csp_cla_config *) config;

  struct cla_tx_queue result = {
      .tx_queue_handle = NULL,
      .tx_queue_sem = NULL,
  };

  uint8_t dest_port = CSPCL_PORT_BP;
  uint8_t dest_addr = cspcl_parse_address(cla_addr, &dest_port);
  if (!cspcl_is_valid_address_string(cla_addr, dest_addr)) {
    LOGF_WARN("CSP: Invalid CLA address: %s", cla_addr);
    return result;
  }
  char *normalized_addr = create_cla_addr(dest_addr, dest_port);
  if (!normalized_addr) {
    return result;
  }

  hal_semaphore_take_blocking(csp_config->param_htab_sem);

  struct csp_contact_parameters *param = htab_get(&csp_config->param_htab, normalized_addr);

  if (param) {
    hal_semaphore_take_blocking(param->param_semphr);
    result.tx_queue_handle = param->link.base.tx_queue_handle;
    result.tx_queue_sem = param->link.base.tx_queue_sem;
    hal_semaphore_release(param->param_semphr);
  }

  hal_semaphore_release(csp_config->param_htab_sem);
  free(normalized_addr);

  return result;
}

static enum cla_link_update_result
csp_cla_start_scheduled_contact(struct cla_config *config, const char *eid, const char *cla_addr)
{
  (void) eid;
  struct csp_cla_config *const csp_config = (struct csp_cla_config *) config;

  uint8_t dest_port = CSPCL_PORT_BP;
  uint8_t dest_addr = cspcl_parse_address(cla_addr, &dest_port);
  if (!cspcl_is_valid_address_string(cla_addr, dest_addr)) {
    LOGF_WARN("CSP: Invalid CLA address for contact: %s", cla_addr);
    return CLA_LINK_UPDATE_FAILED;
  }
  char *normalized_addr = create_cla_addr(dest_addr, dest_port);
  if (!normalized_addr) {
    return CLA_LINK_UPDATE_FAILED;
  }
  LOGF_INFO("CSP: Starting scheduled contact to csp:%u", dest_addr);

  hal_semaphore_take_blocking(csp_config->param_htab_sem);

  /* Check if link already exists */
  struct csp_contact_parameters *existing = htab_get(&csp_config->param_htab, normalized_addr);

  if (existing) {
    hal_semaphore_take_blocking(existing->param_semphr);
    existing->in_contact = true;
    hal_semaphore_release(existing->param_semphr);
    hal_semaphore_release(csp_config->param_htab_sem);
    free(normalized_addr);
    return CLA_LINK_UPDATE_UNCHANGED;
  }

  /* Create new link */
  launch_connection_management(csp_config, dest_addr, dest_port);

  hal_semaphore_release(csp_config->param_htab_sem);
  free(normalized_addr);

  return CLA_LINK_UPDATE_INITIATED;
}

static enum cla_link_update_result
csp_cla_end_scheduled_contact(struct cla_config *config, const char *eid, const char *cla_addr)
{
  (void) eid;
  struct csp_cla_config *const csp_config = (struct csp_cla_config *) config;
  uint8_t dest_port = CSPCL_PORT_BP;
  uint8_t dest_addr = cspcl_parse_address(cla_addr, &dest_port);
  if (!cspcl_is_valid_address_string(cla_addr, dest_addr)) {
    LOGF_WARN("CSP: Invalid CLA address for contact end: %s", cla_addr);
    return CLA_LINK_UPDATE_FAILED;
  }
  char *normalized_addr = create_cla_addr(dest_addr, dest_port);
  if (!normalized_addr) {
    return CLA_LINK_UPDATE_FAILED;
  }

  LOGF_INFO("CSP: Ending scheduled contact to %s", cla_addr);

  hal_semaphore_take_blocking(csp_config->param_htab_sem);

  struct csp_contact_parameters *param = htab_get(&csp_config->param_htab, normalized_addr);

  if (param) {
    hal_semaphore_take_blocking(param->param_semphr);
    param->in_contact = false;
    /* Notify RX task to finish */
    hal_semaphore_try_take(param->link.base.rx_task_notification, 0);
    hal_semaphore_release(param->param_semphr);
    hal_semaphore_release(csp_config->param_htab_sem);
    free(normalized_addr);
    return CLA_LINK_UPDATE_INITIATED;
  }

  hal_semaphore_release(csp_config->param_htab_sem);
  free(normalized_addr);
  return CLA_LINK_UPDATE_UNCHANGED;
}

enum cla_begin_packet_result csp_cla_begin_packet(struct cla_link *link,
                                                  const struct bundle *const bundle, size_t length,
                                                  char *cla_addr)
{
  (void) bundle;
  (void) cla_addr;

  struct csp_link *const csp_link = (struct csp_link *) link;

  /* Reset TX buffer for new bundle */
  csp_link->tx_buffer_len = 0;

  /* Check if bundle fits in buffer */
  if (length > csp_link->tx_buffer_capacity) {
    LOG_WARN("CSP: Bundle too large for TX buffer");
    return CLA_BEGIN_PACKET_FAIL;
  }

  LOGF_DEBUG("CSP: Beginning packet of %zu bytes to csp:%u", length, csp_link->dest_addr);

  return CLA_BEGIN_PACKET_OK;
}

enum ud3tn_result csp_cla_end_packet(struct cla_link *link)
{
  struct csp_link *const csp_link = (struct csp_link *) link;
  struct csp_cla_config *const config = (struct csp_cla_config *) link->config;

  if (csp_link->tx_buffer_len == 0) {
    LOG_WARN("CSP: Empty packet, nothing to send");
    return UD3TN_OK;
  }

  LOGF_DEBUG("CSP: Sending %zu bytes to csp:%u:%u", csp_link->tx_buffer_len, csp_link->dest_addr,
             csp_link->dest_port);

  /* Send bundle via CSPCL */
  cspcl_error_t err =
      cspcl_send_bundle(&config->cspcl, csp_link->tx_buffer, csp_link->tx_buffer_len,
                        csp_link->dest_addr, csp_link->dest_port);

  /* Reset TX buffer */
  csp_link->tx_buffer_len = 0;

  if (err != CSPCL_OK) {
    /* Enhanced error logging with diagnostic information */
    if (err == CSPCL_ERR_TIMEOUT) {
      LOGF_ERROR("CSP: TX TIMEOUT - No ACKs received from csp:%u:%u (receiver offline or no link)",
                 csp_link->dest_addr, csp_link->dest_port);
    } else if (err == CSPCL_ERR_CONNECTION) {
      LOGF_ERROR("CSP: TX CONNECTION FAILED - Link broken to csp:%u:%u (will reconnect)",
                 csp_link->dest_addr, csp_link->dest_port);
    } else if (err == CSPCL_ERR_CSP_SEND) {
      LOGF_ERROR("CSP: TX CSP_SEND error - CSP layer problem csp:%u:%u", csp_link->dest_addr,
                 csp_link->dest_port);
    } else if (err == CSPCL_ERR_BUNDLE_TOO_LARGE) {
      LOGF_ERROR("CSP: TX BUNDLE TOO LARGE - %zu bytes exceeds max", csp_link->tx_buffer_len);
    } else {
      LOGF_WARN("CSP: TX failed - %s", cspcl_strerror(err));
    }

    link->config->vtable->cla_disconnect_handler(link);
    return UD3TN_FAIL;
  }

  LOGF_DEBUG("CSP: TX success - bundle ACK'd by receiver (%zu bytes)", csp_link->tx_buffer_len);
  return UD3TN_OK;
}

enum ud3tn_result csp_cla_send_packet_data(struct cla_link *link, const void *data,
                                           const size_t length)
{
  struct csp_link *const csp_link = (struct csp_link *) link;

  /* Accumulate data in TX buffer */
  if (csp_link->tx_buffer_len + length > csp_link->tx_buffer_capacity) {
    LOG_WARN("CSP: TX buffer overflow");
    return UD3TN_FAIL;
  }

  memcpy(csp_link->tx_buffer + csp_link->tx_buffer_len, data, length);
  csp_link->tx_buffer_len += length;

  return UD3TN_OK;
}

void csp_cla_reset_parsers(struct cla_link *link)
{
  struct csp_link *const csp_link = (struct csp_link *) link;

  rx_task_reset_parsers(&link->rx_task_data);

  csp_parser_reset(&csp_link->csp_parser);
  link->rx_task_data.cur_parser = &csp_link->csp_parser;
}

size_t csp_cla_forward_to_specific_parser(struct cla_link *link, const uint8_t *buffer,
                                          size_t length)
{
  struct rx_task_data *const rx_data = &link->rx_task_data;
  size_t result = 0;

  switch (rx_data->payload_type) {
  case PAYLOAD_UNKNOWN:
    result = select_bundle_parser_version(rx_data, buffer, length);
    if (result == 0)
      csp_cla_reset_parsers(link);
    break;
  case PAYLOAD_BUNDLE6:
    rx_data->cur_parser = rx_data->bundle6_parser.basedata;
    result = bundle6_parser_read(&rx_data->bundle6_parser, buffer, length);
    break;
  case PAYLOAD_BUNDLE7:
    rx_data->cur_parser = rx_data->bundle7_parser.basedata;
    result = bundle7_parser_read(&rx_data->bundle7_parser, buffer, length);
    break;
  default:
    csp_cla_reset_parsers(link);
    result = length;
    break;
  }

  return result;
}

enum ud3tn_result csp_cla_read(struct cla_link *link, uint8_t *buffer, size_t length,
                               size_t *bytes_read)
{
  struct csp_link *const csp_link = (struct csp_link *) link;

  *bytes_read = 0;

  /* Check if there's data in the RX buffer */
  if (csp_link->rx_buffer_pos >= csp_link->rx_buffer_len) {
    /* No data available, wait briefly */
    hal_task_delay(10);
    return UD3TN_OK;
  }

  /* Calculate how much data to return */
  size_t available = csp_link->rx_buffer_len - csp_link->rx_buffer_pos;
  size_t to_read = (length < available) ? length : available;

  memcpy(buffer, csp_link->rx_buffer + csp_link->rx_buffer_pos, to_read);
  csp_link->rx_buffer_pos += to_read;
  *bytes_read = to_read;

  /* Reset buffer if fully consumed */
  if (csp_link->rx_buffer_pos >= csp_link->rx_buffer_len) {
    csp_link->rx_buffer_len = 0;
    csp_link->rx_buffer_pos = 0;
  }

  /* Update last RX time */
  link->last_rx_time_ms = hal_time_get_timestamp_ms();

  return UD3TN_OK;
}

static void csp_cla_disconnect_handler(struct cla_link *link)
{
  LOGF_DEBUG("CSP: Disconnect handler called for %s",
             link->cla_addr ? link->cla_addr : "(unknown)");
  cla_generic_disconnect_handler(link);
}

/*===========================================================================*/
/* VTable Definition                                                          */
/*===========================================================================*/

static const struct cla_vtable csp_vtable = {
    .cla_name_get = csp_cla_name_get,

    .cla_launch = csp_cla_launch,
    .cla_terminate = csp_cla_terminate,

    .cla_mbs_get = csp_cla_mbs_get,
    .cla_get_tx_queue = csp_cla_get_tx_queue,

    .cla_start_scheduled_contact = csp_cla_start_scheduled_contact,
    .cla_end_scheduled_contact = csp_cla_end_scheduled_contact,

    .cla_begin_packet = csp_cla_begin_packet,
    .cla_end_packet = csp_cla_end_packet,
    .cla_send_packet_data = csp_cla_send_packet_data,

    .cla_rx_task_reset_parsers = csp_cla_reset_parsers,
    .cla_rx_task_forward_to_specific_parser = csp_cla_forward_to_specific_parser,

    .cla_read = csp_cla_read,

    .cla_disconnect_handler = csp_cla_disconnect_handler,
};

/*===========================================================================*/
/* Initialization                                                             */
/*===========================================================================*/

static enum ud3tn_result csp_cla_init(struct csp_cla_config *config, uint8_t local_addr,
                                      uint8_t csp_port,
                                      const struct bundle_agent_interface *bundle_agent_interface)
{
  /* Initialize base config */
  if (cla_config_init(&config->base, bundle_agent_interface) != UD3TN_OK)
    return UD3TN_FAIL;

  config->base.vtable = &csp_vtable;

  /* Initialize hash table semaphore */
  config->param_htab_sem = hal_semaphore_init_binary();
  if (!config->param_htab_sem) {
    cspcl_cleanup(&config->cspcl);
    return UD3TN_FAIL;
  }
  hal_semaphore_release(config->param_htab_sem);

  /* Initialize RX task semaphore */
  config->rx_task_sem = hal_semaphore_init_binary();
  if (!config->rx_task_sem) {
    hal_semaphore_delete(config->param_htab_sem);
    cspcl_cleanup(&config->cspcl);
    return UD3TN_FAIL;
  }

  /* Initialize hash table */
  htab_init(&config->param_htab, CSP_PARAM_HTAB_SLOT_COUNT, config->param_htab_elem);

  config->rx_running = false;

  LOGF_INFO("CSP: Initialized with local address %u, port %u", local_addr, csp_port);

  return UD3TN_OK;
}

struct cla_config *csp_cla_create(const char *const options[], const size_t option_count,
                                  const struct bundle_agent_interface *bundle_agent_interface)
{
  if (option_count < 2 || option_count > 3) {
    LOG_ERROR("CSP: Options format: <local_addr>,<csp_port>[,<iface>]");
    LOG_ERROR("CSP: Where <iface> is one of:");
    LOG_ERROR("CSP:   zmqhub           - ZMQHUB broker at localhost (default)");
    LOG_ERROR("CSP:   zmqhub:<host>    - ZMQHUB broker at <host>");
    LOG_ERROR("CSP:   can              - SocketCAN on vcan0 (default)");
    LOG_ERROR("CSP:   can:<iface>      - SocketCAN on <iface> (e.g. can0)");
    LOG_ERROR("CSP:   loopback         - local loopback");
    LOG_ERROR("CSP: Examples:");
    LOG_ERROR("CSP:   csp:1,10");
    LOG_ERROR("CSP:   csp:1,10,zmqhub");
    LOG_ERROR("CSP:   csp:1,10,zmqhub:192.168.1.10");
    LOG_ERROR("CSP:   csp:1,10,can");
    LOG_ERROR("CSP:   csp:1,10,can:can0");
    return NULL;
  }

  struct csp_cla_config *config = malloc(sizeof(struct csp_cla_config));
  if (!config) {
    LOG_ERROR("CSP: Memory allocation failed");
    return NULL;
  }
  memset(config, 0, sizeof(*config));
  config->cspcl.local_addr = (uint8_t) atoi(options[0]);
  config->cspcl.csp_port = (uint8_t) atoi(options[1]);

  config->cspcl.iface_type = CSP_IFACE_ZMQHUB;
  strncpy(config->cspcl.zmqhub_addr, CSPCL_ZMQHUB_ADDR_DEFAULT, CSPCL_IFACE_PARAM_MAX - 1);
  strncpy(config->cspcl.can_iface, CSPCL_CAN_IFACE_DEFAULT, CSPCL_IFACE_PARAM_MAX - 1);

  if (option_count >= 3) {
    const char *iface_spec = options[2];

    if (strncmp(iface_spec, "zmqhub:", 7) == 0) {
      config->cspcl.iface_type = CSP_IFACE_ZMQHUB;
      strncpy(config->cspcl.zmqhub_addr, iface_spec + 7, CSPCL_IFACE_PARAM_MAX - 1);
      if (config->cspcl.zmqhub_addr[0] == '\0') {
        LOG_ERROR("CSP: zmqhub:<host> — host must not be empty");
        free(config);
        return NULL;
      }
    } else if (strcmp(iface_spec, "zmqhub") == 0) {
      config->cspcl.iface_type = CSP_IFACE_ZMQHUB;
    } else if (strncmp(iface_spec, "can:", 4) == 0) {
      config->cspcl.iface_type = CSP_IFACE_CAN;
      strncpy(config->cspcl.can_iface, iface_spec + 4, CSPCL_IFACE_PARAM_MAX - 1);
      if (config->cspcl.can_iface[0] == '\0') {
        LOG_ERROR("CSP: can:<iface> — interface name must not be empty");
        free(config);
        return NULL;
      }
    } else if (strcmp(iface_spec, "can") == 0) {
      config->cspcl.iface_type = CSP_IFACE_CAN;
    } else if (strcmp(iface_spec, "loopback") == 0) {
      config->cspcl.iface_type = CSP_IFACE_LOOPBACK;
    } else {
      LOGF_ERROR("CSP: Unknown interface specifier '%s'", iface_spec);
      LOG_ERROR("CSP: Valid: zmqhub, zmqhub:<host>, can, can:<iface>, loopback");
      free(config);
      return NULL;
    }
  }

  LOGF_DEBUG("CSP: Creating CLA with local_addr=%u, csp_port=%u", config->cspcl.local_addr,
             config->cspcl.csp_port);
  switch (config->cspcl.iface_type) {
  case CSP_IFACE_ZMQHUB:
    LOGF_DEBUG("CSP:   iface=zmqhub, host='%s'", config->cspcl.zmqhub_addr);
    break;
  case CSP_IFACE_CAN:
    LOGF_DEBUG("CSP:   iface=can, iface='%s'", config->cspcl.can_iface);
    break;
  case CSP_IFACE_LOOPBACK:
    LOG_DEBUG("CSP:   iface=loopback");
    break;
  }

  if (csp_cla_init(config, config->cspcl.local_addr, config->cspcl.csp_port,
                   bundle_agent_interface) != UD3TN_OK) {
    free(config);
    LOG_ERROR("CSP: Initialization failed");
    return NULL;
  }

  cspcl_error_t ret = cspcl_init(&config->cspcl);
  if (ret != CSPCL_OK) {
    free(config);
    switch (ret) {
    case CSPCL_ERR_INVALID_PARAM:
      LOG_ERROR("CSP: Initialization failed: invalid parameter");
      break;
    case CSPCL_ERR_NO_MEMORY:
      LOG_ERROR("CSP: Initialization failed: memory allocation failed");
      break;
    case CSPCL_ERR_CSP_STACK_INIT:
      LOG_ERROR("CSP: Initialization failed: csp_init() failed");
      break;
    case CSPCL_ERR_CSP_ZMQHUB_INIT:
      LOGF_ERROR("CSP: Initialization failed: ZMQ hub interface init failed "
                 "(host: '%s')",
                 config->cspcl.zmqhub_addr);
      break;
    case CSPCL_ERR_CSP_CAN_INIT:
      LOGF_ERROR("CSP: Initialization failed: CAN interface init failed (iface: '%s')",
                 config->cspcl.can_iface);
      break;
    case CSPCL_ERR_CSP_CAN_NOT_SUPPORTED:
      LOGF_ERROR("CSP: Initialization failed: CAN support not compiled in "
                 "(iface: '%s')",
                 config->cspcl.can_iface);
      break;
    case CSPCL_ERR_CSP_ROUTER:
      LOG_ERROR("CSP: Initialization failed: CSP router task start failed");
      break;
    default:
      LOGF_ERROR("CSP: Initialization failed (error %d)", ret);
      break;
    }
    return NULL;
  }
  return &config->base;
}

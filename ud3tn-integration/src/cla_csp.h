// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
/**
 * @file cla_csp.h
 * @brief CSP Convergence Layer Adapter for uD3TN
 *
 * This CLA enables Bundle Protocol 7 (BP7) bundles to be transmitted over
 * CubeSat Space Protocol (CSP) using the CSPCL library.
 *
 * Architecture:
 *   uD3TN BP7 → CLA_CSP → CSPCL → CSP UDP → Physical (CAN/ZMQHUB)
 *
 * Configuration format:
 *   csp:<local_addr>,<csp_port>
 *   Example: csp:1,10
 *
 * CLA Address format:
 *   csp:<dest_addr>
 *   Example: csp:2
 *
 * @version 1.0
 */

#ifndef CLA_CSP_H
#define CLA_CSP_H

#include "cla/cla.h"

#include "ud3tn/bundle.h"
#include "ud3tn/bundle_processor.h"

#include <stddef.h>
#include <stdint.h>

/**
 * @brief Create a CSP CLA instance
 *
 * @param options       Array of configuration options [local_addr, csp_port]
 * @param option_count  Number of options (must be 2)
 * @param bundle_agent_interface  Bundle agent interface
 * @return Pointer to CLA config, or NULL on failure
 */
struct cla_config *csp_cla_create(
    const char *const options[],
    const size_t option_count,
    const struct bundle_agent_interface *bundle_agent_interface);

/* INTERNAL API */

struct csp_link;

/**
 * @brief Get maximum bundle size for CSP CLA
 */
size_t csp_cla_mbs_get(struct cla_config *const config);

/**
 * @brief Reset parsers for CSP link
 */
void csp_cla_reset_parsers(struct cla_link *link);

/**
 * @brief Forward data to bundle parser
 */
size_t csp_cla_forward_to_specific_parser(struct cla_link *link,
                                          const uint8_t *buffer,
                                          size_t length);

/**
 * @brief Begin sending a packet
 */
enum cla_begin_packet_result csp_cla_begin_packet(
    struct cla_link *link,
    const struct bundle *const bundle,
    size_t length,
    char *cla_addr);

/**
 * @brief End sending a packet
 */
enum ud3tn_result csp_cla_end_packet(struct cla_link *link);

/**
 * @brief Send packet data
 */
enum ud3tn_result csp_cla_send_packet_data(
    struct cla_link *link,
    const void *data,
    const size_t length);

/**
 * @brief Read data from CSP
 */
enum ud3tn_result csp_cla_read(struct cla_link *link,
                               uint8_t *buffer,
                               size_t length,
                               size_t *bytes_read);

#endif /* CLA_CSP_H */


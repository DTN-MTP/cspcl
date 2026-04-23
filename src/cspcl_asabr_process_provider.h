/**
 * @file cspcl_asabr_process_provider.h
 * @brief Process-based adapter for calling the A-SABR routing subprocess.
 */

#ifndef CSPCL_ASABR_PROCESS_PROVIDER_H
#define CSPCL_ASABR_PROCESS_PROVIDER_H

#include <stddef.h>

#include "cspcl_route_bridge.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  char *adapter_binary_path;
  char *contact_plan_path;
  cspcl_route_next_hop_t *next_hops;
  size_t next_hop_count;
  size_t next_hop_capacity;
  cspcl_route_mode_t mode;
  cspcl_route_decision_status_t decision_status;
  char diagnostic[CSPCL_ROUTE_DIAGNOSTIC_MAX_LEN];
  void *bridge_ctx;
} cspcl_asabr_process_provider_t;

cspcl_route_error_t
cspcl_asabr_process_provider_init(cspcl_asabr_process_provider_t *provider,
                                  const char *adapter_binary_path,
                                  const char *contact_plan_path);

void cspcl_asabr_process_provider_cleanup(
    cspcl_asabr_process_provider_t *provider);

cspcl_route_error_t
cspcl_asabr_process_provider_register(cspcl_route_bridge_t *bridge,
                                      cspcl_asabr_process_provider_t *provider);

cspcl_route_error_t cspcl_asabr_process_provider_unregister(
    cspcl_route_bridge_t *bridge, cspcl_asabr_process_provider_t *provider);

#ifdef __cplusplus
}
#endif

#endif /* CSPCL_ASABR_PROCESS_PROVIDER_H */
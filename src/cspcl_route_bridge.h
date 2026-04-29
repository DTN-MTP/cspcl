/**
 * @file cspcl_route_bridge.h
 * @brief BPA-agnostic routing bridge API for CSPCL.
 *
 * This API defines a stable request/response contract that allows
 * routing providers (e.g. A-SABR adapters) to be plugged into CSPCL
 * without introducing BPA-specific types.
 */

#ifndef CSPCL_ROUTE_BRIDGE_H
#define CSPCL_ROUTE_BRIDGE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifndef FREERTOS
#include <pthread.h>
#else
#include <FreeRTOS.h>
#include <semphr.h>
#endif

#ifdef __cplusplus
extern "C" {
#endif

/** Maximum length copied into diagnostic output buffers. */
#define CSPCL_ROUTE_DIAGNOSTIC_MAX_LEN 128

/** Route mode produced by a provider. */
typedef enum {
  CSPCL_ROUTE_MODE_NONE = 0,
  CSPCL_ROUTE_MODE_UNICAST,
  CSPCL_ROUTE_MODE_MULTICAST
} cspcl_route_mode_t;

/** Decision status returned by a route provider. */
typedef enum {
  CSPCL_ROUTE_DECISION_FOUND = 0,
  CSPCL_ROUTE_DECISION_NO_ROUTE,
  CSPCL_ROUTE_DECISION_PROVIDER_ERROR,
  CSPCL_ROUTE_DECISION_TIMEOUT
} cspcl_route_decision_status_t;

/**
 * Bridge API return codes.
 *
 * These describe bridge-level operation success/failure (validation,
 * lifecycle, provider registration/dispatch), not transport send status.
 */
typedef enum {
  CSPCL_ROUTE_OK = 0,
  CSPCL_ROUTE_ERR_INVALID_PARAM,
  CSPCL_ROUTE_ERR_NOT_INITIALIZED,
  CSPCL_ROUTE_ERR_ALREADY_INITIALIZED,
  CSPCL_ROUTE_ERR_NO_PROVIDER,
  CSPCL_ROUTE_ERR_PROVIDER_FAILED,
  CSPCL_ROUTE_ERR_NO_MEMORY,
  CSPCL_ROUTE_ERR_INTERNAL
} cspcl_route_error_t;

/** One next-hop selection returned by routing logic. */
typedef struct {
  uint16_t next_hop_node_id;
  uint64_t contact_identifier;
  double estimated_arrival_time;
} cspcl_route_next_hop_t;

/** Immutable route request input. Arrays are caller-owned borrows. */
typedef struct {
  uint16_t source_node_id;
  const uint16_t *destination_node_ids;
  size_t destination_count;
  int8_t bundle_priority;
  double bundle_size;
  double bundle_expiration;
  double current_time;
  const uint16_t *excluded_node_ids;
  size_t excluded_node_count;
  uint32_t timeout_ms;
} cspcl_route_request_t;

/**
 * Route result returned by bridge to caller.
 *
 * Memory ownership:
 * - `next_hops` is heap-owned by this struct after route() returns.
 * - Caller must release it via cspcl_route_result_cleanup().
 */
typedef struct {
  cspcl_route_decision_status_t decision_status;
  cspcl_route_mode_t mode;
  cspcl_route_next_hop_t *next_hops;
  size_t next_hop_count;
  char diagnostic[CSPCL_ROUTE_DIAGNOSTIC_MAX_LEN];
} cspcl_route_result_t;

/**
 * Lightweight provider output borrowed during callback execution.
 *
 * Memory ownership:
 * - `next_hops` and `diagnostic` are provider-owned borrows that need only
 *   remain valid for the duration of the callback.
 * - Bridge deep-copies these into cspcl_route_result_t before returning.
 */
typedef struct {
  cspcl_route_decision_status_t decision_status;
  cspcl_route_mode_t mode;
  const cspcl_route_next_hop_t *next_hops;
  size_t next_hop_count;
  const char *diagnostic;
} cspcl_route_provider_output_t;

/** Provider callback signature used by the route bridge. */
typedef cspcl_route_error_t (*cspcl_route_provider_cb_t)(
    const cspcl_route_request_t *request,
    cspcl_route_provider_output_t *output,
    void *user_ctx);

/** Mutable bridge runtime and provider registration state. */
typedef struct {
  bool initialized;
#ifndef FREERTOS
  pthread_mutex_t lock;
#else
  SemaphoreHandle_t lock;
#endif
  cspcl_route_provider_cb_t provider_cb;
  void *provider_user_ctx;
} cspcl_route_bridge_t;

/** Initialize a route bridge instance. */
cspcl_route_error_t cspcl_route_bridge_init(cspcl_route_bridge_t *bridge);

/** Cleanup a route bridge instance and clear provider registration. */
void cspcl_route_bridge_cleanup(cspcl_route_bridge_t *bridge);

/** Register or replace the route provider callback. */
cspcl_route_error_t cspcl_route_bridge_register_provider(
    cspcl_route_bridge_t *bridge,
    cspcl_route_provider_cb_t provider_cb,
    void *provider_user_ctx);

/** Unregister the route provider callback. */
cspcl_route_error_t
cspcl_route_bridge_unregister_provider(cspcl_route_bridge_t *bridge);

/**
 * Execute routing using the registered provider.
 *
 * On success, `result` is fully owned by caller and must be released with
 * cspcl_route_result_cleanup().
 */
cspcl_route_error_t cspcl_route_bridge_route(cspcl_route_bridge_t *bridge,
                                             const cspcl_route_request_t *request,
                                             cspcl_route_result_t *result);

/** Release heap-owned fields in cspcl_route_result_t. */
void cspcl_route_result_cleanup(cspcl_route_result_t *result);

/** Error string helper for bridge return codes. */
const char *cspcl_route_strerror(cspcl_route_error_t err);

#ifdef __cplusplus
}
#endif

#endif /* CSPCL_ROUTE_BRIDGE_H */

#include "cspcl_route_bridge.h"

#include <stdlib.h>
#include <string.h>

static int cspcl_route_lock(cspcl_route_bridge_t *bridge) {
#ifndef FREERTOS
  return pthread_mutex_lock(&bridge->lock);
#else
  return xSemaphoreTake(bridge->lock, portMAX_DELAY) == pdTRUE ? 0 : -1;
#endif
}

static int cspcl_route_unlock(cspcl_route_bridge_t *bridge) {
#ifndef FREERTOS
  return pthread_mutex_unlock(&bridge->lock);
#else
  return xSemaphoreGive(bridge->lock) == pdTRUE ? 0 : -1;
#endif
}

static bool cspcl_route_mode_valid(cspcl_route_mode_t mode) {
  return mode == CSPCL_ROUTE_MODE_NONE || mode == CSPCL_ROUTE_MODE_UNICAST ||
         mode == CSPCL_ROUTE_MODE_MULTICAST;
}

static bool cspcl_route_decision_valid(cspcl_route_decision_status_t status) {
  return status == CSPCL_ROUTE_DECISION_FOUND ||
         status == CSPCL_ROUTE_DECISION_NO_ROUTE ||
         status == CSPCL_ROUTE_DECISION_PROVIDER_ERROR ||
         status == CSPCL_ROUTE_DECISION_TIMEOUT;
}

static cspcl_route_error_t
cspcl_route_validate_request(const cspcl_route_request_t *request) {
  if (request == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  if (request->destination_count == 0 ||
      request->destination_node_ids == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  if (request->excluded_node_count > 0 && request->excluded_node_ids == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  return CSPCL_ROUTE_OK;
}

static cspcl_route_error_t cspcl_route_validate_provider_output(
    const cspcl_route_provider_output_t *provider_output) {
  if (provider_output == NULL) {
    return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
  }

  if (!cspcl_route_decision_valid(provider_output->decision_status)) {
    return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
  }

  if (!cspcl_route_mode_valid(provider_output->mode)) {
    return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
  }

  if (provider_output->decision_status == CSPCL_ROUTE_DECISION_FOUND) {
    if (provider_output->next_hops == NULL ||
        provider_output->next_hop_count == 0) {
      return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
    }
  } else {
    if (provider_output->next_hop_count != 0) {
      return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
    }
  }

  return CSPCL_ROUTE_OK;
}

cspcl_route_error_t cspcl_route_bridge_init(cspcl_route_bridge_t *bridge) {
  if (bridge == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  if (bridge->initialized) {
    return CSPCL_ROUTE_ERR_ALREADY_INITIALIZED;
  }

  memset(bridge, 0, sizeof(*bridge));

#ifndef FREERTOS
  if (pthread_mutex_init(&bridge->lock, NULL) != 0) {
    return CSPCL_ROUTE_ERR_NO_MEMORY;
  }
#else
  bridge->lock = xSemaphoreCreateMutex();
  if (bridge->lock == NULL) {
    return CSPCL_ROUTE_ERR_NO_MEMORY;
  }
#endif

  bridge->initialized = true;
  return CSPCL_ROUTE_OK;
}

void cspcl_route_bridge_cleanup(cspcl_route_bridge_t *bridge) {
  if (bridge == NULL || !bridge->initialized) {
    return;
  }

  if (cspcl_route_lock(bridge) != 0) {
    return;
  }

  bridge->provider_cb = NULL;
  bridge->provider_user_ctx = NULL;
  bridge->initialized = false;

  if (cspcl_route_unlock(bridge) != 0) {
    return;
  }

#ifndef FREERTOS
  pthread_mutex_destroy(&bridge->lock);
#else
  vSemaphoreDelete(bridge->lock);
  bridge->lock = NULL;
#endif
}

cspcl_route_error_t
cspcl_route_bridge_register_provider(cspcl_route_bridge_t *bridge,
                                     cspcl_route_provider_cb_t provider_cb,
                                     void *provider_user_ctx) {
  if (bridge == NULL || provider_cb == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  if (!bridge->initialized) {
    return CSPCL_ROUTE_ERR_NOT_INITIALIZED;
  }

  if (cspcl_route_lock(bridge) != 0) {
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  bridge->provider_cb = provider_cb;
  bridge->provider_user_ctx = provider_user_ctx;

  if (cspcl_route_unlock(bridge) != 0) {
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  return CSPCL_ROUTE_OK;
}

cspcl_route_error_t
cspcl_route_bridge_unregister_provider(cspcl_route_bridge_t *bridge) {
  if (bridge == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  if (!bridge->initialized) {
    return CSPCL_ROUTE_ERR_NOT_INITIALIZED;
  }

  if (cspcl_route_lock(bridge) != 0) {
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  bridge->provider_cb = NULL;
  bridge->provider_user_ctx = NULL;

  if (cspcl_route_unlock(bridge) != 0) {
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  return CSPCL_ROUTE_OK;
}

void cspcl_route_result_cleanup(cspcl_route_result_t *result) {
  if (result == NULL) {
    return;
  }

  if (result->next_hops != NULL) {
    free(result->next_hops);
  }

  memset(result, 0, sizeof(*result));
}

cspcl_route_error_t
cspcl_route_bridge_route(cspcl_route_bridge_t *bridge,
                         const cspcl_route_request_t *request,
                         cspcl_route_result_t *result) {
  cspcl_route_provider_cb_t provider_cb = NULL;
  void *provider_user_ctx = NULL;
  cspcl_route_provider_output_t provider_output;
  cspcl_route_error_t provider_rc;
  cspcl_route_error_t validate_rc;

  if (bridge == NULL || result == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  if (!bridge->initialized) {
    return CSPCL_ROUTE_ERR_NOT_INITIALIZED;
  }

  validate_rc = cspcl_route_validate_request(request);
  if (validate_rc != CSPCL_ROUTE_OK) {
    return validate_rc;
  }

  memset(result, 0, sizeof(*result));

  if (cspcl_route_lock(bridge) != 0) {
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  provider_cb = bridge->provider_cb;
  provider_user_ctx = bridge->provider_user_ctx;

  if (cspcl_route_unlock(bridge) != 0) {
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  if (provider_cb == NULL) {
    return CSPCL_ROUTE_ERR_NO_PROVIDER;
  }

  memset(&provider_output, 0, sizeof(provider_output));
  provider_rc = provider_cb(request, &provider_output, provider_user_ctx);
  if (provider_rc != CSPCL_ROUTE_OK) {
    return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
  }

  validate_rc = cspcl_route_validate_provider_output(&provider_output);
  if (validate_rc != CSPCL_ROUTE_OK) {
    return validate_rc;
  }

  result->decision_status = provider_output.decision_status;
  result->mode = provider_output.mode;
  result->next_hop_count = provider_output.next_hop_count;

  if (provider_output.next_hop_count > 0) {
    size_t bytes =
        provider_output.next_hop_count * sizeof(cspcl_route_next_hop_t);
    result->next_hops = (cspcl_route_next_hop_t *)malloc(bytes);
    if (result->next_hops == NULL) {
      cspcl_route_result_cleanup(result);
      return CSPCL_ROUTE_ERR_NO_MEMORY;
    }
    memcpy(result->next_hops, provider_output.next_hops, bytes);
  }

  if (provider_output.diagnostic != NULL) {
    strncpy(result->diagnostic, provider_output.diagnostic,
            CSPCL_ROUTE_DIAGNOSTIC_MAX_LEN - 1);
    result->diagnostic[CSPCL_ROUTE_DIAGNOSTIC_MAX_LEN - 1] = '\0';
  }

  return CSPCL_ROUTE_OK;
}

const char *cspcl_route_strerror(cspcl_route_error_t err) {
  switch (err) {
  case CSPCL_ROUTE_OK:
    return "Success";
  case CSPCL_ROUTE_ERR_INVALID_PARAM:
    return "Invalid parameter";
  case CSPCL_ROUTE_ERR_NOT_INITIALIZED:
    return "Route bridge not initialized";
  case CSPCL_ROUTE_ERR_ALREADY_INITIALIZED:
    return "Route bridge already initialized";
  case CSPCL_ROUTE_ERR_NO_PROVIDER:
    return "No route provider registered";
  case CSPCL_ROUTE_ERR_PROVIDER_FAILED:
    return "Route provider failed";
  case CSPCL_ROUTE_ERR_NO_MEMORY:
    return "Memory allocation failed";
  case CSPCL_ROUTE_ERR_INTERNAL:
    return "Internal route bridge error";
  default:
    return "Unknown route bridge error";
  }
}

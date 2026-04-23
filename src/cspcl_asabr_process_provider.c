#include "cspcl_asabr_process_provider.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef FREERTOS
#include <sys/wait.h>
#include <unistd.h>
#endif

typedef struct {
  cspcl_asabr_process_provider_t *provider;
} cspcl_asabr_process_provider_ctx_t;

static char *cspcl_strdup_safe(const char *value) {
  size_t length;
  char *copy;

  if (value == NULL || value[0] == '\0') {
    return NULL;
  }

  length = strlen(value);
  copy = (char *)malloc(length + 1);
  if (copy == NULL) {
    return NULL;
  }

  memcpy(copy, value, length + 1);
  return copy;
}

static const char *cspcl_pick_config_value(const char *explicit_value,
                                           const char *env_name) {
  const char *env_value;

  if (explicit_value != NULL && explicit_value[0] != '\0') {
    return explicit_value;
  }

  env_value = getenv(env_name);
  if (env_value != NULL && env_value[0] != '\0') {
    return env_value;
  }

  return NULL;
}

static void
cspcl_provider_reset_result(cspcl_asabr_process_provider_t *provider) {
  if (provider == NULL) {
    return;
  }

  provider->next_hop_count = 0;
  provider->mode = CSPCL_ROUTE_MODE_NONE;
  provider->decision_status = CSPCL_ROUTE_DECISION_PROVIDER_ERROR;
  provider->diagnostic[0] = '\0';
}

static cspcl_route_error_t
cspcl_provider_ensure_capacity(cspcl_asabr_process_provider_t *provider,
                               size_t required_count) {
  cspcl_route_next_hop_t *next_hops;
  size_t new_capacity;

  if (provider->next_hop_capacity >= required_count) {
    return CSPCL_ROUTE_OK;
  }

  new_capacity =
      provider->next_hop_capacity == 0 ? 4 : provider->next_hop_capacity;
  while (new_capacity < required_count) {
    new_capacity *= 2;
  }

  next_hops = (cspcl_route_next_hop_t *)realloc(
      provider->next_hops, new_capacity * sizeof(cspcl_route_next_hop_t));
  if (next_hops == NULL) {
    return CSPCL_ROUTE_ERR_NO_MEMORY;
  }

  provider->next_hops = next_hops;
  provider->next_hop_capacity = new_capacity;
  return CSPCL_ROUTE_OK;
}

static cspcl_route_error_t cspcl_provider_join_u16_list(const uint16_t *values,
                                                        size_t count,
                                                        char **out_text) {
  size_t buffer_size = 1;
  char *buffer;
  size_t offset = 0;

  if (values == NULL || count == 0) {
    *out_text = (char *)malloc(1);
    if (*out_text == NULL) {
      return CSPCL_ROUTE_ERR_NO_MEMORY;
    }
    (*out_text)[0] = '\0';
    return CSPCL_ROUTE_OK;
  }

  for (size_t i = 0; i < count; i++) {
    char temp[32];
    int written = snprintf(temp, sizeof(temp), "%u", (unsigned)values[i]);
    if (written < 0) {
      return CSPCL_ROUTE_ERR_INTERNAL;
    }
    buffer_size += (size_t)written + 1;
  }

  buffer = (char *)malloc(buffer_size);
  if (buffer == NULL) {
    return CSPCL_ROUTE_ERR_NO_MEMORY;
  }

  for (size_t i = 0; i < count; i++) {
    int written = snprintf(buffer + offset, buffer_size - offset, "%u",
                           (unsigned)values[i]);
    if (written < 0) {
      free(buffer);
      return CSPCL_ROUTE_ERR_INTERNAL;
    }
    offset += (size_t)written;
    if (i + 1 < count) {
      buffer[offset++] = ',';
      buffer[offset] = '\0';
    }
  }

  *out_text = buffer;
  return CSPCL_ROUTE_OK;
}

static cspcl_route_error_t
cspcl_provider_parse_line(cspcl_asabr_process_provider_t *provider,
                          const char *line) {
  if (strncmp(line, "STATUS=", 7) == 0) {
    const char *value = line + 7;
    if (strncmp(value, "FOUND", 5) == 0) {
      provider->decision_status = CSPCL_ROUTE_DECISION_FOUND;
    } else if (strncmp(value, "NO_ROUTE", 8) == 0) {
      provider->decision_status = CSPCL_ROUTE_DECISION_NO_ROUTE;
    } else if (strncmp(value, "PROVIDER_ERROR", 14) == 0) {
      provider->decision_status = CSPCL_ROUTE_DECISION_PROVIDER_ERROR;
    } else if (strncmp(value, "TIMEOUT", 7) == 0) {
      provider->decision_status = CSPCL_ROUTE_DECISION_TIMEOUT;
    } else {
      return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
    }
    return CSPCL_ROUTE_OK;
  }

  if (strncmp(line, "MODE=", 5) == 0) {
    const char *value = line + 5;
    if (strncmp(value, "NONE", 4) == 0) {
      provider->mode = CSPCL_ROUTE_MODE_NONE;
    } else if (strncmp(value, "UNICAST", 7) == 0) {
      provider->mode = CSPCL_ROUTE_MODE_UNICAST;
    } else if (strncmp(value, "MULTICAST", 9) == 0) {
      provider->mode = CSPCL_ROUTE_MODE_MULTICAST;
    } else {
      return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
    }
    return CSPCL_ROUTE_OK;
  }

  if (strncmp(line, "DIAG=", 5) == 0) {
    strncpy(provider->diagnostic, line + 5, CSPCL_ROUTE_DIAGNOSTIC_MAX_LEN - 1);
    provider->diagnostic[CSPCL_ROUTE_DIAGNOSTIC_MAX_LEN - 1] = '\0';
    return CSPCL_ROUTE_OK;
  }

  if (strncmp(line, "HOP=", 4) == 0) {
    unsigned int next_hop_node_id = 0;
    unsigned long long contact_identifier = 0;
    double estimated_arrival_time = 0.0;
    int parsed = sscanf(line + 4, "%u,%llu,%lf", &next_hop_node_id,
                        &contact_identifier, &estimated_arrival_time);
    cspcl_route_error_t ensure_rc;

    if (parsed != 3) {
      return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
    }

    ensure_rc =
        cspcl_provider_ensure_capacity(provider, provider->next_hop_count + 1);
    if (ensure_rc != CSPCL_ROUTE_OK) {
      return ensure_rc;
    }

    provider->next_hops[provider->next_hop_count].next_hop_node_id =
        (uint16_t)next_hop_node_id;
    provider->next_hops[provider->next_hop_count].contact_identifier =
        (uint64_t)contact_identifier;
    provider->next_hops[provider->next_hop_count].estimated_arrival_time =
        estimated_arrival_time;
    provider->next_hop_count++;
    return CSPCL_ROUTE_OK;
  }

  return CSPCL_ROUTE_OK;
}

static cspcl_route_error_t
cspcl_provider_run_process(cspcl_asabr_process_provider_t *provider,
                           const cspcl_route_request_t *request) {
#ifndef FREERTOS
  int pipe_fds[2];
  pid_t child_pid;
  char *destination_text = NULL;
  char *excluded_text = NULL;
  char timeout_text[32];
  char source_text[32];
  char priority_text[32];
  char size_text[64];
  char expiration_text[64];
  char current_time_text[64];
  char *argv[24];
  size_t argv_index = 0;
  FILE *stream = NULL;
  char *line = NULL;
  size_t line_capacity = 0;
  ssize_t line_length;
  int wait_status = 0;
  cspcl_route_error_t rc = CSPCL_ROUTE_OK;

  if (pipe(pipe_fds) != 0) {
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  rc = cspcl_provider_join_u16_list(request->destination_node_ids,
                                    request->destination_count,
                                    &destination_text);
  if (rc != CSPCL_ROUTE_OK) {
    close(pipe_fds[0]);
    close(pipe_fds[1]);
    return rc;
  }

  rc = cspcl_provider_join_u16_list(
      request->excluded_node_ids, request->excluded_node_count, &excluded_text);
  if (rc != CSPCL_ROUTE_OK) {
    free(destination_text);
    close(pipe_fds[0]);
    close(pipe_fds[1]);
    return rc;
  }

  if (snprintf(source_text, sizeof(source_text), "%u",
               (unsigned)request->source_node_id) < 0 ||
      snprintf(priority_text, sizeof(priority_text), "%d",
               (int)request->bundle_priority) < 0 ||
      snprintf(size_text, sizeof(size_text), "%.17g", request->bundle_size) <
          0 ||
      snprintf(expiration_text, sizeof(expiration_text), "%.17g",
               request->bundle_expiration) < 0 ||
      snprintf(current_time_text, sizeof(current_time_text), "%.17g",
               request->current_time) < 0 ||
      snprintf(timeout_text, sizeof(timeout_text), "%u", request->timeout_ms) <
          0) {
    free(destination_text);
    free(excluded_text);
    close(pipe_fds[0]);
    close(pipe_fds[1]);
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  child_pid = fork();
  if (child_pid < 0) {
    free(destination_text);
    free(excluded_text);
    close(pipe_fds[0]);
    close(pipe_fds[1]);
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  if (child_pid == 0) {
    dup2(pipe_fds[1], STDOUT_FILENO);
    dup2(pipe_fds[1], STDERR_FILENO);
    close(pipe_fds[0]);
    close(pipe_fds[1]);

    argv[argv_index++] = provider->adapter_binary_path;
    argv[argv_index++] = "query";
    argv[argv_index++] = "--cp";
    argv[argv_index++] = provider->contact_plan_path;
    argv[argv_index++] = "--source";
    argv[argv_index++] = source_text;
    argv[argv_index++] = "--dest";
    argv[argv_index++] = destination_text;
    argv[argv_index++] = "--priority";
    argv[argv_index++] = priority_text;
    argv[argv_index++] = "--size";
    argv[argv_index++] = size_text;
    argv[argv_index++] = "--expiration";
    argv[argv_index++] = expiration_text;
    argv[argv_index++] = "--current-time";
    argv[argv_index++] = current_time_text;
    argv[argv_index++] = "--timeout-ms";
    argv[argv_index++] = timeout_text;
    if (excluded_text[0] != '\0') {
      argv[argv_index++] = "--excluded";
      argv[argv_index++] = excluded_text;
    }
    argv[argv_index++] = NULL;

    execvp(provider->adapter_binary_path, argv);
    _exit(127);
  }

  close(pipe_fds[1]);
  stream = fdopen(pipe_fds[0], "r");
  if (stream == NULL) {
    free(destination_text);
    free(excluded_text);
    close(pipe_fds[0]);
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  cspcl_provider_reset_result(provider);

  while ((line_length = getline(&line, &line_capacity, stream)) != -1) {
    while (line_length > 0 &&
           (line[line_length - 1] == '\n' || line[line_length - 1] == '\r')) {
      line[--line_length] = '\0';
    }

    rc = cspcl_provider_parse_line(provider, line);
    if (rc != CSPCL_ROUTE_OK) {
      break;
    }
  }

  free(line);
  fclose(stream);
  free(destination_text);
  free(excluded_text);

  if (waitpid(child_pid, &wait_status, 0) < 0) {
    return CSPCL_ROUTE_ERR_INTERNAL;
  }

  if (rc != CSPCL_ROUTE_OK) {
    return rc;
  }

  if (!WIFEXITED(wait_status) || WEXITSTATUS(wait_status) != 0) {
    return CSPCL_ROUTE_ERR_PROVIDER_FAILED;
  }

  if (provider->diagnostic[0] == '\0') {
    strncpy(provider->diagnostic, "asabr-process-ok",
            CSPCL_ROUTE_DIAGNOSTIC_MAX_LEN - 1);
    provider->diagnostic[CSPCL_ROUTE_DIAGNOSTIC_MAX_LEN - 1] = '\0';
  }

  return CSPCL_ROUTE_OK;
#else
  (void)provider;
  (void)request;
  return CSPCL_ROUTE_ERR_INTERNAL;
#endif
}

static cspcl_route_error_t
cspcl_asabr_process_provider_cb(const cspcl_route_request_t *request,
                                cspcl_route_provider_output_t *output,
                                void *user_ctx) {
  cspcl_asabr_process_provider_ctx_t *ctx =
      (cspcl_asabr_process_provider_ctx_t *)user_ctx;
  cspcl_route_error_t rc;

  if (ctx == NULL || ctx->provider == NULL || output == NULL ||
      request == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  rc = cspcl_provider_run_process(ctx->provider, request);
  if (rc != CSPCL_ROUTE_OK) {
    return rc;
  }

  output->decision_status = ctx->provider->decision_status;
  output->mode = ctx->provider->mode;
  output->next_hops = ctx->provider->next_hops;
  output->next_hop_count = ctx->provider->next_hop_count;
  output->diagnostic = ctx->provider->diagnostic;

  return CSPCL_ROUTE_OK;
}

cspcl_route_error_t
cspcl_asabr_process_provider_init(cspcl_asabr_process_provider_t *provider,
                                  const char *adapter_binary_path,
                                  const char *contact_plan_path) {
  const char *selected_binary_path;
  const char *selected_contact_plan_path;

  if (provider == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  memset(provider, 0, sizeof(*provider));

  selected_binary_path =
      cspcl_pick_config_value(adapter_binary_path, "CSPCL_ASABR_ADAPTER_BIN");
  selected_contact_plan_path = cspcl_pick_config_value(
      contact_plan_path, "CSPCL_ASABR_CONTACT_PLAN_PATH");

  if (selected_binary_path == NULL || selected_contact_plan_path == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  provider->adapter_binary_path = cspcl_strdup_safe(selected_binary_path);
  provider->contact_plan_path = cspcl_strdup_safe(selected_contact_plan_path);
  if (provider->adapter_binary_path == NULL ||
      provider->contact_plan_path == NULL) {
    cspcl_asabr_process_provider_cleanup(provider);
    return CSPCL_ROUTE_ERR_NO_MEMORY;
  }

  provider->diagnostic[0] = '\0';
  provider->bridge_ctx = NULL;
  return CSPCL_ROUTE_OK;
}

void cspcl_asabr_process_provider_cleanup(
    cspcl_asabr_process_provider_t *provider) {
  if (provider == NULL) {
    return;
  }

  free(provider->adapter_binary_path);
  free(provider->contact_plan_path);
  free(provider->next_hops);
  memset(provider, 0, sizeof(*provider));
}

cspcl_route_error_t cspcl_asabr_process_provider_register(
    cspcl_route_bridge_t *bridge, cspcl_asabr_process_provider_t *provider) {
  cspcl_asabr_process_provider_ctx_t *ctx;
  cspcl_route_error_t rc;

  if (bridge == NULL || provider == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  ctx = (cspcl_asabr_process_provider_ctx_t *)malloc(sizeof(*ctx));
  if (ctx == NULL) {
    return CSPCL_ROUTE_ERR_NO_MEMORY;
  }

  ctx->provider = provider;
  provider->bridge_ctx = ctx;
  rc = cspcl_route_bridge_register_provider(
      bridge, cspcl_asabr_process_provider_cb, ctx);
  if (rc != CSPCL_ROUTE_OK) {
    provider->bridge_ctx = NULL;
    free(ctx);
    return rc;
  }

  return CSPCL_ROUTE_OK;
}

cspcl_route_error_t cspcl_asabr_process_provider_unregister(
    cspcl_route_bridge_t *bridge, cspcl_asabr_process_provider_t *provider) {
  cspcl_route_error_t rc;

  if (bridge == NULL || provider == NULL) {
    return CSPCL_ROUTE_ERR_INVALID_PARAM;
  }

  rc = cspcl_route_bridge_unregister_provider(bridge);
  free(provider->bridge_ctx);
  provider->bridge_ctx = NULL;
  return rc;
}
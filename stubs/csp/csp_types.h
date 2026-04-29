#ifndef CSP_CSP_TYPES_H
#define CSP_CSP_TYPES_H

#include <stdint.h>

typedef struct csp_iface_s {
  uint8_t addr;
  char name[16];
} csp_iface_t;

typedef struct {
  uint8_t address;
  const char *hostname;
  const char *model;
  const char *revision;
  uint16_t conn_max;
  uint16_t conn_queue_length;
  uint16_t fifo_length;
  uint8_t port_max_bind;
  uint16_t rdp_max_window;
  uint16_t buffers;
  uint16_t buffer_data_size;
} csp_conf_t;

#endif /* CSP_CSP_TYPES_H */
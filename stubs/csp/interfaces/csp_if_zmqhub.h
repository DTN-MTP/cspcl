#ifndef CSP_IF_ZMQHUB_H
#define CSP_IF_ZMQHUB_H

#include <stdint.h>

#include "../csp_types.h"

int csp_zmqhub_init(uint8_t addr, const char *server, uint16_t rx_filter, csp_iface_t **iface);

#endif /* CSP_IF_ZMQHUB_H */
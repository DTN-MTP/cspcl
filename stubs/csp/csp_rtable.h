#ifndef CSP_CSP_RTABLE_H
#define CSP_CSP_RTABLE_H

#include <stdint.h>

#include "csp_types.h"

#define CSP_DEFAULT_ROUTE 0
#define CSP_NODE_MAC 0

int csp_rtable_set(uint8_t addr, uint8_t netmask, csp_iface_t *ifc, uint16_t via);

#endif /* CSP_CSP_RTABLE_H */
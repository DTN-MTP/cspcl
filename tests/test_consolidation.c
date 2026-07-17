/**
 * @file test_consolidation.c
 * @brief Tests for CSPCL Consolidation (Phases 2-4)
 *
 * Tests for:
 * - Phase 2: Unified Address Parsing API
 * - Phase 3: Error Categorization API
 * - Phase 4: Interface Parsing API
 */

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cspcl.h"

/*===========================================================================*/
/* Test Utilities                                                             */
/*===========================================================================*/

#define TEST_PASS() printf("  [PASS] %s\n", __func__)
#define TEST_FAIL(msg)                                                                             \
  do {                                                                                             \
    printf("  [FAIL] %s: %s\n", __func__, msg);                                                    \
    return 1;                                                                                      \
  } while (0)

#define ASSERT_EQUAL(a, b)                                                                         \
  do {                                                                                             \
    if ((a) != (b)) {                                                                              \
      printf("  [FAIL] %s: expected %d, got %d at line %d\n", __func__, (int)(b), (int)(a), __LINE__); \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

#define ASSERT_STR_EQUAL(a, b)                                                                     \
  do {                                                                                             \
    if (strcmp((a), (b)) != 0) {                                                                   \
      printf("  [FAIL] %s: expected '%s', got '%s' at line %d\n", __func__, (b), (a), __LINE__);  \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

#define ASSERT(cond, msg)                                                                          \
  do {                                                                                             \
    if (!(cond)) {                                                                                 \
      printf("  [FAIL] %s: %s at line %d\n", __func__, msg, __LINE__);                             \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

/* ========================================================================== */
/* PHASE 2: Unified Address Parsing Tests                                    */
/* ========================================================================== */

static int test_parse_address_ipn(void)
{
  uint8_t port = 0;
  uint8_t addr = cspcl_parse_address("ipn:42.0", &port);
  ASSERT_EQUAL(addr, 42);
  ASSERT_EQUAL(port, CSPCL_PORT_BP);
  TEST_PASS();
  return 0;
}

static int test_parse_address_csp(void)
{
  uint8_t port = 0;
  uint8_t addr = cspcl_parse_address("csp:5", &port);
  ASSERT_EQUAL(addr, 5);
  ASSERT_EQUAL(port, CSPCL_PORT_BP);
  TEST_PASS();
  return 0;
}

static int test_parse_address_csp_with_port(void)
{
  uint8_t port = 0;
  uint8_t addr = cspcl_parse_address("csp:5,20", &port);
  ASSERT_EQUAL(addr, 5);
  ASSERT_EQUAL(port, 20);
  TEST_PASS();
  return 0;
}

static int test_parse_address_bare_integer(void)
{
  uint8_t port = 0;
  uint8_t addr = cspcl_parse_address("42", &port);
  ASSERT_EQUAL(addr, 42);
  ASSERT_EQUAL(port, CSPCL_PORT_BP);
  TEST_PASS();
  return 0;
}

static int test_parse_address_dtn_node(void)
{
  uint8_t port = 0;
  uint8_t addr = cspcl_parse_address("dtn://node7", &port);
  ASSERT_EQUAL(addr, 7);
  ASSERT_EQUAL(port, CSPCL_PORT_BP);
  TEST_PASS();
  return 0;
}

static int test_parse_address_zero(void)
{
  uint8_t addr = cspcl_parse_address("ipn:0.5", NULL);
  ASSERT_EQUAL(addr, 0);
  TEST_PASS();
  return 0;
}

static int test_is_valid_address_string_zero(void)
{
  uint8_t addr = cspcl_parse_address("0", NULL);
  bool valid = cspcl_is_valid_address_string("0", addr);
  ASSERT(valid, "Address '0' should be valid");
  TEST_PASS();
  return 0;
}

static int test_is_valid_address_string_invalid(void)
{
  uint8_t addr = cspcl_parse_address("xyz", NULL);
  bool valid = cspcl_is_valid_address_string("xyz", addr);
  ASSERT(!valid, "Address 'xyz' should be invalid");
  TEST_PASS();
  return 0;
}

static int test_parse_port_from_csp(void)
{
  uint8_t port = cspcl_parse_port("csp:10,25");
  ASSERT_EQUAL(port, 25);
  TEST_PASS();
  return 0;
}

static int test_parse_port_default(void)
{
  uint8_t port = cspcl_parse_port("ipn:10.5");
  ASSERT_EQUAL(port, CSPCL_PORT_BP);
  TEST_PASS();
  return 0;
}

static int test_identify_address_scheme_ipn(void)
{
  char scheme[16];
  cspcl_error_t err = cspcl_identify_address_scheme("ipn:1.5", scheme, sizeof(scheme));
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_STR_EQUAL(scheme, "ipn");
  TEST_PASS();
  return 0;
}

static int test_identify_address_scheme_csp(void)
{
  char scheme[16];
  cspcl_error_t err = cspcl_identify_address_scheme("csp:5", scheme, sizeof(scheme));
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_STR_EQUAL(scheme, "csp");
  TEST_PASS();
  return 0;
}

static int test_identify_address_scheme_bare(void)
{
  char scheme[16];
  cspcl_error_t err = cspcl_identify_address_scheme("42", scheme, sizeof(scheme));
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_STR_EQUAL(scheme, "bare");
  TEST_PASS();
  return 0;
}

/* ========================================================================== */
/* PHASE 3: Error Categorization Tests                                       */
/* ========================================================================== */

static int test_categorize_error_ok(void)
{
  cspcl_error_category_t cat = cspcl_categorize_error(CSPCL_OK);
  ASSERT_EQUAL(cat, CSPCL_ERRCATEGORY_OK);
  TEST_PASS();
  return 0;
}

static int test_categorize_error_param(void)
{
  cspcl_error_category_t cat = cspcl_categorize_error(CSPCL_ERR_INVALID_PARAM);
  ASSERT_EQUAL(cat, CSPCL_ERRCATEGORY_PARAM);
  TEST_PASS();
  return 0;
}

static int test_categorize_error_resource(void)
{
  cspcl_error_category_t cat = cspcl_categorize_error(CSPCL_ERR_NO_MEMORY);
  ASSERT_EQUAL(cat, CSPCL_ERRCATEGORY_RESOURCE);
  TEST_PASS();
  return 0;
}

static int test_categorize_error_timeout(void)
{
  cspcl_error_category_t cat = cspcl_categorize_error(CSPCL_ERR_TIMEOUT);
  ASSERT_EQUAL(cat, CSPCL_ERRCATEGORY_TIMEOUT);
  TEST_PASS();
  return 0;
}

static int test_categorize_error_csp(void)
{
  cspcl_error_category_t cat = cspcl_categorize_error(CSPCL_ERR_CSP_SEND);
  ASSERT_EQUAL(cat, CSPCL_ERRCATEGORY_CSP);
  TEST_PASS();
  return 0;
}

static int test_error_is_retryable_timeout(void)
{
  bool retryable = cspcl_error_is_retryable(CSPCL_ERR_TIMEOUT);
  ASSERT(retryable, "TIMEOUT should be retryable");
  TEST_PASS();
  return 0;
}

static int test_error_is_retryable_not(void)
{
  bool retryable = cspcl_error_is_retryable(CSPCL_ERR_INVALID_PARAM);
  ASSERT(!retryable, "INVALID_PARAM should not be retryable");
  TEST_PASS();
  return 0;
}

/* ========================================================================== */
/* PHASE 4: Interface Parsing Tests                                          */
/* ========================================================================== */

static int test_parse_interface_spec_zmqhub(void)
{
  cspcl_t cspcl = {.local_addr = 1};
  cspcl_error_t err = cspcl_parse_interface_spec("zmqhub", &cspcl);
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_EQUAL(cspcl.iface_type, CSP_IFACE_ZMQHUB);
  TEST_PASS();
  return 0;
}

static int test_parse_interface_spec_zmqhub_host(void)
{
  cspcl_t cspcl = {.local_addr = 1};
  cspcl_error_t err = cspcl_parse_interface_spec("zmqhub:192.168.1.100", &cspcl);
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_EQUAL(cspcl.iface_type, CSP_IFACE_ZMQHUB);
  ASSERT_STR_EQUAL(cspcl.zmqhub_addr, "192.168.1.100");
  TEST_PASS();
  return 0;
}

static int test_parse_interface_spec_can(void)
{
  cspcl_t cspcl = {.local_addr = 1};
  cspcl_error_t err = cspcl_parse_interface_spec("can", &cspcl);
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_EQUAL(cspcl.iface_type, CSP_IFACE_CAN);
  TEST_PASS();
  return 0;
}

static int test_parse_interface_spec_can_device(void)
{
  cspcl_t cspcl = {.local_addr = 1};
  cspcl_error_t err = cspcl_parse_interface_spec("can:can0", &cspcl);
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_EQUAL(cspcl.iface_type, CSP_IFACE_CAN);
  ASSERT_STR_EQUAL(cspcl.can_iface, "can0");
  TEST_PASS();
  return 0;
}

static int test_parse_interface_spec_loopback(void)
{
  cspcl_t cspcl = {.local_addr = 1};
  cspcl_error_t err = cspcl_parse_interface_spec("loopback", &cspcl);
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_EQUAL(cspcl.iface_type, CSP_IFACE_LOOPBACK);
  TEST_PASS();
  return 0;
}

static int test_interface_type_to_string_zmqhub(void)
{
  cspcl_t cspcl = {.iface_type = CSP_IFACE_ZMQHUB};
  strncpy(cspcl.zmqhub_addr, "localhost", sizeof(cspcl.zmqhub_addr) - 1);
  char buf[64];
  cspcl_error_t err = cspcl_interface_type_to_string(&cspcl, buf, sizeof(buf));
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_STR_EQUAL(buf, "zmqhub:localhost");
  TEST_PASS();
  return 0;
}

static int test_interface_type_to_string_can(void)
{
  cspcl_t cspcl = {.iface_type = CSP_IFACE_CAN};
  strncpy(cspcl.can_iface, "vcan0", sizeof(cspcl.can_iface) - 1);
  char buf[64];
  cspcl_error_t err = cspcl_interface_type_to_string(&cspcl, buf, sizeof(buf));
  ASSERT_EQUAL(err, CSPCL_OK);
  ASSERT_STR_EQUAL(buf, "can:vcan0");
  TEST_PASS();
  return 0;
}

/* ========================================================================== */
/* Test Registry and Main                                                     */
/* ========================================================================== */

struct {
  const char *name;
  int (*func)(void);
} tests[] = {
  /* Phase 2: Address Parsing */
  {"test_parse_address_ipn", test_parse_address_ipn},
  {"test_parse_address_csp", test_parse_address_csp},
  {"test_parse_address_csp_with_port", test_parse_address_csp_with_port},
  {"test_parse_address_bare_integer", test_parse_address_bare_integer},
  {"test_parse_address_dtn_node", test_parse_address_dtn_node},
  {"test_parse_address_zero", test_parse_address_zero},
  {"test_is_valid_address_string_zero", test_is_valid_address_string_zero},
  {"test_is_valid_address_string_invalid", test_is_valid_address_string_invalid},
  {"test_parse_port_from_csp", test_parse_port_from_csp},
  {"test_parse_port_default", test_parse_port_default},
  {"test_identify_address_scheme_ipn", test_identify_address_scheme_ipn},
  {"test_identify_address_scheme_csp", test_identify_address_scheme_csp},
  {"test_identify_address_scheme_bare", test_identify_address_scheme_bare},

  /* Phase 3: Error Categorization */
  {"test_categorize_error_ok", test_categorize_error_ok},
  {"test_categorize_error_param", test_categorize_error_param},
  {"test_categorize_error_resource", test_categorize_error_resource},
  {"test_categorize_error_timeout", test_categorize_error_timeout},
  {"test_categorize_error_csp", test_categorize_error_csp},
  {"test_error_is_retryable_timeout", test_error_is_retryable_timeout},
  {"test_error_is_retryable_not", test_error_is_retryable_not},

  /* Phase 4: Interface Parsing */
  {"test_parse_interface_spec_zmqhub", test_parse_interface_spec_zmqhub},
  {"test_parse_interface_spec_zmqhub_host", test_parse_interface_spec_zmqhub_host},
  {"test_parse_interface_spec_can", test_parse_interface_spec_can},
  {"test_parse_interface_spec_can_device", test_parse_interface_spec_can_device},
  {"test_parse_interface_spec_loopback", test_parse_interface_spec_loopback},
  {"test_interface_type_to_string_zmqhub", test_interface_type_to_string_zmqhub},
  {"test_interface_type_to_string_can", test_interface_type_to_string_can},

  {NULL, NULL}
};

int main(void)
{
  int passed = 0, failed = 0;

  printf("\n=== CSPCL Consolidation Tests (Phase 2-4) ===\n\n");

  for (int i = 0; tests[i].func; i++) {
    printf("Running: %s\n", tests[i].name);
    if (tests[i].func() == 0) {
      passed++;
    } else {
      failed++;
    }
  }

  printf("\n=== Results ===\n");
  printf("Total:  %d\n", passed + failed);
  printf("Passed: %d\n", passed);
  printf("Failed: %d\n", failed);

  return failed > 0 ? 1 : 0;
}

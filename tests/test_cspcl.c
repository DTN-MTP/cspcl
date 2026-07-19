/**
 * @file test_cspcl.c
 * @brief Unit tests for CSPCL
 */

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifndef FREERTOS
#include <pthread.h>
#endif

#include "cspcl.h"

/* Failure injection flag exposed by csp_stub.c (stubs build only) */
#ifdef USING_CSP_STUBS
extern int g_csp_sfp_send_fail;
extern int g_csp_rdp_wait_acked_result;
#endif

/*===========================================================================*/
/* Test Utilities                                                             */
/*===========================================================================*/

#define TEST_PASS() printf("  [PASS] %s\n", __func__)
#define TEST_FAIL(msg)                                                                             \
  do {                                                                                             \
    printf("  [FAIL] %s: %s\n", __func__, msg);                                                    \
    return 1;                                                                                      \
  } while (0)

#define ASSERT_EQ(a, b)                                                                            \
  do {                                                                                             \
    if ((a) != (b)) {                                                                              \
      printf("  [FAIL] %s: %s != %s at line %d\n", __func__, #a, #b, __LINE__);                    \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

#define ASSERT_NE(a, b)                                                                            \
  do {                                                                                             \
    if ((a) == (b)) {                                                                              \
      printf("  [FAIL] %s: %s == %s at line %d\n", __func__, #a, #b, __LINE__);                    \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

#define ASSERT_TRUE(cond)                                                                          \
  do {                                                                                             \
    if (!(cond)) {                                                                                 \
      printf("  [FAIL] %s: %s is false at line %d\n", __func__, #cond, __LINE__);                  \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

#ifndef FREERTOS
typedef struct {
  cspcl_t *cspcl;
  uint8_t *buf;
  size_t buf_len;
  uint32_t timeout_ms;
  cspcl_error_t err;
  size_t received_len;
} recv_ctx_t;

static void *recv_bundle_thread(void *arg)
{
  recv_ctx_t *ctx = (recv_ctx_t *) arg;
  size_t len = ctx->buf_len;
  ctx->err = cspcl_recv_bundle(ctx->cspcl, ctx->buf, &len, NULL, NULL, ctx->timeout_ms);
  ctx->received_len = len;
  return NULL;
}
#endif

/*===========================================================================*/
/* Test: Connection Pool                                                      */
/*===========================================================================*/

static int test_pool_init_valid(void)
{
  cspcl_conn_pool_t pool;
  cspcl_error_t err;

  err = cspcl_conn_pool_init(&pool);
  ASSERT_EQ(err, CSPCL_OK);
  ASSERT_TRUE(pool.initialized);

  cspcl_conn_pool_cleanup(&pool);
  ASSERT_TRUE(!pool.initialized);

  TEST_PASS();
  return 0;
}

static int test_pool_init_null(void)
{
  cspcl_error_t err = cspcl_conn_pool_init(NULL);
  ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

  TEST_PASS();
  return 0;
}

static int test_pool_cleanup_empty(void)
{
  cspcl_conn_pool_t pool;

  ASSERT_EQ(cspcl_conn_pool_init(&pool), CSPCL_OK);
  cspcl_conn_pool_cleanup(&pool); /* Must not crash on empty pool */
  ASSERT_TRUE(!pool.initialized);

  TEST_PASS();
  return 0;
}

static int test_pool_cleanup_uninitialized(void)
{
  cspcl_conn_pool_t pool = {0};

  cspcl_conn_pool_cleanup(&pool); /* Must be a no-op */
  cspcl_conn_pool_cleanup(NULL);  /* NULL must be safe */

  TEST_PASS();
  return 0;
}

static int test_pool_get_stats_empty(void)
{
  cspcl_conn_pool_t pool;
  cspcl_conn_pool_stats_t stats;

  ASSERT_EQ(cspcl_conn_pool_init(&pool), CSPCL_OK);

  cspcl_conn_pool_get_stats(&pool, &stats);
  ASSERT_EQ(stats.hits, (uint32_t) 0);
  ASSERT_EQ(stats.misses, (uint32_t) 0);
  ASSERT_EQ(stats.evictions, (uint32_t) 0);
  ASSERT_EQ(stats.connect_failures, (uint32_t) 0);
  ASSERT_EQ(stats.invalidations, (uint32_t) 0);

  cspcl_conn_pool_cleanup(&pool);

  TEST_PASS();
  return 0;
}

static int test_pool_get_stats_null(void)
{
  cspcl_conn_pool_stats_t stats;
  cspcl_conn_pool_stats_t zeroed = {0};

  /* NULL pool: stats output must remain unchanged */
  stats = zeroed;
  cspcl_conn_pool_get_stats(NULL, &stats);
  ASSERT_EQ(memcmp(&stats, &zeroed, sizeof(stats)), 0);

  /* NULL stats: must be a safe no-op */
  cspcl_conn_pool_t pool;
  ASSERT_EQ(cspcl_conn_pool_init(&pool), CSPCL_OK);
  cspcl_conn_pool_get_stats(&pool, NULL); /* Must not crash */
  cspcl_conn_pool_cleanup(&pool);

  TEST_PASS();
  return 0;
}

static int test_pool_double_cleanup(void)
{
  cspcl_conn_pool_t pool;

  ASSERT_EQ(cspcl_conn_pool_init(&pool), CSPCL_OK);
  cspcl_conn_pool_cleanup(&pool);
  cspcl_conn_pool_cleanup(&pool); /* Second call must be a no-op */

  TEST_PASS();
  return 0;
}

#ifndef FREERTOS
/* Concurrent cleanup stress: two threads call cleanup on the same pool.
 * Only one should proceed; neither should crash or double-free. */
static void *pool_cleanup_thread(void *arg)
{
  cspcl_conn_pool_t *pool = (cspcl_conn_pool_t *) arg;
  cspcl_conn_pool_cleanup(pool);
  return NULL;
}

static int test_pool_concurrent_cleanup(void)
{
  cspcl_conn_pool_t pool;
  ASSERT_EQ(cspcl_conn_pool_init(&pool), CSPCL_OK);

  pthread_t t1, t2;
  pthread_create(&t1, NULL, pool_cleanup_thread, &pool);
  pthread_create(&t2, NULL, pool_cleanup_thread, &pool);
  pthread_join(t1, NULL);
  pthread_join(t2, NULL);

  ASSERT_TRUE(!pool.initialized);

  TEST_PASS();
  return 0;
}
#endif /* FREERTOS */

/*===========================================================================*/
/* Test: Initialization                                                       */
/*===========================================================================*/

static int test_init_cleanup(void)
{
  cspcl_t cspcl = {0};
  cspcl_error_t err;

  /* Set required fields before init */
  cspcl.local_addr = 1;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  cspcl.csp_port = CSPCL_PORT_BP;

  /* Test init with valid parameters */
  err = cspcl_init(&cspcl);
  ASSERT_EQ(err, CSPCL_OK);
  ASSERT_TRUE(cspcl.initialized);
  ASSERT_EQ(cspcl.local_addr, 1);

  /* Cleanup */
  cspcl_cleanup(&cspcl);
  ASSERT_TRUE(!cspcl.initialized);

  TEST_PASS();
  return 0;
}

static int test_init_null_param(void)
{
  cspcl_error_t err;

  /* Test init with NULL parameter */
  err = cspcl_init(NULL);
  ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

  TEST_PASS();
  return 0;
}

/*===========================================================================*/
/* Test: Address Translation                                                  */
/*===========================================================================*/

static int test_endpoint_to_addr_ipn(void)
{
  uint8_t addr;

  /* Test IPN scheme */
  addr = cspcl_endpoint_to_addr("ipn:1.0");
  ASSERT_EQ(addr, 1);

  addr = cspcl_endpoint_to_addr("ipn:42.5");
  ASSERT_EQ(addr, 42);

  addr = cspcl_endpoint_to_addr("ipn:255.999");
  ASSERT_EQ(addr, 255);

  TEST_PASS();
  return 0;
}

static int test_endpoint_to_addr_dtn(void)
{
  uint8_t addr;

  /* Test DTN scheme */
  addr = cspcl_endpoint_to_addr("dtn://node5/app");
  ASSERT_EQ(addr, 5);

  addr = cspcl_endpoint_to_addr("dtn://node123/service");
  ASSERT_EQ(addr, 123);

  TEST_PASS();
  return 0;
}

static int test_endpoint_to_addr_invalid(void)
{
  uint8_t addr;

  /* Test invalid inputs */
  addr = cspcl_endpoint_to_addr(NULL);
  ASSERT_EQ(addr, 0);

  addr = cspcl_endpoint_to_addr("");
  ASSERT_EQ(addr, 0);

  addr = cspcl_endpoint_to_addr("invalid");
  ASSERT_EQ(addr, 0);

  addr = cspcl_endpoint_to_addr("ipn:");
  ASSERT_EQ(addr, 0);

  TEST_PASS();
  return 0;
}

static int test_addr_to_endpoint(void)
{
  char endpoint[32];
  cspcl_error_t err;

  /* Test address to endpoint conversion */
  err = cspcl_addr_to_endpoint(1, endpoint, sizeof(endpoint));
  ASSERT_EQ(err, CSPCL_OK);
  ASSERT_EQ(strcmp(endpoint, "ipn:1.0"), 0);

  err = cspcl_addr_to_endpoint(42, endpoint, sizeof(endpoint));
  ASSERT_EQ(err, CSPCL_OK);
  ASSERT_EQ(strcmp(endpoint, "ipn:42.0"), 0);

  err = cspcl_addr_to_endpoint(255, endpoint, sizeof(endpoint));
  ASSERT_EQ(err, CSPCL_OK);
  ASSERT_EQ(strcmp(endpoint, "ipn:255.0"), 0);

  TEST_PASS();
  return 0;
}

static int test_addr_to_endpoint_invalid(void)
{
  char endpoint[32];
  cspcl_error_t err;

  /* Test NULL buffer */
  err = cspcl_addr_to_endpoint(1, NULL, 32);
  ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

  /* Test buffer too small */
  err = cspcl_addr_to_endpoint(1, endpoint, 5);
  ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

  TEST_PASS();
  return 0;
}

/*===========================================================================*/
/* Test: Error Strings                                                        */
/*===========================================================================*/

static int test_error_strings(void)
{
  const char *str;

  str = cspcl_strerror(CSPCL_OK);
  ASSERT_NE(str, NULL);
  ASSERT_TRUE(strlen(str) > 0);

  str = cspcl_strerror(CSPCL_ERR_INVALID_PARAM);
  ASSERT_NE(str, NULL);
  ASSERT_TRUE(strlen(str) > 0);

  str = cspcl_strerror(CSPCL_ERR_NO_MEMORY);
  ASSERT_NE(str, NULL);

  str = cspcl_strerror(CSPCL_ERR_BUNDLE_TOO_LARGE);
  ASSERT_NE(str, NULL);

  /* Test unknown error */
  str = cspcl_strerror((cspcl_error_t) 999);
  ASSERT_NE(str, NULL);
  ASSERT_EQ(strcmp(str, "Unknown error"), 0);

  TEST_PASS();
  return 0;
}

/*===========================================================================*/
/* Test: Send Bundle (requires CSP stub)                                      */
/*===========================================================================*/

static int test_send_bundle_not_initialized(void)
{
  cspcl_t cspcl = {0};
  uint8_t bundle[] = {0x01, 0x02, 0x03};
  cspcl_error_t err;

  /* Test send without init */
  err = cspcl_send_bundle(&cspcl, bundle, sizeof(bundle), 2, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_ERR_NOT_INITIALIZED);

  TEST_PASS();
  return 0;
}

static int test_send_bundle_invalid_params(void)
{
  cspcl_t cspcl = {0};
  uint8_t bundle[] = {0x01, 0x02, 0x03};
  cspcl_error_t err;

  /* Test NULL cspcl — caught before initialization check */
  err = cspcl_send_bundle(NULL, bundle, sizeof(bundle), 2, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

  /* Test NULL bundle — caught before initialization check */
  err = cspcl_send_bundle(&cspcl, NULL, 10, 2, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

  /* Test zero length — caught before initialization check */
  err = cspcl_send_bundle(&cspcl, bundle, 0, 2, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

  TEST_PASS();
  return 0;
}

static int test_accept_conn_invalid_params(void)
{
  cspcl_t cspcl = {0};
  csp_conn_t *conn = NULL;
  uint8_t src_addr = 0;
  uint8_t src_port = 0;

  ASSERT_EQ(cspcl_accept_conn(NULL, &conn, &src_addr, &src_port, 1), CSPCL_ERR_INVALID_PARAM);
  ASSERT_EQ(cspcl_accept_conn(&cspcl, NULL, &src_addr, &src_port, 1), CSPCL_ERR_INVALID_PARAM);
  ASSERT_EQ(cspcl_accept_conn(&cspcl, &conn, NULL, &src_port, 1), CSPCL_ERR_INVALID_PARAM);
  ASSERT_EQ(cspcl_accept_conn(&cspcl, &conn, &src_addr, NULL, 1), CSPCL_ERR_INVALID_PARAM);

  ASSERT_EQ(cspcl_accept_conn(&cspcl, &conn, &src_addr, &src_port, 1), CSPCL_ERR_NOT_INITIALIZED);

  TEST_PASS();
  return 0;
}

static int test_recv_bundle_from_conn_invalid_params(void)
{
  uint8_t bundle[16] = {0};
  size_t len = sizeof(bundle);
  uint8_t src_addr = 0;
  uint8_t src_port = 0;

  ASSERT_EQ(cspcl_recv_bundle_from_conn(NULL, bundle, &len, &src_addr, &src_port, 1, 2),
            CSPCL_ERR_INVALID_PARAM);
  ASSERT_EQ(
      cspcl_recv_bundle_from_conn((csp_conn_t *) bundle, NULL, &len, &src_addr, &src_port, 1, 2),
      CSPCL_ERR_INVALID_PARAM);
  ASSERT_EQ(
      cspcl_recv_bundle_from_conn((csp_conn_t *) bundle, bundle, NULL, &src_addr, &src_port, 1, 2),
      CSPCL_ERR_INVALID_PARAM);

  TEST_PASS();
  return 0;
}

static int test_send_bundle_too_large(void)
{
  cspcl_t cspcl = {0};
  uint8_t bundle[100];
  cspcl_error_t err;

  cspcl.local_addr = 1;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  cspcl.csp_port = CSPCL_PORT_BP;
  ASSERT_EQ(cspcl_init(&cspcl), CSPCL_OK);

  /* Test bundle exceeding max size */
  err = cspcl_send_bundle(&cspcl, bundle, CSPCL_MAX_BUNDLE_SIZE + 1, 2, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_ERR_BUNDLE_TOO_LARGE);

  cspcl_cleanup(&cspcl);

  TEST_PASS();
  return 0;
}

static int test_send_small_bundle(void)
{
  cspcl_t cspcl = {0};
  uint8_t bundle[] = "Hello Bundle Protocol!";
  cspcl_error_t err;

  cspcl.local_addr = 1;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  cspcl.csp_port = CSPCL_PORT_BP;
  ASSERT_EQ(cspcl_init(&cspcl), CSPCL_OK);

#if !defined(FREERTOS) && !defined(USING_CSP_STUBS)
  uint8_t recv_buf[64] = {0};
  recv_ctx_t ctx = {
      .cspcl = &cspcl,
      .buf = recv_buf,
      .buf_len = sizeof(recv_buf),
      .timeout_ms = 2000,
      .err = CSPCL_ERR_TIMEOUT,
      .received_len = 0,
  };
  pthread_t rx_thread;
  ASSERT_EQ(pthread_create(&rx_thread, NULL, recv_bundle_thread, &ctx), 0);
#endif

  /* Send small bundle (no fragmentation needed) */
  err = cspcl_send_bundle(&cspcl, bundle, sizeof(bundle), cspcl.local_addr, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_OK);

#if !defined(FREERTOS) && !defined(USING_CSP_STUBS)
  ASSERT_EQ(pthread_join(rx_thread, NULL), 0);
  ASSERT_EQ(ctx.err, CSPCL_OK);
  ASSERT_EQ(ctx.received_len, sizeof(bundle));
  ASSERT_TRUE(memcmp(recv_buf, bundle, sizeof(bundle)) == 0);
#endif

  cspcl_cleanup(&cspcl);

  TEST_PASS();
  return 0;
}

#ifdef USING_CSP_STUBS
static int test_send_bundle_ack_timeout(void)
{
  cspcl_t cspcl = {0};
  uint8_t bundle[] = "Hello Bundle Protocol!";
  cspcl_error_t err;

  cspcl.local_addr = 1;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  cspcl.csp_port = CSPCL_PORT_BP;
  ASSERT_EQ(cspcl_init(&cspcl), CSPCL_OK);

  /* SFP send succeeds (data queued in the RDP window) but the peer never
   * acknowledges it: the send must be reported as failed, not OK. */
  g_csp_rdp_wait_acked_result = CSP_ERR_TIMEDOUT;
  err = cspcl_send_bundle(&cspcl, bundle, sizeof(bundle), 2, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_ERR_TIMEOUT);

  /* Connection closed while waiting for the acknowledgements */
  g_csp_rdp_wait_acked_result = CSP_ERR_RESET;
  err = cspcl_send_bundle(&cspcl, bundle, sizeof(bundle), 2, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_ERR_CSP_SEND);

  /* Acks arrive again: sends succeed again */
  g_csp_rdp_wait_acked_result = CSP_ERR_NONE;
  err = cspcl_send_bundle(&cspcl, bundle, sizeof(bundle), 2, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_OK);

  cspcl_cleanup(&cspcl);

  TEST_PASS();
  return 0;
}
#endif

static int test_send_large_bundle(void)
{
  cspcl_t cspcl = {0};
  cspcl_error_t err;

  /* Create bundle larger than MTU to test fragmentation */
  size_t bundle_size = CSPCL_MAX_PAYLOAD * 3 + 50; /* 3+ fragments */
  uint8_t *bundle = (uint8_t *) malloc(bundle_size);
  ASSERT_NE(bundle, NULL);

  /* Fill with pattern */
  for (size_t i = 0; i < bundle_size; i++) {
    bundle[i] = (uint8_t) (i & 0xFF);
  }

  cspcl.local_addr = 1;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  cspcl.csp_port = CSPCL_PORT_BP;
  ASSERT_EQ(cspcl_init(&cspcl), CSPCL_OK);

#if !defined(FREERTOS) && !defined(USING_CSP_STUBS)
  uint8_t *recv_buf = (uint8_t *) malloc(bundle_size);
  ASSERT_NE(recv_buf, NULL);
  recv_ctx_t ctx = {
      .cspcl = &cspcl,
      .buf = recv_buf,
      .buf_len = bundle_size,
      .timeout_ms = 5000,
      .err = CSPCL_ERR_TIMEOUT,
      .received_len = 0,
  };
  pthread_t rx_thread;
  ASSERT_EQ(pthread_create(&rx_thread, NULL, recv_bundle_thread, &ctx), 0);
#endif

  /* Send large bundle (fragmentation needed) */
  err = cspcl_send_bundle(&cspcl, bundle, bundle_size, cspcl.local_addr, CSPCL_PORT_BP);
  ASSERT_EQ(err, CSPCL_OK);

#if !defined(FREERTOS) && !defined(USING_CSP_STUBS)
  ASSERT_EQ(pthread_join(rx_thread, NULL), 0);
  ASSERT_EQ(ctx.err, CSPCL_OK);
  ASSERT_EQ(ctx.received_len, bundle_size);
  ASSERT_TRUE(memcmp(recv_buf, bundle, bundle_size) == 0);
  free(recv_buf);
#endif

  free(bundle);
  cspcl_cleanup(&cspcl);

  TEST_PASS();
  return 0;
}

/*===========================================================================*/
/* Test: Constants                                                            */
/*===========================================================================*/

static int test_constants(void)
{
  /* Verify critical constants are reasonable */
  ASSERT_TRUE(CSPCL_PORT_BP == 10);
  ASSERT_TRUE(CSPCL_CSP_MTU >= 64);
  ASSERT_TRUE(CSPCL_CSP_MTU <= 4096);
  ASSERT_TRUE(CSPCL_SFP_HEADER_SIZE == 8);
  ASSERT_TRUE(CSPCL_MAX_PAYLOAD > 0);
  ASSERT_TRUE(CSPCL_MAX_PAYLOAD < CSPCL_CSP_MTU);
  ASSERT_TRUE(CSPCL_MAX_BUNDLE_SIZE >= 1024);

  TEST_PASS();
  return 0;
}

/*===========================================================================*/
/* Main Test Runner                                                           */
/*===========================================================================*/

typedef int (*test_func_t)(void);

typedef struct {
  const char *name;
  test_func_t func;
} test_case_t;

static test_case_t tests[] = {
    /* Pool unit tests (no CSP required) */
    {"test_pool_init_valid", test_pool_init_valid},
    {"test_pool_init_null", test_pool_init_null},
    {"test_pool_cleanup_empty", test_pool_cleanup_empty},
    {"test_pool_cleanup_uninitialized", test_pool_cleanup_uninitialized},
    {"test_pool_get_stats_empty", test_pool_get_stats_empty},
    {"test_pool_get_stats_null", test_pool_get_stats_null},
    {"test_pool_double_cleanup", test_pool_double_cleanup},
#ifndef FREERTOS
    {"test_pool_concurrent_cleanup", test_pool_concurrent_cleanup},
#endif

    /* Initialization tests */
    {"test_init_cleanup", test_init_cleanup},
    {"test_init_null_param", test_init_null_param},

    /* Address translation tests */
    {"test_endpoint_to_addr_ipn", test_endpoint_to_addr_ipn},
    {"test_endpoint_to_addr_dtn", test_endpoint_to_addr_dtn},
    {"test_endpoint_to_addr_invalid", test_endpoint_to_addr_invalid},
    {"test_addr_to_endpoint", test_addr_to_endpoint},
    {"test_addr_to_endpoint_invalid", test_addr_to_endpoint_invalid},

    /* Error string tests */
    {"test_error_strings", test_error_strings},

    /* Send tests */
    {"test_send_bundle_not_initialized", test_send_bundle_not_initialized},
    {"test_send_bundle_invalid_params", test_send_bundle_invalid_params},
    {"test_accept_conn_invalid_params", test_accept_conn_invalid_params},
    {"test_recv_bundle_from_conn_invalid_params", test_recv_bundle_from_conn_invalid_params},
    {"test_send_bundle_too_large", test_send_bundle_too_large},
    {"test_send_small_bundle", test_send_small_bundle},
#ifdef USING_CSP_STUBS
    {"test_send_bundle_ack_timeout", test_send_bundle_ack_timeout},
#endif
    {"test_send_large_bundle", test_send_large_bundle},

    /* Constant tests */
    {"test_constants", test_constants},

    {NULL, NULL}};

int main(int argc, char *argv[])
{
  (void) argc;
  (void) argv;

  printf("\n=== CSPCL Unit Tests ===\n\n");

  int total = 0;
  int passed = 0;
  int failed = 0;

  for (int i = 0; tests[i].name != NULL; i++) {
    total++;
    printf("Running: %s\n", tests[i].name);

    if (tests[i].func() == 0) {
      passed++;
    } else {
      failed++;
    }
  }

  printf("\n=== Results ===\n");
  printf("Total:  %d\n", total);
  printf("Passed: %d\n", passed);
  printf("Failed: %d\n", failed);
  printf("\n");

  return failed > 0 ? 1 : 0;
}

/**
 * @file test_cspcl_pool_integration.c
 * @brief Integration tests for connection pool LRU eviction and cache behavior.
 *
 * Uses a single cspcl_t instance initialized once (csp_init is a singleton).
 * Direct pool struct manipulation fills the pool without requiring sends to
 * succeed.
 */

#include "cspcl.h"
#include "cspcl_config.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifndef FREERTOS
#include <pthread.h>
#endif

#include <csp/csp.h>

#ifndef CSP_ANY
#define CSP_ANY 255
#endif

/*===========================================================================*/
/* Test Utilities                                                             */
/*===========================================================================*/

#define TEST_PASS()                                                                                \
  do {                                                                                             \
    printf("  [PASS] %s\n", __func__);                                                             \
    return 0;                                                                                      \
  } while (0)

#define TEST_FAIL(msg)                                                                             \
  do {                                                                                             \
    printf("  [FAIL] %s: %s\n", __func__, msg);                                                    \
    return 1;                                                                                      \
  } while (0)

#define ASSERT_EQ(a, b)                                                                            \
  do {                                                                                             \
    if ((a) != (b)) {                                                                              \
      printf("  [FAIL] %s: " #a " (%d) != " #b " (%d) at line %d\n", __func__, (int) (a),          \
             (int) (b), __LINE__);                                                                 \
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

/* Global instance — initialized exactly once in main(). */
static cspcl_t g_cspcl = {0};

#ifndef FREERTOS
static csp_socket_t *g_discard_socket = NULL;
static pthread_t g_discard_thread;
static volatile bool g_discard_running = false;

static void *discard_thread(void *arg)
{
  (void) arg;
  while (g_discard_running) {
    csp_conn_t *conn = csp_accept(g_discard_socket, 100);
    if (conn == NULL) {
      continue;
    }

    void *data = NULL;
    int datasize = 0;
    (void) csp_sfp_recv(conn, &data, &datasize, CSPCL_SFP_TIMEOUT_MS);

    if (data != NULL) {
      csp_free(data);
    }
    csp_close(conn);
  }
  return NULL;
}

static int start_discard_server(void)
{
  g_discard_socket = csp_socket(CSPCL_CSP_SOCKET_OPTIONS);
  if (g_discard_socket == NULL) {
    return -1;
  }
  if (csp_bind(g_discard_socket, CSP_ANY) != CSP_ERR_NONE) {
    return -1;
  }
  if (csp_listen(g_discard_socket, 5) != CSP_ERR_NONE) {
    return -1;
  }

  g_discard_running = true;
  if (pthread_create(&g_discard_thread, NULL, discard_thread, NULL) != 0) {
    g_discard_running = false;
    return -1;
  }
  return 0;
}

static void stop_discard_server(void)
{
  if (!g_discard_running) {
    return;
  }
  g_discard_running = false;
  (void) pthread_join(g_discard_thread, NULL);
}
#endif

/* Reset pool state between tests without destroying the mutex or
 * cspcl.initialized. */
static void reset_pool(void)
{
  memset(g_cspcl.conn_pool.entries, 0, sizeof(g_cspcl.conn_pool.entries));
  memset(&g_cspcl.conn_pool.stats, 0, sizeof(g_cspcl.conn_pool.stats));
  g_cspcl.conn_pool.tick = 0;
}

/*===========================================================================*/
/* Test: cache miss / no eviction when a free slot is available              */
/*===========================================================================*/

static int test_pool_no_eviction_when_free_slot(void)
{
  reset_pool();

  uint8_t bundle[] = {0x9f, 0x00}; /* minimal stub payload */
  cspcl_send_bundle(&g_cspcl, bundle, sizeof(bundle), 1, 20);

  cspcl_conn_pool_stats_t stats;
  cspcl_conn_pool_get_stats(&g_cspcl.conn_pool, &stats);

  ASSERT_EQ(stats.evictions, 0u);
  ASSERT_EQ(stats.misses, 1u);

  TEST_PASS();
}

/*===========================================================================*/
/* Test: LRU eviction when the pool is full                                  */
/*===========================================================================*/

static int test_pool_lru_eviction(void)
{
  reset_pool();

  /* Fill all 16 slots with real CSP connections via direct struct writes.
   * last_used values 1..16: slot 0 → LRU (1), slot 15 → MRU (16). */
  csp_conn_t *conns[CSPCL_CONN_POOL_SIZE];
  int i;
  for (i = 0; i < CSPCL_CONN_POOL_SIZE; i++) {
    conns[i] = csp_connect(CSP_PRIO_NORM, 1, (uint8_t) (20 + i), CSPCL_CSP_TIMEOUT_MS, CSP_O_NONE);
    if (conns[i] == NULL) {
      printf("  [SKIP] %s: csp_connect returned NULL for port %d\n", __func__, 20 + i);
      /* Close any already-opened connections. */
      for (int j = 0; j < i; j++) {
        if (conns[j])
          csp_close(conns[j]);
      }
      return 0;
    }
    g_cspcl.conn_pool.entries[i].used = true;
    g_cspcl.conn_pool.entries[i].dest_addr = 1;
    g_cspcl.conn_pool.entries[i].dest_port = (uint8_t) (20 + i);
    g_cspcl.conn_pool.entries[i].conn = conns[i];
    g_cspcl.conn_pool.entries[i].last_used = (uint32_t) (i + 1);
  }
  g_cspcl.conn_pool.tick = CSPCL_CONN_POOL_SIZE;
  memset(&g_cspcl.conn_pool.stats, 0, sizeof(g_cspcl.conn_pool.stats));

  /* Bump slot 0 (last_used=1, was LRU) → MRU; slot 1 (last_used=2) is now
   * the LRU. */
  g_cspcl.conn_pool.entries[0].last_used = 100;

  /* Send to a new destination not in the pool — should evict the LRU (slot 1,
   * dest_port=21) and increment evictions. */
  uint8_t bundle[] = {0x9f, 0x00};
  cspcl_send_bundle(&g_cspcl, bundle, sizeof(bundle), 1, 50);

  cspcl_conn_pool_stats_t stats;
  cspcl_conn_pool_get_stats(&g_cspcl.conn_pool, &stats);

  ASSERT_EQ(stats.evictions, 1u);

  /* Slot 1 (dest_port=21) must have been evicted — not present as used. */
  bool slot1_found = false;
  for (i = 0; i < CSPCL_CONN_POOL_SIZE; i++) {
    if (g_cspcl.conn_pool.entries[i].used && g_cspcl.conn_pool.entries[i].dest_port == 21) {
      slot1_found = true;
      break;
    }
  }
  ASSERT_TRUE(!slot1_found);

  /* Slot 0 (dest_port=20) must still be present — it was the MRU. */
  bool slot0_found = false;
  for (i = 0; i < CSPCL_CONN_POOL_SIZE; i++) {
    if (g_cspcl.conn_pool.entries[i].used && g_cspcl.conn_pool.entries[i].dest_port == 20) {
      slot0_found = true;
      break;
    }
  }
  ASSERT_TRUE(slot0_found);

  TEST_PASS();
}

/*===========================================================================*/
/* Test: cache hit increments hit counter                                    */
/*===========================================================================*/

static int test_pool_cache_stats(void)
{
  reset_pool();

  /* Obtain a real connection to populate one slot. */
  csp_conn_t *conn = csp_connect(CSP_PRIO_NORM, 1, 20, CSPCL_CSP_TIMEOUT_MS, CSP_O_NONE);
  if (conn == NULL) {
    printf("  [SKIP] %s: csp_connect returned NULL\n", __func__);
    return 0;
  }

  g_cspcl.conn_pool.entries[0].used = true;
  g_cspcl.conn_pool.entries[0].dest_addr = 1;
  g_cspcl.conn_pool.entries[0].dest_port = 20;
  g_cspcl.conn_pool.entries[0].conn = conn;
  g_cspcl.conn_pool.entries[0].last_used = 1;
  g_cspcl.conn_pool.tick = 1;
  memset(&g_cspcl.conn_pool.stats, 0, sizeof(g_cspcl.conn_pool.stats));

  /* Send to the same (addr=1, port=20) — should be a cache hit. */
  uint8_t bundle[] = {0x9f, 0x00};
  cspcl_send_bundle(&g_cspcl, bundle, sizeof(bundle), 1, 20);

  cspcl_conn_pool_stats_t stats;
  cspcl_conn_pool_get_stats(&g_cspcl.conn_pool, &stats);

  /* We expect at least one hit (the send may also fail and invalidate, but
   * the hit must have been counted before any failure). */
  ASSERT_TRUE(stats.hits >= 1u);
  ASSERT_EQ(stats.misses, 0u);

  TEST_PASS();
}

/*===========================================================================*/
/* main                                                                      */
/*===========================================================================*/

int main(void)
{
  printf("=== CSPCL Pool Integration Tests ===\n");

  /* Initialize CSP + cspcl exactly once. */
  g_cspcl.local_addr = 1;
  g_cspcl.iface_type = CSP_IFACE_LOOPBACK;
  g_cspcl.csp_port = CSPCL_PORT_BP;

  cspcl_error_t err = cspcl_init(&g_cspcl);
  if (err != CSPCL_OK) {
    printf("SKIP: cspcl_init failed: %s\n", cspcl_strerror(err));
    return 77;
  }

#ifndef FREERTOS
  if (start_discard_server() != 0) {
    printf("SKIP: failed to start discard server\n");
    cspcl_cleanup(&g_cspcl);
    return 77;
  }
#endif

  int failures = 0;

  failures += test_pool_no_eviction_when_free_slot();
  failures += test_pool_lru_eviction();
  failures += test_pool_cache_stats();

  if (failures == 0)
    printf("All pool integration tests passed.\n");
  else
    printf("%d pool integration test(s) FAILED.\n", failures);

#ifndef FREERTOS
  stop_discard_server();
#endif
  cspcl_cleanup(&g_cspcl);
  return failures;
}

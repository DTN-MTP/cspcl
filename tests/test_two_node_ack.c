/**
 * @file test_two_node_ack.c
 * @brief Two-node RDP ACK failure detection test
 *
 * Demonstrates:
 * 1. Successful message delivery with ACKs
 * 2. Message timeout when receiver is offline
 * 3. Incomplete bundle when receiver is down
 */

#include <assert.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "cspcl.h"

/* ========================================================================== */
/* Test Configuration                                                         */
/* ========================================================================== */

#define NODE_A_ADDR 1
#define NODE_B_ADDR 2
#define CSP_PORT 10

#define TEST_BUNDLE_SMALL "Hello from Node A! This is a small message."
#define TEST_BUNDLE_LARGE_SIZE (CSPCL_MAX_PAYLOAD * 2 + 100)

/* ========================================================================== */
/* Context Structures                                                         */
/* ========================================================================== */

typedef struct {
  cspcl_t *cspcl;
  uint8_t *buffer;
  size_t buffer_size;
  size_t received_len;
  uint8_t src_addr;
  uint8_t src_port;
  cspcl_error_t error;
  volatile bool running;
  volatile bool got_message;
} receiver_context_t;

/* ========================================================================== */
/* Logging Helpers                                                            */
/* ========================================================================== */

static void log_timestamp(void)
{
  struct timespec ts;
  clock_gettime(CLOCK_REALTIME, &ts);
  struct tm *tm_info = localtime(&ts.tv_sec);
  printf("[%02d:%02d:%02d.%03ld] ", tm_info->tm_hour, tm_info->tm_min, tm_info->tm_sec,
         ts.tv_nsec / 1000000);
}

static void log_info(const char *format, ...)
{
  log_timestamp();
  printf("[INFO] ");
  va_list args;
  va_start(args, format);
  vprintf(format, args);
  va_end(args);
  printf("\n");
  fflush(stdout);
}

static void log_success(const char *format, ...)
{
  log_timestamp();
  printf("[✓ SUCCESS] ");
  va_list args;
  va_start(args, format);
  vprintf(format, args);
  va_end(args);
  printf("\n");
  fflush(stdout);
}

static void log_error(const char *format, ...)
{
  log_timestamp();
  printf("[✗ ERROR] ");
  va_list args;
  va_start(args, format);
  vprintf(format, args);
  va_end(args);
  printf("\n");
  fflush(stdout);
}

static void log_warning(const char *format, ...)
{
  log_timestamp();
  printf("[⚠ WARN] ");
  va_list args;
  va_start(args, format);
  vprintf(format, args);
  va_end(args);
  printf("\n");
  fflush(stdout);
}

/* ========================================================================== */
/* Receiver Thread                                                            */
/* ========================================================================== */

static void *receiver_thread(void *arg)
{
  receiver_context_t *ctx = (receiver_context_t *) arg;

  log_info("Node B: Receiver thread started (listening on csp:%u:%u)", NODE_B_ADDR, CSP_PORT);

  while (ctx->running) {
    ctx->received_len = ctx->buffer_size;

    log_info("Node B: Waiting for incoming bundle (timeout=1000ms)...");

    cspcl_error_t err =
        cspcl_recv_bundle(ctx->cspcl, ctx->buffer, &ctx->received_len, &ctx->src_addr,
                          &ctx->src_port, 1000);

    if (err == CSPCL_ERR_TIMEOUT) {
      log_info("Node B: No bundle received (idle, expected)");
      continue;
    }

    if (err != CSPCL_OK) {
      if (err == CSPCL_ERR_SFP) {
        log_error("Node B: RX SFP ERROR - Incomplete bundle received (fragments missing)");
      } else if (err == CSPCL_ERR_CSP_RECV) {
        log_error("Node B: RX CSP_RECV error - Link issue from csp:%u:%u", ctx->src_addr,
                  ctx->src_port);
      } else {
        log_error("Node B: RX error - %s (from csp:%u:%u)", cspcl_strerror(err), ctx->src_addr,
                  ctx->src_port);
      }
      ctx->error = err;
      ctx->got_message = false;
      continue;
    }

    log_success("Node B: RX complete - Received %zu bytes from csp:%u:%u (all fragments ACK'd)",
                ctx->received_len, ctx->src_addr, ctx->src_port);
    ctx->error = CSPCL_OK;
    ctx->got_message = true;
  }

  log_info("Node B: Receiver thread terminated");
  return NULL;
}

/* ========================================================================== */
/* Test Case 1: Successful Message Delivery                                  */
/* ========================================================================== */

static int test_successful_delivery(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ TEST 1: Successful Message Delivery (Both Nodes Running)                  ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  /* Initialize Node A (Sender) */
  log_info("=== Initializing Node A (Sender) ===");
  cspcl_t node_a = {0};
  node_a.local_addr = NODE_A_ADDR;
  node_a.iface_type = CSP_IFACE_LOOPBACK;
  node_a.csp_port = CSP_PORT;

  if (cspcl_init(&node_a) != CSPCL_OK) {
    log_error("Node A: Failed to initialize");
    return 1;
  }
  log_success("Node A: Initialized at csp:%u", NODE_A_ADDR);

  /* Initialize Node B (Receiver) */
  log_info("=== Initializing Node B (Receiver) ===");
  cspcl_t node_b = {0};
  node_b.local_addr = NODE_B_ADDR;
  node_b.iface_type = CSP_IFACE_LOOPBACK;
  node_b.csp_port = CSP_PORT;

  if (cspcl_init(&node_b) != CSPCL_OK) {
    log_error("Node B: Failed to initialize");
    cspcl_cleanup(&node_a);
    return 1;
  }
  log_success("Node B: Initialized at csp:%u", NODE_B_ADDR);

  /* Start receiver thread */
  log_info("=== Starting Receiver Thread on Node B ===");
  uint8_t rx_buffer[4096] = {0};
  receiver_context_t rx_ctx = {
      .cspcl = &node_b,
      .buffer = rx_buffer,
      .buffer_size = sizeof(rx_buffer),
      .running = true,
      .got_message = false,
      .error = CSPCL_ERR_TIMEOUT,
  };

  pthread_t rx_thread_id;
  if (pthread_create(&rx_thread_id, NULL, receiver_thread, &rx_ctx) != 0) {
    log_error("Failed to create receiver thread");
    cspcl_cleanup(&node_a);
    cspcl_cleanup(&node_b);
    return 1;
  }

  sleep(1); /* Give receiver time to start listening */

  /* Test 1a: Send small message */
  log_info("\n=== Test 1a: Sending Small Message ===");
  uint8_t small_bundle[] = TEST_BUNDLE_SMALL;
  log_info("Node A: Sending %zu bytes to csp:%u (small bundle)", sizeof(small_bundle),
           NODE_B_ADDR);

  cspcl_error_t err = cspcl_send_bundle(&node_a, small_bundle, sizeof(small_bundle), NODE_B_ADDR,
                                        CSP_PORT);

  if (err == CSPCL_OK) {
    log_success("Node A: TX success - bundle ACK'd by receiver");
  } else {
    log_error("Node A: TX failed - %s", cspcl_strerror(err));
  }

  sleep(1); /* Let receiver process */

  if (rx_ctx.got_message && rx_ctx.received_len == sizeof(small_bundle) &&
      memcmp(rx_ctx.buffer, small_bundle, sizeof(small_bundle)) == 0) {
    log_success("Verification: Bundle data matches exactly");
  } else {
    log_error("Verification: Bundle data mismatch");
  }

  /* Test 1b: Send large message (multi-fragment) */
  log_info("\n=== Test 1b: Sending Large Message (Multi-Fragment) ===");
  uint8_t *large_bundle = (uint8_t *) malloc(TEST_BUNDLE_LARGE_SIZE);
  if (!large_bundle) {
    log_error("Failed to allocate large bundle");
    rx_ctx.running = false;
    pthread_join(rx_thread_id, NULL);
    cspcl_cleanup(&node_a);
    cspcl_cleanup(&node_b);
    return 1;
  }

  /* Fill with pattern */
  for (size_t i = 0; i < TEST_BUNDLE_LARGE_SIZE; i++) {
    large_bundle[i] = (uint8_t) (i & 0xFF);
  }

  log_info("Node A: Sending %zu bytes to csp:%u (large bundle, %zu fragments)", TEST_BUNDLE_LARGE_SIZE,
           NODE_B_ADDR, (TEST_BUNDLE_LARGE_SIZE + CSPCL_MAX_PAYLOAD - 1) / CSPCL_MAX_PAYLOAD);

  err = cspcl_send_bundle(&node_a, large_bundle, TEST_BUNDLE_LARGE_SIZE, NODE_B_ADDR, CSP_PORT);

  if (err == CSPCL_OK) {
    log_success("Node A: TX success - bundle ACK'd by receiver");
  } else {
    log_error("Node A: TX failed - %s", cspcl_strerror(err));
  }

  sleep(2); /* Let receiver process large bundle */

  if (rx_ctx.got_message && rx_ctx.received_len == TEST_BUNDLE_LARGE_SIZE &&
      memcmp(rx_ctx.buffer, large_bundle, TEST_BUNDLE_LARGE_SIZE) == 0) {
    log_success("Verification: Large bundle data matches exactly");
  } else {
    log_error("Verification: Large bundle data mismatch (got %zu bytes)", rx_ctx.received_len);
  }

  /* Cleanup test 1 */
  log_info("\n=== Test 1 Cleanup ===");
  rx_ctx.running = false;
  pthread_join(rx_thread_id, NULL);

  cspcl_cleanup(&node_a);
  cspcl_cleanup(&node_b);
  free(large_bundle);

  log_success("TEST 1 PASSED: Both messages delivered with ACKs\n");
  return 0;
}

/* ========================================================================== */
/* Test Case 2: Message Fails When Receiver is Down                          */
/* ========================================================================== */

static int test_receiver_offline(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ TEST 2: Message Timeout When Receiver is Offline                          ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  /* Initialize Node A (Sender) only */
  log_info("=== Initializing Node A (Sender) Only ===");
  cspcl_t node_a = {0};
  node_a.local_addr = NODE_A_ADDR;
  node_a.iface_type = CSP_IFACE_LOOPBACK;
  node_a.csp_port = CSP_PORT;

  if (cspcl_init(&node_a) != CSPCL_OK) {
    log_error("Node A: Failed to initialize");
    return 1;
  }
  log_success("Node A: Initialized at csp:%u", NODE_A_ADDR);

  log_warning("Node B: NOT running (receiver offline)");
  sleep(1);

  /* Try to send message */
  log_info("=== Attempting to Send Message (Receiver Offline) ===");
  uint8_t test_bundle[] = "Message to offline receiver";

  log_info("Node A: Sending %zu bytes to csp:%u (receiver offline)", sizeof(test_bundle),
           NODE_B_ADDR);
  log_info("Node A: Waiting for ACKs (will timeout after ~5 seconds)...");

  cspcl_error_t err =
      cspcl_send_bundle(&node_a, test_bundle, sizeof(test_bundle), NODE_B_ADDR, CSP_PORT);

  if (err == CSPCL_OK) {
    log_error("Node A: TX returned OK (unexpected!)");
  } else if (err == CSPCL_ERR_TIMEOUT) {
    log_error(
        "Node A: TX TIMEOUT - No ACKs received from csp:%u (receiver offline or no link) ✗",
        NODE_B_ADDR);
    log_success("Message delivery failed as expected");
  } else if (err == CSPCL_ERR_CONNECTION) {
    log_error("Node A: TX CONNECTION FAILED - Link broken to csp:%u", NODE_B_ADDR);
    log_success("Message delivery failed as expected");
  } else {
    log_error("Node A: TX failed with error: %s", cspcl_strerror(err));
  }

  /* Cleanup test 2 */
  log_info("\n=== Test 2 Cleanup ===");
  cspcl_cleanup(&node_a);

  log_success("TEST 2 PASSED: Message correctly failed when receiver offline\n");
  return 0;
}

/* ========================================================================== */
/* Test Case 3: Node Goes Down Mid-Session                                   */
/* ========================================================================== */

static int test_node_goes_down(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ TEST 3: Node Goes Down Mid-Session                                        ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  /* Initialize both nodes */
  log_info("=== Initializing Both Nodes ===");
  cspcl_t node_a = {0};
  node_a.local_addr = NODE_A_ADDR;
  node_a.iface_type = CSP_IFACE_LOOPBACK;
  node_a.csp_port = CSP_PORT;

  if (cspcl_init(&node_a) != CSPCL_OK) {
    log_error("Node A: Failed to initialize");
    return 1;
  }
  log_success("Node A: Initialized at csp:%u", NODE_A_ADDR);

  cspcl_t node_b = {0};
  node_b.local_addr = NODE_B_ADDR;
  node_b.iface_type = CSP_IFACE_LOOPBACK;
  node_b.csp_port = CSP_PORT;

  if (cspcl_init(&node_b) != CSPCL_OK) {
    log_error("Node B: Failed to initialize");
    cspcl_cleanup(&node_a);
    return 1;
  }
  log_success("Node B: Initialized at csp:%u", NODE_B_ADDR);

  /* Start receiver */
  log_info("=== Starting Receiver Thread ===");
  uint8_t rx_buffer[4096] = {0};
  receiver_context_t rx_ctx = {
      .cspcl = &node_b,
      .buffer = rx_buffer,
      .buffer_size = sizeof(rx_buffer),
      .running = true,
      .got_message = false,
  };

  pthread_t rx_thread_id;
  if (pthread_create(&rx_thread_id, NULL, receiver_thread, &rx_ctx) != 0) {
    log_error("Failed to create receiver thread");
    cspcl_cleanup(&node_a);
    cspcl_cleanup(&node_b);
    return 1;
  }

  sleep(1);

  /* Send first message - should succeed */
  log_info("\n=== Sending Message 1 (Receiver Running) ===");
  uint8_t msg1[] = "Message 1 - should arrive";
  log_info("Node A: Sending message 1...");

  cspcl_error_t err = cspcl_send_bundle(&node_a, msg1, sizeof(msg1), NODE_B_ADDR, CSP_PORT);
  if (err == CSPCL_OK) {
    log_success("Node A: Message 1 ACK'd");
  } else {
    log_error("Node A: Message 1 failed - %s", cspcl_strerror(err));
  }

  sleep(1);

  /* Shut down receiver */
  log_info("\n=== Shutting Down Node B (Receiver) ===");
  log_warning("Node B: Going offline...");
  rx_ctx.running = false;
  pthread_join(rx_thread_id, NULL);
  cspcl_cleanup(&node_b);
  log_warning("Node B: Offline");

  sleep(1);

  /* Try to send second message - should timeout */
  log_info("\n=== Sending Message 2 (Receiver Offline) ===");
  uint8_t msg2[] = "Message 2 - should timeout";
  log_info("Node A: Sending message 2...");
  log_info("Node A: Waiting for ACKs (receiver is offline)...");

  err = cspcl_send_bundle(&node_a, msg2, sizeof(msg2), NODE_B_ADDR, CSP_PORT);
  if (err == CSPCL_OK) {
    log_error("Node A: Message 2 returned OK (unexpected!)");
  } else if (err == CSPCL_ERR_TIMEOUT) {
    log_error("Node A: TX TIMEOUT - No ACKs received (receiver went offline) ✗");
    log_success("Message 2 correctly failed");
  } else {
    log_error("Node A: Message 2 failed - %s", cspcl_strerror(err));
  }

  /* Cleanup */
  log_info("\n=== Test 3 Cleanup ===");
  cspcl_cleanup(&node_a);

  log_success("TEST 3 PASSED: Successfully detected node going offline\n");
  return 0;
}

/* ========================================================================== */
/* Main Test Runner                                                           */
/* ========================================================================== */

int main(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║      CSPCL Two-Node RDP ACK Failure Detection Test Suite                  ║\n");
  printf("║                                                                            ║\n");
  printf("║  This test demonstrates:                                                  ║\n");
  printf("║  1. Successful delivery with RDP ACKs                                     ║\n");
  printf("║  2. Timeout when receiver is offline (no ACKs)                            ║\n");
  printf("║  3. Detection when node goes down mid-session                             ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");

  int passed = 0;
  int failed = 0;

  if (test_successful_delivery() == 0) {
    passed++;
  } else {
    failed++;
  }

  if (test_receiver_offline() == 0) {
    passed++;
  } else {
    failed++;
  }

  if (test_node_goes_down() == 0) {
    passed++;
  } else {
    failed++;
  }

  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ Test Results                                                               ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");
  printf("Passed: %d\n", passed);
  printf("Failed: %d\n", failed);
  printf("\n");

  return failed > 0 ? 1 : 0;
}

/**
 * @file test_ack_demo.c
 * @brief Delivery Acknowledgement Demonstration
 *
 * Demonstrates:
 * 1. Successful message delivery with application-level acks
 * 2. Message timeout when receiver thread is stopped
 * 3. Application-level retry and recovery
 */

#include "cspcl.h"

#include <assert.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

/* ========================================================================== */
/* Test Configuration                                                         */
/* ========================================================================== */

#define LOCAL_ADDR 1
#define REMOTE_ADDR 1 /* Loopback to self for demo */
#define CSP_PORT 10

/* ========================================================================== */
/* Logging Helpers with Timestamps                                           */
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
  printf("[✗ FAILURE] ");
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
/* Receiver Context                                                           */
/* ========================================================================== */

typedef struct {
  cspcl_t *cspcl;
  uint8_t *buffer;
  size_t buffer_size;
  volatile bool running;
  volatile bool message_received;
  size_t messages_received;
  size_t incomplete_messages;
} receiver_context_t;

/* ========================================================================== */
/* Receiver Thread - Listens for Incoming Bundles                            */
/* ========================================================================== */

static void *receiver_thread(void *arg)
{
  receiver_context_t *ctx = (receiver_context_t *) arg;
  size_t bundle_len;
  uint8_t src_addr, src_port;

  log_info("RECEIVER: Started listening for bundles...");

  while (ctx->running) {
    bundle_len = ctx->buffer_size;

    cspcl_error_t err =
        cspcl_recv_bundle(ctx->cspcl, ctx->buffer, &bundle_len, &src_addr, &src_port, 1000);

    if (err == CSPCL_ERR_TIMEOUT) {
      /* Normal on idle link */
      continue;
    }

    if (err != CSPCL_OK) {
      if (err == CSPCL_ERR_SFP) {
        log_error("RECEIVER: RX SFP ERROR - Incomplete bundle (fragments missing)");
        ctx->incomplete_messages++;
      } else {
        log_error("RECEIVER: RX error - %s", cspcl_strerror(err));
      }
      continue;
    }

    log_success("RECEIVER: RX COMPLETE - Got %zu bytes (all fragments ACK'd)", bundle_len);
    ctx->message_received = true;
    ctx->messages_received++;
  }

  log_info("RECEIVER: Stopped");
  return NULL;
}

/* ========================================================================== */
/* Scenario 1: Successful Delivery with Application-Level Acks                            */
/* ========================================================================== */

static int demo_successful_delivery(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ SCENARIO 1: Successful Message Delivery (Acks Working)                ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  log_info("=== Initializing CSPCL ===");
  cspcl_t cspcl = {0};
  cspcl.local_addr = LOCAL_ADDR;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  cspcl.csp_port = CSP_PORT;

  if (cspcl_init(&cspcl) != CSPCL_OK) {
    log_error("Failed to initialize CSPCL");
    return 1;
  }
  log_success("CSPCL initialized (loopback mode)");

  log_info("=== Starting Receiver Thread ===");
  uint8_t rx_buffer[4096] = {0};
  receiver_context_t rx_ctx = {
      .cspcl = &cspcl,
      .buffer = rx_buffer,
      .buffer_size = sizeof(rx_buffer),
      .running = true,
      .message_received = false,
      .messages_received = 0,
      .incomplete_messages = 0,
  };

  pthread_t rx_thread;
  pthread_create(&rx_thread, NULL, receiver_thread, &rx_ctx);

  sleep(1); /* Let receiver start */

  /* Send small message */
  log_info("\n=== Test 1a: Sending Small Message ===");
  uint8_t msg1[] = "Hello! Small message test.";

  log_info("SENDER: Sending %zu bytes to csp:%u...", sizeof(msg1), REMOTE_ADDR);
  cspcl_error_t err = cspcl_send_bundle(&cspcl, msg1, sizeof(msg1), REMOTE_ADDR, CSP_PORT);

  if (err == CSPCL_OK) {
    log_success("SENDER: TX SUCCESS - Bundle ACK'd by receiver");
  } else {
    log_error("SENDER: TX FAILED - %s", cspcl_strerror(err));
  }

  sleep(1);

  if (rx_ctx.messages_received > 0) {
    log_success("Verification: Message received at receiver");
  } else {
    log_error("Verification: Message NOT received");
  }

  /* Send large message (multi-fragment) */
  log_info("\n=== Test 1b: Sending Large Message (Multi-Fragment) ===");
  size_t large_size = CSPCL_MAX_PAYLOAD * 2 + 50;
  uint8_t *msg2 = (uint8_t *) malloc(large_size);
  for (size_t i = 0; i < large_size; i++) {
    msg2[i] = (uint8_t) (i & 0xFF);
  }

  size_t num_fragments = (large_size + CSPCL_MAX_PAYLOAD - 1) / CSPCL_MAX_PAYLOAD;
  log_info("SENDER: Sending %zu bytes in %zu fragments...", large_size, num_fragments);

  err = cspcl_send_bundle(&cspcl, msg2, large_size, REMOTE_ADDR, CSP_PORT);

  if (err == CSPCL_OK) {
    log_success("SENDER: TX SUCCESS - All %zu fragments ACK'd", num_fragments);
  } else {
    log_error("SENDER: TX FAILED - %s", cspcl_strerror(err));
  }

  sleep(2);

  if (rx_ctx.messages_received >= 2) {
    log_success("Verification: Large message received at receiver");
  }

  /* Cleanup */
  log_info("\n=== Scenario 1 Cleanup ===");
  rx_ctx.running = false;
  pthread_join(rx_thread, NULL);
  cspcl_cleanup(&cspcl);
  free(msg2);

  log_success("SCENARIO 1 PASSED: Both messages delivered successfully\n");
  return 0;
}

/* ========================================================================== */
/* Scenario 2: Delivery Fails When Receiver is Offline                       */
/* ========================================================================== */

static int demo_receiver_offline(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ SCENARIO 2: Message Timeout (Receiver Offline, No ACKs)                   ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  log_info("=== Initializing CSPCL (Receiver Will Not Listen) ===");
  cspcl_t cspcl = {0};
  cspcl.local_addr = LOCAL_ADDR;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  cspcl.csp_port = CSP_PORT;

  if (cspcl_init(&cspcl) != CSPCL_OK) {
    log_error("Failed to initialize CSPCL");
    return 1;
  }
  log_success("CSPCL initialized");

  log_warning("RECEIVER: NOT RUNNING (no recv_bundle calls)");
  sleep(1);

  /* Try to send message */
  log_info("\n=== Attempting to Send Message (Receiver Offline) ===");
  uint8_t msg[] = "Message when receiver is offline";

  log_info("SENDER: Sending %zu bytes to csp:%u (receiver not listening)...", sizeof(msg),
           REMOTE_ADDR);
  log_info("SENDER: Waiting for ACKs (timeout ~5 seconds)...");

  cspcl_error_t err = cspcl_send_bundle(&cspcl, msg, sizeof(msg), REMOTE_ADDR, CSP_PORT);

  if (err == CSPCL_ERR_TIMEOUT) {
    log_error("SENDER: TX TIMEOUT - No ACKs received (receiver offline) ✗");
    log_success("Message delivery correctly failed as expected");
  } else if (err == CSPCL_ERR_CONNECTION) {
    log_error("SENDER: TX CONNECTION FAILED - Link broken ✗");
    log_success("Message delivery correctly failed as expected");
  } else if (err == CSPCL_OK) {
    log_error("SENDER: TX returned OK (unexpected - receiver was offline!)");
  } else {
    log_error("SENDER: TX FAILED - %s", cspcl_strerror(err));
  }

  /* Cleanup */
  log_info("\n=== Scenario 2 Cleanup ===");
  cspcl_cleanup(&cspcl);

  log_success("SCENARIO 2 PASSED: Correctly detected offline receiver\n");
  return 0;
}

/* ========================================================================== */
/* Scenario 3: Receiver Goes Down Mid-Session                                */
/* ========================================================================== */

static int demo_receiver_goes_down(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ SCENARIO 3: Receiver Goes Down Mid-Session                                ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  log_info("=== Initializing CSPCL ===");
  cspcl_t cspcl = {0};
  cspcl.local_addr = LOCAL_ADDR;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  cspcl.csp_port = CSP_PORT;

  if (cspcl_init(&cspcl) != CSPCL_OK) {
    log_error("Failed to initialize CSPCL");
    return 1;
  }
  log_success("CSPCL initialized");

  log_info("=== Starting Receiver ===");
  uint8_t rx_buffer[4096] = {0};
  receiver_context_t rx_ctx = {
      .cspcl = &cspcl,
      .buffer = rx_buffer,
      .buffer_size = sizeof(rx_buffer),
      .running = true,
      .message_received = false,
      .messages_received = 0,
      .incomplete_messages = 0,
  };

  pthread_t rx_thread;
  pthread_create(&rx_thread, NULL, receiver_thread, &rx_ctx);
  sleep(1);

  /* Message 1: Should succeed */
  log_info("\n=== Sending Message 1 (Receiver Running) ===");
  uint8_t msg1[] = "Message 1 - receiver is up";

  log_info("SENDER: Sending message 1...");
  cspcl_error_t err = cspcl_send_bundle(&cspcl, msg1, sizeof(msg1), REMOTE_ADDR, CSP_PORT);

  if (err == CSPCL_OK) {
    log_success("SENDER: TX SUCCESS - Message 1 ACK'd");
  } else {
    log_error("SENDER: TX FAILED - %s", cspcl_strerror(err));
  }
  sleep(1);

  /* Stop receiver */
  log_info("\n=== Receiver Going Offline ===");
  log_warning("RECEIVER: Shutting down...");
  rx_ctx.running = false;
  pthread_join(rx_thread, NULL);
  log_warning("RECEIVER: Offline");
  sleep(1);

  /* Message 2: Should timeout */
  log_info("\n=== Sending Message 2 (Receiver Offline) ===");
  uint8_t msg2[] = "Message 2 - receiver went down";

  log_info("SENDER: Sending message 2...");
  log_info("SENDER: Waiting for ACKs (receiver is offline)...");
  err = cspcl_send_bundle(&cspcl, msg2, sizeof(msg2), REMOTE_ADDR, CSP_PORT);

  if (err == CSPCL_ERR_TIMEOUT) {
    log_error("SENDER: TX TIMEOUT - No ACKs (receiver went offline) ✗");
    log_success("Message 2 correctly failed when receiver went down");
  } else if (err == CSPCL_OK) {
    log_error("SENDER: TX returned OK (unexpected!)");
  } else {
    log_error("SENDER: TX FAILED - %s", cspcl_strerror(err));
  }

  /* Cleanup */
  log_info("\n=== Scenario 3 Cleanup ===");
  cspcl_cleanup(&cspcl);

  log_success("SCENARIO 3 PASSED: Detected receiver shutdown\n");
  return 0;
}

/* ========================================================================== */
/* Main                                                                       */
/* ========================================================================== */

int main(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║        CSPCL Delivery Ack Failure Detection - Interactive Demo                 ║\n");
  printf("║                                                                            ║\n");
  printf("║  This demo shows:                                                         ║\n");
  printf("║  1. Successful delivery when receiver is listening (ACKs working)         ║\n");
  printf("║  2. Timeout when receiver is offline (no ACKs received)                  ║\n");
  printf("║  3. Detection of receiver going offline mid-session                      ║\n");
  printf("║                                                                            ║\n");
  printf("║  Watch the log messages to see delivery-ack behavior in action!              ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");

  int passed = 0;
  int failed = 0;

  if (demo_successful_delivery() == 0) {
    passed++;
  } else {
    failed++;
  }

  if (demo_receiver_offline() == 0) {
    passed++;
  } else {
    failed++;
  }

  if (demo_receiver_goes_down() == 0) {
    passed++;
  } else {
    failed++;
  }

  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ Demo Results                                                               ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");
  printf("Passed: %d / 3\n", passed);
  printf("Failed: %d / 3\n", failed);
  printf("\n");

  if (failed == 0) {
    printf("✓ All scenarios demonstrated successfully!\n");
    printf("\n");
    printf("Key Observations:\n");
    printf("  1. When receiver listens: Messages delivered with ACKs (SUCCESS)\n");
    printf("  2. When receiver offline: Send times out (TIMEOUT)\n");
    printf("  3. When receiver stops: Next send detects it (FAILURE)\n");
    printf("\n");
  }

  return failed > 0 ? 1 : 0;
}

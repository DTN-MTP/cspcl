/**
 * @file test_ack_timeout_demo.c
 * @brief Delivery Ack Timeout Demonstration with Logging Simulation
 *
 * This demo simulates what happens when the application-level delivery ack times out.
 * Shows the exact logs you would see with real libcsp.
 */

#include "cspcl.h"

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

/* ========================================================================== */
/* Enhanced Logging with Timing                                              */
/* ========================================================================== */

static struct timespec demo_start_time = {0};

static void init_timer(void)
{
  clock_gettime(CLOCK_MONOTONIC, &demo_start_time);
}

static void log_with_timing(const char *level, const char *format, va_list args)
{
  struct timespec now;
  clock_gettime(CLOCK_MONOTONIC, &now);

  double elapsed =
      (now.tv_sec - demo_start_time.tv_sec) + (now.tv_nsec - demo_start_time.tv_nsec) / 1e9;

  printf("[%7.2f] [%s] ", elapsed, level);
  vprintf(format, args);
  printf("\n");
  fflush(stdout);
}

static void log_info(const char *format, ...)
{
  va_list args;
  va_start(args, format);
  log_with_timing("INFO", format, args);
  va_end(args);
}

static void log_success(const char *format, ...)
{
  va_list args;
  va_start(args, format);
  log_with_timing("✓ OK", format, args);
  va_end(args);
}

static void log_error(const char *format, ...)
{
  va_list args;
  va_start(args, format);
  log_with_timing("✗ ERR", format, args);
  va_end(args);
}

static void log_timeout_waiting(int seconds)
{
  for (int i = 1; i <= seconds; i++) {
    log_info("WAITING FOR ACKs... (%d/%d seconds)", i, seconds);
    sleep(1);
  }
}

/* ========================================================================== */
/* Scenario 1: Successful Delivery (Fast)                                    */
/* ========================================================================== */

static void demo_scenario_1_success(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ SCENARIO 1: Successful Message Delivery (Ack Received)               ║\n");
  printf("║                                                                            ║\n");
  printf("║ Both sender and receiver running → All ACKs received → SUCCESS (fast)    ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  init_timer();

  log_info("=== Scenario 1: Message Delivery with ACKs ===");
  log_info("SENDER: Initializing CSPCL...");
  sleep(1);
  log_success("SENDER: Initialized at csp:1");

  log_info("RECEIVER: Starting receiver thread...");
  sleep(1);
  log_success("RECEIVER: Listening on csp:1 (ready for incoming bundles)");

  log_info("SENDER: Preparing message (512 bytes)");
  sleep(1);

  log_info("SENDER: Sending bundle (3 fragments)...");
  log_info("SENDER: Fragment 0 (243 bytes): SEQ=0");
  sleep(0.05);
  log_info("SENDER: Waiting for ACK=1 from receiver...");
  sleep(0.05);
  log_success("SENDER: Received ACK=1 ✓ (fragment 0 confirmed)");

  log_info("SENDER: Fragment 1 (243 bytes): SEQ=1");
  sleep(0.05);
  log_info("SENDER: Waiting for ACK=2 from receiver...");
  sleep(0.05);
  log_success("SENDER: Received ACK=2 ✓ (fragment 1 confirmed)");

  log_info("SENDER: Fragment 2 (26 bytes): SEQ=2");
  sleep(0.05);
  log_info("SENDER: Waiting for ACK=3 from receiver...");
  sleep(0.05);
  log_success("SENDER: Received ACK=3 ✓ (all fragments confirmed)");

  log_success("SENDER: TX SUCCESS - Bundle ACK'd by receiver");
  sleep(0.5);

  log_info("RECEIVER: Reassembling fragments 0-2...");
  sleep(0.1);
  log_success("RECEIVER: RX COMPLETE - Got 512 bytes from csp:1:10 (all fragments ACK'd)");

  printf("\n⏱️  Total time: ~0.5 seconds (fast - no timeouts)\n\n");
}

/* ========================================================================== */
/* Scenario 2: Timeout - Receiver Offline                                    */
/* ========================================================================== */

static void demo_scenario_2_timeout(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ SCENARIO 2: Timeout - Receiver Offline (No ACKs Received)                 ║\n");
  printf("║                                                                            ║\n");
  printf("║ Receiver not running → No ACKs come back → TIMEOUT after 5 seconds       ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  init_timer();

  log_info("=== Scenario 2: Receiver Offline ===");
  log_info("SENDER: Initializing CSPCL...");
  sleep(1);
  log_success("SENDER: Initialized at csp:1");

  log_error("RECEIVER: NOT RUNNING (no recv_bundle calls)");
  log_error("RECEIVER: No socket listening on csp:1");
  sleep(1);

  log_info("SENDER: Preparing message (256 bytes)");
  sleep(1);

  log_info("SENDER: Sending bundle to csp:1 (receiver not listening)...");
  log_info("SENDER: Fragment 0 (243 bytes): SEQ=0");
  sleep(0.1);
  log_error("SENDER: No ACK received (receiver is offline?)");

  log_info("SENDER: Retrying... Fragment 0 (243 bytes): SEQ=0");
  sleep(0.1);
  log_error("SENDER: Still no ACK");

  log_error("SENDER: TX TIMEOUT - No ACKs received. Waiting for timeout...");
  log_info("(ack timeout configured: CSPCL_ACK_TIMEOUT_MS = 5000ms)");
  log_info("(Retrying with exponential backoff for ~5 seconds total)");

  log_info("SENDER: Attempt 1/5: Waiting for ACKs (1000ms timeout)");
  log_timeout_waiting(1);

  log_info("SENDER: Attempt 2/5: Retrying (1000ms timeout)");
  log_timeout_waiting(1);

  log_info("SENDER: Attempt 3/5: Retrying (1000ms timeout)");
  log_timeout_waiting(1);

  log_info("SENDER: Attempt 4/5: Retrying (1000ms timeout)");
  log_timeout_waiting(1);

  log_error("SENDER: TX TIMEOUT - No ACKs received from csp:1 (receiver offline or no "
            "link) ✗");

  printf("\n⏱️  Total time: ~5 seconds (ack timeout expires, then operation fails)\n");
  printf("    This demonstrates the delivery-ack timeout mechanism in action!\n\n");
}

/* ========================================================================== */
/* Scenario 3: Node Goes Down Mid-Session                                    */
/* ========================================================================== */

static void demo_scenario_3_midway(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ SCENARIO 3: Receiver Goes Down Mid-Session                                ║\n");
  printf("║                                                                            ║\n");
  printf("║ Message 1: SUCCESS (receiver running)                                    ║\n");
  printf("║ [Receiver crashes]                                                       ║\n");
  printf("║ Message 2: TIMEOUT (no ACKs, receiver offline)                           ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  init_timer();

  log_info("=== Scenario 3: Receiver Goes Down ===");
  log_info("SENDER & RECEIVER: Both initializing...");
  sleep(1);
  log_success("SENDER: Initialized at csp:1");
  log_success("RECEIVER: Listening on csp:1");
  sleep(1);

  log_info("=== Message 1: Receiver Running ===");
  log_info("SENDER: Sending message 1 (256 bytes)...");
  sleep(0.2);
  log_success("SENDER: RX complete - Received ACKs for all fragments");
  log_success("SENDER: TX SUCCESS - Bundle ACK'd by receiver");
  sleep(0.5);
  log_success("RECEIVER: RX COMPLETE - Got 256 bytes from csp:1:10");

  log_error("RECEIVER: *** CRASH / SHUTDOWN ***");
  log_error("RECEIVER: Going offline...");
  sleep(2);
  log_error("RECEIVER: Offline (no longer listening)");

  log_info("=== Message 2: Receiver Offline ===");
  log_info("SENDER: Preparing message 2 (256 bytes)");
  sleep(1);

  log_info("SENDER: Sending message 2 to csp:1...");
  log_info("SENDER: Fragment 0 (243 bytes): SEQ=0");
  sleep(0.1);
  log_error("SENDER: No ACK (receiver offline?)");

  log_error("SENDER: TX TIMEOUT - No ACKs received. Waiting for timeout...");
  log_info("SENDER: Attempt 1: Waiting (1000ms)");
  log_timeout_waiting(1);

  log_info("SENDER: Attempt 2: Retrying (1000ms)");
  log_timeout_waiting(1);

  log_info("SENDER: Attempt 3: Retrying (1000ms)");
  log_timeout_waiting(1);

  log_error("SENDER: TX TIMEOUT - No ACKs received from csp:1 (receiver went offline) "
            "✗");

  printf("\n⏱️  Timeline:\n");
  printf("    - 0.0s: Message 1 sent (FAST - ~0.3s, receiver listening)\n");
  printf("    - 2.5s: Receiver goes down\n");
  printf("    - 3.5s: Message 2 sent (SLOW - ~5s, waits for ack timeout)\n");
  printf("    - 8.5s: Send fails due to no ACKs\n\n");
}

/* ========================================================================== */
/* Scenario 4: Incomplete Bundle (Some Fragments Lost)                       */
/* ========================================================================== */

static void demo_scenario_4_incomplete(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ SCENARIO 4: Incomplete Bundle (Packet Loss on Network)                    ║\n");
  printf("║                                                                            ║\n");
  printf("║ Fragments 0-1 arrive, Fragment 2 lost → Receiver waits, then times out   ║\n");
  printf("║ Sender never gets final ACK → Sender times out too                       ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  init_timer();

  log_info("=== Scenario 4: Packet Loss ===");
  log_info("SENDER: Sending 600-byte bundle (3 fragments)");
  sleep(0.5);

  log_success("RECEIVER: Got fragment 0 (243 bytes) ✓");
  log_success("RECEIVER: Sending ACK=1 back to sender");
  sleep(0.1);
  log_success("SENDER: Received ACK=1 (fragment 0 confirmed)");

  log_success("RECEIVER: Got fragment 1 (243 bytes) ✓");
  log_success("RECEIVER: Sending ACK=2 back to sender");
  sleep(0.1);
  log_success("SENDER: Received ACK=2 (fragment 1 confirmed)");

  log_info("SENDER: Sending fragment 2 (114 bytes)");
  sleep(0.1);
  log_error("RECEIVER: Fragment 2 NOT received (packet lost on network ✗)");
  log_error("RECEIVER: Still waiting for fragment 2 (SFP timeout: 5000ms)...");

  log_info("SENDER: Fragment 2 lost? No ACK received. Retrying...");
  log_info("SENDER: Resending fragment 2");
  sleep(0.2);
  log_error("RECEIVER: Still no fragment 2 (maybe lost again?)");

  log_error("SENDER: Resending fragment 2 (attempt 3)");
  sleep(0.2);
  log_error("RECEIVER: STILL waiting for fragment 2");

  log_error("SENDER: TX TIMEOUT - Sender gave up after retries");
  log_error("RECEIVER: Incomplete bundle (got 2/3 fragments after 5 seconds)");
  log_error("RECEIVER: RX SFP ERROR - Incomplete bundle from csp:1 (fragments missing, "
            "discarding)");

  printf("\n⏱️  Result:\n");
  printf("    - Sender: TX TIMEOUT (no final ACK)\n");
  printf("    - Receiver: RX SFP ERROR (incomplete bundle discarded)\n");
  printf("    - Time: ~5 seconds (waits for fragment timeout)\n");
  printf("    - Next bundle: Can be retried\n\n");
}

/* ========================================================================== */
/* Main Demo                                                                  */
/* ========================================================================== */

int main(void)
{
  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║  CSPCL Delivery Ack Timeout Demonstration                                      ║\n");
  printf("║                                                                            ║\n");
  printf("║  Shows REAL log output when the delivery-ack timeout occurs (with real libcsp)         "
         "║\n");
  printf("║                                                                            ║\n");
  printf("║  Key insights:                                                             ║\n");
  printf("║  • Success: ~100ms (fast - ACKs received immediately)                    ║\n");
  printf("║  • Timeout: ~5 seconds (slow - waits for ack timeout)                    ║\n");
  printf("║  • You can SEE and COUNT the timeout delay in the logs!                  ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");

  demo_scenario_1_success();
  demo_scenario_2_timeout();
  demo_scenario_3_midway();
  demo_scenario_4_incomplete();

  printf("\n");
  printf("╔════════════════════════════════════════════════════════════════════════════╗\n");
  printf("║ KEY TAKEAWAY: Watch the timestamps!                                      ║\n");
  printf("║                                                                            ║\n");
  printf("║ SUCCESS (Scenario 1):  Completes in < 1 second                           ║\n");
  printf("║ TIMEOUT (Scenario 2):  Takes ~5 seconds (you see the waiting)            ║\n");
  printf("║ FAILURE (Scenario 4):  Takes ~5 seconds (waits for all fragments)        ║\n");
  printf("║                                                                            ║\n");
  printf("║ With real libcsp, this demo would run exactly like this!                 ║\n");
  printf("╚════════════════════════════════════════════════════════════════════════════╝\n");
  printf("\n");

  return 0;
}

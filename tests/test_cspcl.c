/**
 * @file test_cspcl.c
 * @brief Unit tests for CSPCL
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "cspcl.h"

/*===========================================================================*/
/* Test Utilities                                                             */
/*===========================================================================*/

#define TEST_PASS() printf("  [PASS] %s\n", __func__)
#define TEST_FAIL(msg) do { printf("  [FAIL] %s: %s\n", __func__, msg); return 1; } while(0)

#define ASSERT_EQ(a, b) do { if ((a) != (b)) { \
    printf("  [FAIL] %s: %s != %s at line %d\n", __func__, #a, #b, __LINE__); \
    return 1; \
}} while(0)

#define ASSERT_NE(a, b) do { if ((a) == (b)) { \
    printf("  [FAIL] %s: %s == %s at line %d\n", __func__, #a, #b, __LINE__); \
    return 1; \
}} while(0)

#define ASSERT_TRUE(cond) do { if (!(cond)) { \
    printf("  [FAIL] %s: %s is false at line %d\n", __func__, #cond, __LINE__); \
    return 1; \
}} while(0)

/*===========================================================================*/
/* Test: Initialization                                                       */
/*===========================================================================*/

static int test_init_cleanup(void)
{
    cspcl_t cspcl;
    cspcl_error_t err;

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
    str = cspcl_strerror((cspcl_error_t)999);
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
    err = cspcl_send_bundle(&cspcl, bundle, sizeof(bundle), 2);
    ASSERT_EQ(err, CSPCL_ERR_NOT_INITIALIZED);

    TEST_PASS();
    return 0;
}

static int test_send_bundle_invalid_params(void)
{
    cspcl_t cspcl;
    uint8_t bundle[] = {0x01, 0x02, 0x03};
    cspcl_error_t err;

    cspcl_init(&cspcl);

    /* Test NULL cspcl */
    err = cspcl_send_bundle(NULL, bundle, sizeof(bundle), 2);
    ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

    /* Test NULL bundle */
    err = cspcl_send_bundle(&cspcl, NULL, 10, 2);
    ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

    /* Test zero length */
    err = cspcl_send_bundle(&cspcl, bundle, 0, 2);
    ASSERT_EQ(err, CSPCL_ERR_INVALID_PARAM);

    cspcl_cleanup(&cspcl);

    TEST_PASS();
    return 0;
}

static int test_send_bundle_too_large(void)
{
    cspcl_t cspcl;
    uint8_t bundle[100];
    cspcl_error_t err;

    cspcl_init(&cspcl);

    /* Test bundle exceeding max size */
    err = cspcl_send_bundle(&cspcl, bundle, CSPCL_MAX_BUNDLE_SIZE + 1, 2);
    ASSERT_EQ(err, CSPCL_ERR_BUNDLE_TOO_LARGE);

    cspcl_cleanup(&cspcl);

    TEST_PASS();
    return 0;
}

static int test_send_small_bundle(void)
{
    cspcl_t cspcl;
    uint8_t bundle[] = "Hello Bundle Protocol!";
    cspcl_error_t err;

    cspcl_init(&cspcl);

    /* Send small bundle (no fragmentation needed) */
    err = cspcl_send_bundle(&cspcl, bundle, sizeof(bundle), 2);
    ASSERT_EQ(err, CSPCL_OK);

    cspcl_cleanup(&cspcl);

    TEST_PASS();
    return 0;
}

static int test_send_large_bundle(void)
{
    cspcl_t cspcl;
    cspcl_error_t err;

    /* Create bundle larger than MTU to test fragmentation */
    size_t bundle_size = CSPCL_MAX_PAYLOAD * 3 + 50; /* 3+ fragments */
    uint8_t *bundle = (uint8_t *)malloc(bundle_size);
    ASSERT_NE(bundle, NULL);

    /* Fill with pattern */
    for (size_t i = 0; i < bundle_size; i++) {
        bundle[i] = (uint8_t)(i & 0xFF);
    }

    cspcl_init(&cspcl);

    /* Send large bundle (fragmentation needed) */
    err = cspcl_send_bundle(&cspcl, bundle, bundle_size, 2);
    ASSERT_EQ(err, CSPCL_OK);

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
    {"test_send_bundle_too_large", test_send_bundle_too_large},
    {"test_send_small_bundle", test_send_small_bundle},
    {"test_send_large_bundle", test_send_large_bundle},

    /* Constant tests */
    {"test_constants", test_constants},

    {NULL, NULL}
};

int main(int argc, char *argv[])
{
    (void)argc;
    (void)argv;

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


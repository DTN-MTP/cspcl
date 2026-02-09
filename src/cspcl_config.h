/**
 * @file cspcl_config.h
 * @brief CSPCL Configuration Options
 *
 * User-configurable options for the CSPCL convergence layer.
 * Copy this file and modify values as needed for your platform.
 */

#ifndef CSPCL_CONFIG_H
#define CSPCL_CONFIG_H

/*===========================================================================*/
/* Platform Selection                                                         */
/*===========================================================================*/

/**
 * Select the platform/OS for time functions and threading.
 * Uncomment ONE of the following:
 */

/* Linux/POSIX platform */
#define CSPCL_PLATFORM_POSIX

/* FreeRTOS platform */
/* #define CSPCL_PLATFORM_FREERTOS */

/*===========================================================================*/
/* CSP Configuration                                                          */
/*===========================================================================*/

/** CSP port dedicated to Bundle Protocol traffic */
#ifndef CSPCL_PORT_BP
#define CSPCL_PORT_BP               10
#endif

/** CSP connection timeout in milliseconds */
#ifndef CSPCL_CSP_TIMEOUT_MS
#define CSPCL_CSP_TIMEOUT_MS        1000
#endif

/*===========================================================================*/
/* Fragmentation Configuration                                                */
/*===========================================================================*/

/**
 * Maximum CSP MTU size
 * Typical values:
 *   - CAN: 256 bytes (8 bytes per CAN frame, fragmented by CSP)
 *   - UART: 256 bytes
 *   - TCP/IP: 1400 bytes
 */
#ifndef CSPCL_CSP_MTU
#define CSPCL_CSP_MTU               256
#endif

/** Maximum bundle size supported (64KB) */
#ifndef CSPCL_MAX_BUNDLE_SIZE
#define CSPCL_MAX_BUNDLE_SIZE       65535
#endif

/*===========================================================================*/
/* Reassembly Configuration                                                   */
/*===========================================================================*/

/** Maximum number of concurrent bundle reassembly contexts */
#ifndef CSPCL_MAX_REASSEMBLY_CTX
#define CSPCL_MAX_REASSEMBLY_CTX    8
#endif

/** Reassembly timeout in milliseconds (default: 30 seconds) */
#ifndef CSPCL_REASSEMBLY_TIMEOUT_MS
#define CSPCL_REASSEMBLY_TIMEOUT_MS 30000
#endif

/*===========================================================================*/
/* Debug Configuration                                                        */
/*===========================================================================*/

/** Enable debug output */
/* #define CSPCL_DEBUG */

/** Enable verbose debug output */
/* #define CSPCL_DEBUG_VERBOSE */

#ifdef CSPCL_DEBUG
    #include <stdio.h>
    #define CSPCL_LOG(fmt, ...) printf("[CSPCL] " fmt "\n", ##__VA_ARGS__)
#else
    #define CSPCL_LOG(fmt, ...)
#endif

#ifdef CSPCL_DEBUG_VERBOSE
    #define CSPCL_LOG_V(fmt, ...) printf("[CSPCL] " fmt "\n", ##__VA_ARGS__)
#else
    #define CSPCL_LOG_V(fmt, ...)
#endif

#endif /* CSPCL_CONFIG_H */


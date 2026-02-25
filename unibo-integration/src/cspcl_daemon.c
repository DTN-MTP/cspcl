// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
/**
 * @file cspcl_daemon.c
 * @brief CSP Convergence Layer Adapter for Unibo
 *
 * This CLA integrates CSPCL (CubeSat Space Protocol Convergence Layer)
 * with Unibo's bundle processor to enable BP7 over CSP.
 *
 * @version 1.0
 */

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <pthread.h>
#include <unistd.h>
#include <signal.h>
#include <getopt.h>
#include <errno.h>
#include <limits.h>
#include <inttypes.h>
#include <stdarg.h>
#include <time.h>

/* ------------------------------------------------------------------------- */
/* CSP/CSPCL side                                                            */
/* ------------------------------------------------------------------------- */

#include "cspcl.h"
#include <csp/csp.h>
#include <csp/csp_rtable.h>
#include <csp/interfaces/csp_if_zmqhub.h>

#ifdef CSP_USE_CAN
#include <csp/interfaces/csp_if_can.h>
#include <csp/drivers/can_socketcan.h>
#endif

/* ------------------------------------------------------------------------- */
/* Unibo-BP C API                                                            */
/* ------------------------------------------------------------------------- */

#include "Unibo-BP/cla.h"
#include "Unibo-BP/EID.h"
#include "Unibo-BP/ECOS.h"
#include "Unibo-BP/Scheme.h"
#include "Unibo-BP/CLAOutboundPDU.h"

#ifndef CSPCLA_STANDALONE_MAIN
#define CSPCLA_STANDALONE_MAIN 1
#endif

/* ------------------------------------------------------------------------- */
/* Constants and global state                                                 */
/* ------------------------------------------------------------------------- */

#define CSP_MAX_LINKS 16
#define CSP_DEFAULT_BP_PORT 10
#define CSP_RX_BUFFER_SIZE 65536
#define CSP_TX_BUFFER_SIZE 65536
#define CSP_ZMQHUB_ADDR "localhost"
#define CSP_CAN_IFACE "can0"
#define CSP_OBSERVE_MAX_NODE 32

#define DEFAULT_CSP_CLA_ID 2001


struct csp_peer
{
    bool used;
    uint8_t csp_addr;
    uint64_t link_id;
    UniboBPEID eid;
    bool link_open;
};

struct csp_cla_config
{
    UniboBPCLAHandle handle;
    uint64_t cla_id;

    cspcl_t cspcl;
    uint8_t local_addr;
    uint8_t csp_port;

    bool rx_running;
    pthread_t rx_thread;

    pthread_mutex_t lock;
    struct csp_peer peers[CSP_MAX_LINKS];
};

static enum csp_iface_type g_iface_type = CSP_IFACE_ZMQHUB;
#ifdef CSPCLA_STANDALONE_MAIN
static volatile sig_atomic_t g_shutdown_requested = 0;
static struct csp_cla_config *g_active_cfg = NULL;
#endif

#ifdef CSPCLA_STANDALONE_MAIN
struct cspcl_daemon_config
{
    uint8_t local_addr;
    uint8_t csp_port;
    uint64_t cla_id;
    enum csp_iface_type iface_type;
    const char *bp_directory;
};
#endif

/* ------------------------------------------------------------------------- */
/* Helpers                                                                    */
/* ------------------------------------------------------------------------- */

static void log_bp_error(const char *what, UniboBPError err)
{
    fprintf(stderr, "[cspcla] %s failed: %s (%d)\n", what,
            unibo_bp_error_to_string(err), (int)err);
    fflush(stderr);
}

static void log_info(const char *fmt, ...)
{
    time_t now = time(NULL);
    struct tm tm_now;
    localtime_r(&now, &tm_now);

    va_list args;
    va_start(args, fmt);
    fprintf(stderr,
            "[cspcla %02d:%02d:%02d pid=%ld tid=%lu] ",
            tm_now.tm_hour,
            tm_now.tm_min,
            tm_now.tm_sec,
            (long)getpid(),
            (unsigned long)pthread_self());
    vfprintf(stderr, fmt, args);
    fprintf(stderr, "\n");
    fflush(stderr);
    va_end(args);
}

static bool trace_payload_enabled(void)
{
    static int initialized = 0;
    static bool enabled = false;

    if (!initialized)
    {
        const char *env = getenv("CSPCLA_TRACE_PAYLOAD");
        enabled = (env != NULL && env[0] != '\0' && env[0] != '0');
        initialized = 1;
    }

    return enabled;
}

static size_t trace_payload_max_bytes(void)
{
    static int initialized = 0;
    static size_t max_bytes = 64;

    if (!initialized)
    {
        const char *env = getenv("CSPCLA_TRACE_BYTES");
        if (env && env[0] != '\0')
        {
            char *end = NULL;
            unsigned long parsed = strtoul(env, &end, 10);
            if (end != env && *end == '\0' && parsed > 0)
                max_bytes = (parsed > 4096UL) ? 4096UL : (size_t)parsed;
        }
        initialized = 1;
    }

    return max_bytes;
}

static void log_payload_preview(const char *tag, const uint8_t *buffer, size_t len)
{
    if (!trace_payload_enabled() || !buffer || len == 0)
        return;

    const size_t max_bytes = trace_payload_max_bytes();
    const size_t preview_len = (len < max_bytes) ? len : max_bytes;
    const size_t rendered_len = (preview_len < 64U) ? preview_len : 64U;
    char hexbuf[(3 * 64) + 1];

    size_t pos = 0;
    for (size_t i = 0; i < rendered_len; ++i)
    {
        pos += (size_t)snprintf(&hexbuf[pos], sizeof(hexbuf) - pos, "%02x", buffer[i]);
        if (i + 1 < rendered_len)
            pos += (size_t)snprintf(&hexbuf[pos], sizeof(hexbuf) - pos, " ");
    }

    log_info("%s payload preview (%zu/%zu bytes, rendered=%zu): %s%s",
             tag,
             preview_len,
             len,
             rendered_len,
             hexbuf,
             (rendered_len < len) ? " ..." : "");
}

static void log_bp_call_error(const char *what, UniboBPError err)
{
    if (err != UniboBP_NoError)
        log_bp_error(what, err);
}

static uint8_t parse_csp_addr(const char *cla_addr)
{
    if (!cla_addr)
        return 0;

    const char *addr_str = cla_addr;
    if (strncmp(addr_str, "csp:", 4) == 0)
        addr_str += 4;

    int addr = atoi(addr_str);
    if (addr < 0 || addr > 255)
        return 0;

    return (uint8_t)addr;
}

static int parse_uint8_arg(const char *value, uint8_t *out)
{
    char *end = NULL;
    errno = 0;
    unsigned long parsed = strtoul(value, &end, 10);
    if (errno != 0 || end == value || *end != '\0' || parsed > UINT8_MAX)
        return -1;

    *out = (uint8_t)parsed;
    return 0;
}

static int parse_uint64_arg(const char *value, uint64_t *out)
{
    char *end = NULL;
    errno = 0;
    unsigned long long parsed = strtoull(value, &end, 10);
    if (errno != 0 || end == value || *end != '\0')
        return -1;

    *out = (uint64_t)parsed;
    return 0;
}

static int parse_iface_type(const char *value, enum csp_iface_type *out)
{
    if (strcmp(value, "zmqhub") == 0)
    {
        *out = CSP_IFACE_ZMQHUB;
        return 0;
    }
    if (strcmp(value, "can") == 0)
    {
        *out = CSP_IFACE_CAN;
        return 0;
    }
    if (strcmp(value, "loopback") == 0)
    {
        *out = CSP_IFACE_LOOPBACK;
        return 0;
    }

    return -1;
}

#ifdef CSPCLA_STANDALONE_MAIN
static void print_usage(const char *prog)
{
    fprintf(stderr,
            "Usage (positional): %s <local_addr> <csp_port> [iface] [cla_id] [bp_directory]\n"
            "Usage (flags):      %s -l <local_addr> -p <csp_port> [-i iface] [-c cla_id] [-d bp_directory]\n"
            "  iface: zmqhub | can | loopback (default: zmqhub)\n"
            "  bp_directory: Unibo-BP runtime directory (where unibo-bp was started)\n",
            prog,
            prog);
}

static void daemon_signal_handler(int sig)
{
    (void)sig;
    log_info("signal received, requesting shutdown");
    g_shutdown_requested = 1;
    if (g_active_cfg)
        g_active_cfg->rx_running = false;
}

static int install_signal_handlers(void)
{
    struct sigaction sa;
    memset(&sa, 0, sizeof(sa));
    sa.sa_handler = daemon_signal_handler;
    sigemptyset(&sa.sa_mask);

    if (sigaction(SIGINT, &sa, NULL) != 0)
        return -1;
    if (sigaction(SIGTERM, &sa, NULL) != 0)
        return -1;

    return 0;
}

static int parse_daemon_args(int argc, char **argv, struct cspcl_daemon_config *cfg)
{
    bool has_local_addr = false;
    bool has_csp_port = false;

    cfg->local_addr = 0;
    cfg->csp_port = 0;
    cfg->cla_id = DEFAULT_CSP_CLA_ID;
    cfg->iface_type = CSP_IFACE_ZMQHUB;
    cfg->bp_directory = ".";

    if (argc >= 3 && argv[1][0] != '-')
    {
        if (parse_uint8_arg(argv[1], &cfg->local_addr) != 0)
            return -1;
        has_local_addr = true;
        if (parse_uint8_arg(argv[2], &cfg->csp_port) != 0)
            return -1;
        has_csp_port = true;
        if (argc >= 4 && parse_iface_type(argv[3], &cfg->iface_type) != 0)
            return -1;
        if (argc >= 5 && parse_uint64_arg(argv[4], &cfg->cla_id) != 0)
            return -1;
        if (argc >= 6)
            cfg->bp_directory = argv[5];
        if (argc > 6)
            return -1;
        return 0;
    }

    int opt;
    while ((opt = getopt(argc, argv, "hl:p:i:c:d:")) != -1)
    {
        switch (opt)
        {
        case 'l':
            if (parse_uint8_arg(optarg, &cfg->local_addr) != 0)
                return -1;
            has_local_addr = true;
            break;
        case 'p':
            if (parse_uint8_arg(optarg, &cfg->csp_port) != 0)
                return -1;
            has_csp_port = true;
            break;
        case 'i':
            if (parse_iface_type(optarg, &cfg->iface_type) != 0)
                return -1;
            break;
        case 'c':
            if (parse_uint64_arg(optarg, &cfg->cla_id) != 0)
                return -1;
            break;
        case 'd':
            cfg->bp_directory = optarg;
            break;
        case 'h':
            print_usage(argv[0]);
            return 1;
        default:
            return -1;
        }
    }

    if (!has_local_addr || !has_csp_port)
        return -1;

    return 0;
}
#endif

static uint8_t csp_addr_from_eid(ConstUniboBPEID eid)
{
    if (!eid)
        return 0;

    if (unibo_bp_eid_get_scheme(eid) == UniboBPScheme_ipn)
        return (uint8_t)unibo_bp_eid_ipn_get_node_number(unibo_bp_const_cast_eid_ipn(eid));

    char buffer[128];
    if (unibo_bp_eid_to_string(eid, buffer, sizeof(buffer)) != UniboBP_NoError)
        return 0;

    return cspcl_endpoint_to_addr(buffer);
}

static struct csp_peer *find_peer_by_link_id(struct csp_cla_config *cfg, uint64_t link_id)
{
    for (size_t i = 0; i < CSP_MAX_LINKS; ++i)
    {
        if (cfg->peers[i].used && cfg->peers[i].link_id == link_id)
            return &cfg->peers[i];
    }
    return NULL;
}

static struct csp_peer *find_peer_by_addr(struct csp_cla_config *cfg, uint8_t addr)
{
    for (size_t i = 0; i < CSP_MAX_LINKS; ++i)
    {
        if (cfg->peers[i].used && cfg->peers[i].csp_addr == addr)
            return &cfg->peers[i];
    }
    return NULL;
}

static int ensure_link_for_addr(struct csp_cla_config *cfg, uint8_t csp_addr)
{
    struct csp_peer *peer = find_peer_by_addr(cfg, csp_addr);
    if (peer)
    {
        log_info("link already exists for csp_addr=%u link_id=%" PRIu64, csp_addr, peer->link_id);
        return 0;
    }

    log_info("creating new link for csp_addr=%u", csp_addr);

    size_t free_idx = CSP_MAX_LINKS;
    for (size_t i = 0; i < CSP_MAX_LINKS; ++i)
    {
        if (!cfg->peers[i].used)
        {
            free_idx = i;
            break;
        }
    }

    if (free_idx == CSP_MAX_LINKS)
    {
        log_info("no free peer slots available (max=%u)", CSP_MAX_LINKS);
        return -1;
    }

    UniboBPIPN ipn = NULL;
    UniboBPError err = unibo_bp_eid_ipn_create(csp_addr, 0, &ipn);
    if (err != UniboBP_NoError)
    {
        log_bp_error("unibo_bp_eid_ipn_create", err);
        return -1;
    }

    uint64_t link_id = 0;
    err = unibo_bp_cla_create_link(
        cfg->handle,
        unibo_bp_const_cast_eid(ipn),
        true,
        true,
        &link_id);
    if (err != UniboBP_NoError)
    {
        log_bp_error("unibo_bp_cla_create_link", err);
        unibo_bp_eid_destroy((UniboBPEID *)&ipn);
        return -1;
    }

    err = unibo_bp_cla_set_link_param(cfg->handle, link_id, false, CSPCL_MAX_BUNDLE_SIZE);
    if (err != UniboBP_NoError)
    {
        log_bp_error("unibo_bp_cla_set_link_param", err);
        unibo_bp_cla_destroy_link(cfg->handle, link_id);
        unibo_bp_eid_destroy((UniboBPEID *)&ipn);
        return -1;
    }

    err = unibo_bp_cla_start_observation_planned_neighbor(
        cfg->handle,
        unibo_bp_const_cast_eid(ipn));
    if (err != UniboBP_NoError)
    {
        log_bp_error("unibo_bp_cla_start_observation_planned_neighbor", err);
    }

    err = unibo_bp_cla_open_link(cfg->handle, link_id);
    if (err != UniboBP_NoError)
    {
        log_bp_error("unibo_bp_cla_open_link", err);
        unibo_bp_cla_destroy_link(cfg->handle, link_id);
        unibo_bp_eid_destroy((UniboBPEID *)&ipn);
        return -1;
    }

    cfg->peers[free_idx].used = true;
    cfg->peers[free_idx].csp_addr = csp_addr;
    cfg->peers[free_idx].link_id = link_id;
    cfg->peers[free_idx].eid = unibo_bp_cast_eid(ipn);
    cfg->peers[free_idx].link_open = true;

    log_info("link ready csp_addr=%u link_id=%" PRIu64 " slot=%zu", csp_addr, link_id, free_idx);

    return 0;
}

static void observe_default_peers(struct csp_cla_config *cfg)
{
    unsigned observed = 0;
    for (uint8_t node = 1; node <= CSP_OBSERVE_MAX_NODE; ++node)
    {
        if (node == cfg->local_addr)
            continue;

        UniboBPIPN ipn = NULL;
        UniboBPError err = unibo_bp_eid_ipn_create(node, 0, &ipn);
        if (err != UniboBP_NoError)
            continue;

        err = unibo_bp_cla_start_observation_planned_neighbor(
            cfg->handle,
            unibo_bp_const_cast_eid(ipn));
        if (err != UniboBP_NoError)
            log_bp_error("unibo_bp_cla_start_observation_planned_neighbor", err);
        else
            observed++;

        unibo_bp_eid_destroy((UniboBPEID *)&ipn);
    }

    log_info("started planned-neighbor observation for %u peers", observed);
}

/*
 * Derived from Unibo-LTP bridge behavior: reliable-forward ECOS flag can be
 * used as one input to transmission policy decisions.
 */
static bool pdu_prefers_reliable_ltp(ConstUniboBPCLAOutboundPDU pdu)
{
    ConstUniboBPECOS ecos = unibo_bp_cla_outbound_pdu_get_ecos(pdu);
    if (!ecos)
        return false;

    return unibo_bp_ecos_get_flag(ecos, UniboBPECOSFlag_reliableForward);
}

/* ------------------------------------------------------------------------- */
/* BP callbacks                                                              */
/* ------------------------------------------------------------------------- */

static bool on_outbound_pdu(uint64_t link_id, ConstUniboBPCLAOutboundPDU pdu, void *user_arg)
{
    struct csp_cla_config *cfg = user_arg;
    if (!cfg || !pdu)
        return false;

    const uint64_t tx_id = unibo_bp_cla_outbound_pdu_get_transmission_id(pdu);
    const int fd = unibo_bp_cla_outbound_pdu_get_fd(pdu);
    const uint64_t len = unibo_bp_cla_outbound_pdu_get_size(pdu);

    log_info("outbound_pdu callback link_id=%" PRIu64 " tx_id=%" PRIu64 " len=%" PRIu64,
             link_id, tx_id, len);

    pthread_mutex_lock(&cfg->lock);
    struct csp_peer *peer = find_peer_by_link_id(cfg, link_id);
    pthread_mutex_unlock(&cfg->lock);

    if (!peer)
    {
        log_info("outbound_pdu: no peer for link_id=%" PRIu64 " tx_id=%" PRIu64, link_id, tx_id);
        (void)unibo_bp_cla_transmission_failure(cfg->handle, link_id, tx_id);
        return false;
    }

    uint8_t *buffer = malloc((size_t)len);
    if (!buffer)
    {
        log_info("outbound_pdu: malloc failed len=%" PRIu64, len);
        (void)unibo_bp_cla_transmission_failure(cfg->handle, link_id, tx_id);
        return false;
    }

    uint64_t remaining = len;
    uint8_t *cursor = buffer;
    while (remaining > 0)
    {
        const size_t chunk = (remaining > 4096U) ? 4096U : (size_t)remaining;
        const ssize_t n = read(fd, cursor, chunk);
        if (n <= 0)
        {
            log_info("outbound_pdu: read failed fd=%d errno=%d (%s)", fd, errno, strerror(errno));
            free(buffer);
            log_bp_call_error("unibo_bp_cla_transmission_failure",
                              unibo_bp_cla_transmission_failure(cfg->handle, link_id, tx_id));
            return false;
        }
        remaining -= (uint64_t)n;
        cursor += n;
    }

    (void)pdu_prefers_reliable_ltp(pdu);

    log_payload_preview("tx", buffer, (size_t)len);

    cspcl_error_t cerr = cspcl_send_bundle(&cfg->cspcl, buffer, (size_t)len, peer->csp_addr, cfg->csp_port);
    free(buffer);

    if (cerr == CSPCL_OK)
    {
        log_info("tx success tx_id=%" PRIu64 " dst_csp=%u bytes=%" PRIu64, tx_id, peer->csp_addr, len);
        log_bp_call_error("unibo_bp_cla_transmission_success",
                          unibo_bp_cla_transmission_success(cfg->handle, link_id, tx_id));
        return true;
    }

    log_info("tx failure tx_id=%" PRIu64 " dst_csp=%u cspcl_err=%d", tx_id, peer->csp_addr, (int)cerr);
    log_bp_call_error("unibo_bp_cla_transmission_failure",
                      unibo_bp_cla_transmission_failure(cfg->handle, link_id, tx_id));
    return false;
}

static void on_stop(void *user_arg)
{
    struct csp_cla_config *cfg = user_arg;
    log_info("stop callback received");
    if (cfg)
        cfg->rx_running = false;
}

static void on_update_config_flags(uint64_t enabled_flags, uint64_t disabled_flags, void *user_arg)
{
    (void)user_arg;
    log_info("update_config_flags callback: enable=0x%" PRIx64 " disable=0x%" PRIx64,
             enabled_flags, disabled_flags);
}

static void on_add_contact(
    ConstUniboBPEID neighbor,
    bool uplink,
    struct timeval *start_time,
    struct timeval *end_time,
    uint64_t xmit_rate_Bps,
    void *user_arg)
{
    (void)start_time;
    (void)end_time;
    (void)xmit_rate_Bps;

    struct csp_cla_config *cfg = user_arg;
    if (!cfg || !neighbor || !uplink)
        return;

    const uint8_t csp_addr = csp_addr_from_eid(neighbor);
    if (csp_addr == 0)
    {
        log_info("add_contact callback: could not map EID to CSP address");
        return;
    }

    log_info("add_contact callback: uplink=%d csp_addr=%u", (int)uplink, csp_addr);

    pthread_mutex_lock(&cfg->lock);
    if (ensure_link_for_addr(cfg, csp_addr) == 0)
    {
        struct csp_peer *peer = find_peer_by_addr(cfg, csp_addr);
        if (peer)
        {
            UniboBPError err = unibo_bp_cla_open_link(cfg->handle, peer->link_id);
            if (err != UniboBP_NoError)
                log_bp_error("unibo_bp_cla_open_link", err);
            else
                log_info("add_contact callback: opened link_id=%" PRIu64 " for csp_addr=%u",
                         peer->link_id, csp_addr);
        }
    }
    pthread_mutex_unlock(&cfg->lock);
}

static void on_remove_contact(
    ConstUniboBPEID neighbor,
    bool uplink,
    struct timeval *start_time,
    void *user_arg)
{
    struct csp_cla_config *cfg = user_arg;
    const uint8_t csp_addr = csp_addr_from_eid(neighbor);
    log_info("remove_contact callback: uplink=%d csp_addr=%u", (int)uplink, csp_addr);

    if (cfg)
    {
        pthread_mutex_lock(&cfg->lock);
        struct csp_peer *peer = find_peer_by_addr(cfg, csp_addr);
        if (peer)
        {
            UniboBPError err = unibo_bp_cla_close_link(cfg->handle, peer->link_id);
            if (err != UniboBP_NoError)
                log_bp_error("unibo_bp_cla_close_link", err);
            else
                log_info("remove_contact callback: closed link_id=%" PRIu64 " for csp_addr=%u",
                         peer->link_id, csp_addr);
        }
        pthread_mutex_unlock(&cfg->lock);
    }

    (void)start_time;
}

static void on_add_range(
    ConstUniboBPEID neighbor,
    bool uplink,
    struct timeval *start_time,
    struct timeval *end_time,
    uint64_t owlt_s,
    void *user_arg)
{
    const uint8_t csp_addr = csp_addr_from_eid(neighbor);
    log_info("add_range callback: uplink=%d csp_addr=%u owlt=%" PRIu64, (int)uplink, csp_addr, owlt_s);
    (void)start_time;
    (void)end_time;
    (void)user_arg;
}

static void on_remove_range(
    ConstUniboBPEID neighbor,
    bool uplink,
    struct timeval *start_time,
    void *user_arg)
{
    const uint8_t csp_addr = csp_addr_from_eid(neighbor);
    log_info("remove_range callback: uplink=%d csp_addr=%u", (int)uplink, csp_addr);
    (void)start_time;
    (void)user_arg;
}

/* ------------------------------------------------------------------------- */
/* RX worker                                                                  */
/* ------------------------------------------------------------------------- */

static void *csp_rx_task(void *arg)
{
    struct csp_cla_config *cfg = arg;
    uint8_t bundle_buffer[CSP_RX_BUFFER_SIZE];

    log_info("rx thread started");

    while (cfg->rx_running)
    {
        size_t bundle_len = sizeof(bundle_buffer);
        uint8_t src_addr = 0;
        uint8_t src_port = 0;

        cspcl_error_t err = cspcl_recv_bundle(
            &cfg->cspcl,
            bundle_buffer,
            &bundle_len,
            &src_addr,
            &src_port,
            1000);

        if (err == CSPCL_ERR_TIMEOUT)
            continue;
        if (err != CSPCL_OK)
        {
            log_info("rx: cspcl_recv_bundle error=%d", (int)err);
            continue;
        }

        log_info("rx: bundle received src_csp=%u bytes=%zu", src_addr, bundle_len);
        log_payload_preview("rx", bundle_buffer, bundle_len);

        pthread_mutex_lock(&cfg->lock);
        if (ensure_link_for_addr(cfg, src_addr) != 0)
        {
            pthread_mutex_unlock(&cfg->lock);
            continue;
        }

        struct csp_peer *peer = find_peer_by_addr(cfg, src_addr);
        uint64_t link_id = peer ? peer->link_id : 0;
        pthread_mutex_unlock(&cfg->lock);

        if (link_id == 0)
        {
            log_info("rx: no link_id for src_csp=%u", src_addr);
            continue;
        }

        int fd = unibo_bp_cla_open_file(cfg->handle);
        if (fd < 0)
        {
            log_info("rx: unibo_bp_cla_open_file failed");
            continue;
        }

        size_t remaining = bundle_len;
        const uint8_t *cursor = bundle_buffer;
        while (remaining > 0)
        {
            size_t chunk = (remaining > 4096U) ? 4096U : remaining;
            ssize_t n = write(fd, cursor, chunk);
            if (n <= 0)
            {
                log_info("rx: write failed fd=%d errno=%d (%s)", fd, errno, strerror(errno));
                break;
            }

            remaining -= (size_t)n;
            cursor += n;
        }

        if (remaining == 0)
        {
            lseek(fd, 0, SEEK_SET);
            UniboBPError in_err = unibo_bp_cla_inbound_pdu(cfg->handle, link_id, fd, bundle_len);
            if (in_err != UniboBP_NoError)
                log_bp_error("unibo_bp_cla_inbound_pdu", in_err);
            else
                log_info("rx: delivered inbound pdu link_id=%" PRIu64 " bytes=%zu", link_id, bundle_len);
        }
        else
        {
            log_info("rx: write to inbound fd incomplete, remaining=%zu", remaining);
        }

        close(fd);
    }

    log_info("rx thread exiting");

    return NULL;
}

/* ------------------------------------------------------------------------- */
/* Init / cleanup                                                             */
/* ------------------------------------------------------------------------- */

static int csp_cla_init(
    struct csp_cla_config *cfg,
    uint8_t local_addr,
    uint8_t csp_port,
    uint64_t cla_id,
    const char *bp_directory)
{
    memset(cfg, 0, sizeof(*cfg));
    cfg->local_addr = local_addr;
    cfg->csp_port = csp_port;
    cfg->cla_id = cla_id;

    log_info("csp_cla_init begin local_addr=%u csp_port=%u cla_id=%" PRIu64 " bp_dir=%s",
             local_addr, csp_port, cla_id, bp_directory ? bp_directory : "(null)");

    if (pthread_mutex_init(&cfg->lock, NULL) != 0)
    {
        log_info("pthread_mutex_init failed");
        return -1;
    }

    /* Populate cspcl struct before calling cspcl_init */
    cfg->cspcl.local_addr = local_addr;
    cfg->cspcl.csp_port = csp_port;
    cfg->cspcl.iface_type = g_iface_type;
    strncpy(cfg->cspcl.zmqhub_addr, CSP_ZMQHUB_ADDR, CSP_IFACE_PARAM_MAX - 1);
    strncpy(cfg->cspcl.can_iface, CSP_CAN_IFACE, CSP_IFACE_PARAM_MAX - 1);

    if (cspcl_init(&cfg->cspcl) != CSPCL_OK)
    {
        log_info("cspcl_init failed");
        return -1;
    }

    if (!bp_directory)
        bp_directory = ".";

    UniboBPError err = unibo_bp_cla_connect(bp_directory, &cfg->handle);
    if (err != UniboBP_NoError)
    {
        log_bp_error("unibo_bp_cla_connect", err);
        return -1;
    }

    log_info("connected to unibo-bp cla api");

    err = unibo_bp_cla_set_id(cfg->handle, cfg->cla_id);
    if (err != UniboBP_NoError)
    {
        log_bp_error("unibo_bp_cla_set_id", err);
        unibo_bp_cla_disconnect(&cfg->handle);
        return -1;
    }

    err = unibo_bp_cla_set_cla_name(cfg->handle, "CSPCLA");
    if (err != UniboBP_NoError)
    {
        log_bp_error("unibo_bp_cla_set_cla_name", err);
        unibo_bp_cla_disconnect(&cfg->handle);
        return -1;
    }

    unibo_bp_cla_set_user_arg(cfg->handle, cfg);
    unibo_bp_cla_register_callback_update_config_flags(cfg->handle, on_update_config_flags);
    unibo_bp_cla_register_callback_outbound_pdu(cfg->handle, on_outbound_pdu);
    unibo_bp_cla_register_callback_stop(cfg->handle, on_stop);
    unibo_bp_cla_register_callback_add_contact(cfg->handle, on_add_contact);
    unibo_bp_cla_register_callback_remove_contact(cfg->handle, on_remove_contact);
    unibo_bp_cla_register_callback_add_range(cfg->handle, on_add_range);
    unibo_bp_cla_register_callback_remove_range(cfg->handle, on_remove_range);
    unibo_bp_cla_register_callback_cancel_transmission(cfg->handle, NULL);
    unibo_bp_cla_register_callback_change_contact_start_time(cfg->handle, NULL);
    unibo_bp_cla_register_callback_change_contact_end_time(cfg->handle, NULL);
    unibo_bp_cla_register_callback_change_contact_xmit_rate(cfg->handle, NULL);
    unibo_bp_cla_register_callback_change_range_start_time(cfg->handle, NULL);
    unibo_bp_cla_register_callback_change_range_end_time(cfg->handle, NULL);
    unibo_bp_cla_register_callback_change_range_owlt(cfg->handle, NULL);

    observe_default_peers(cfg);

    log_info("trace settings: CSPCL_TRACE=%s CSPCLA_TRACE_PAYLOAD=%s CSPCLA_TRACE_BYTES=%s",
             getenv("CSPCL_TRACE") ? getenv("CSPCL_TRACE") : "(unset)",
             getenv("CSPCLA_TRACE_PAYLOAD") ? getenv("CSPCLA_TRACE_PAYLOAD") : "(unset)",
             getenv("CSPCLA_TRACE_BYTES") ? getenv("CSPCLA_TRACE_BYTES") : "(unset)");

    cfg->rx_running = true;
    if (pthread_create(&cfg->rx_thread, NULL, csp_rx_task, cfg) != 0)
    {
        log_info("pthread_create rx thread failed");
        cfg->rx_running = false;
        unibo_bp_cla_disconnect(&cfg->handle);
        return -1;
    }

    log_info("csp_cla_init completed");

    return 0;
}

static void csp_cla_cleanup(struct csp_cla_config *cfg)
{
    if (!cfg)
        return;

    log_info("csp_cla_cleanup begin");

    cfg->rx_running = false;
    if (cfg->rx_thread)
        pthread_join(cfg->rx_thread, NULL);

    for (size_t i = 0; i < CSP_MAX_LINKS; ++i)
    {
        if (!cfg->peers[i].used)
            continue;

        (void)unibo_bp_cla_close_link(cfg->handle, cfg->peers[i].link_id);
        if (cfg->peers[i].eid)
            (void)unibo_bp_cla_stop_observation_planned_neighbor(cfg->handle, unibo_bp_const_cast_eid(cfg->peers[i].eid));
        (void)unibo_bp_cla_destroy_link(cfg->handle, cfg->peers[i].link_id);
        if (cfg->peers[i].eid)
            unibo_bp_eid_destroy(&cfg->peers[i].eid);

        memset(&cfg->peers[i], 0, sizeof(cfg->peers[i]));
    }

    if (cfg->handle)
        unibo_bp_cla_disconnect(&cfg->handle);

    cspcl_cleanup(&cfg->cspcl);
    pthread_mutex_destroy(&cfg->lock);
    log_info("csp_cla_cleanup completed");
}

/* ------------------------------------------------------------------------- */
/* Entry point                                                                */
/* ------------------------------------------------------------------------- */

/*
 * Host-managed startup path:
 *   a host process parses config and calls this create function.
 *
 * Standalone path:
 *   compile with -DCSPCLA_STANDALONE_MAIN to enable the main() below.
 *
 * Options:
 *   <local_addr>,<csp_port>[,<iface_type>[,<cla_id>]]
 * iface_type: zmqhub | can | loopback
 */
int csp_cla_create_unibo_update(const char *const options[], size_t option_count)
{
    if (option_count < 2 || option_count > 5)
        return -1;

    uint8_t local_addr = (uint8_t)atoi(options[0]);
    uint8_t csp_port = (uint8_t)atoi(options[1]);
    uint64_t cla_id = DEFAULT_CSP_CLA_ID;
    const char *bp_directory = ".";

    if (option_count >= 3)
    {
        if (parse_iface_type(options[2], &g_iface_type) != 0)
            return -1;
    }

    if (option_count >= 4)
        cla_id = (uint64_t)strtoull(options[3], NULL, 10);

    if (option_count >= 5)
        bp_directory = options[4];

    struct csp_cla_config *cfg = malloc(sizeof(*cfg));
    if (!cfg)
        return -1;

    if (csp_cla_init(cfg, local_addr, csp_port, cla_id, bp_directory) != 0)
    {
        free(cfg);
        return -1;
    }

    log_info("host-managed create successful local_addr=%u csp_port=%u cla_id=%" PRIu64, local_addr, csp_port, cla_id);

    return 0;
}

#ifdef CSPCLA_STANDALONE_MAIN
int main(int argc, char **argv)
{
    struct cspcl_daemon_config daemon_cfg;
    const int parse_result = parse_daemon_args(argc, argv, &daemon_cfg);
    if (parse_result != 0)
    {
        if (parse_result < 0)
            print_usage(argv[0]);
        return (parse_result > 0) ? 0 : EXIT_FAILURE;
    }

    if (install_signal_handlers() != 0)
    {
        perror("sigaction");
        return EXIT_FAILURE;
    }

    log_info("signal handlers installed");

    g_iface_type = daemon_cfg.iface_type;

    struct csp_cla_config cfg;
    if (csp_cla_init(
            &cfg,
            daemon_cfg.local_addr,
            daemon_cfg.csp_port,
            daemon_cfg.cla_id,
            daemon_cfg.bp_directory) != 0)
    {
        fprintf(stderr, "[cspcla] initialization failed\n");
        return EXIT_FAILURE;
    }

    g_active_cfg = &cfg;

    fprintf(stdout,
            "[cspcla] running local_addr=%u csp_port=%u iface=%d cla_id=%" PRIu64 " bp_dir=%s\n",
            daemon_cfg.local_addr,
            daemon_cfg.csp_port,
            (int)daemon_cfg.iface_type,
            daemon_cfg.cla_id,
            daemon_cfg.bp_directory);

    while (!g_shutdown_requested && cfg.rx_running)
        sleep(1);

    log_info("main loop exiting shutdown_requested=%d rx_running=%d", (int)g_shutdown_requested, (int)cfg.rx_running);
    csp_cla_cleanup(&cfg);
    g_active_cfg = NULL;
    return EXIT_SUCCESS;
}
#endif

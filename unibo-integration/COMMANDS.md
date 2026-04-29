# CSPCL + Unibo-BP Commands (Clean Runbook)

This is the minimal, known-good sequence for end-to-end transfer over CSPCL.

## 0) Environment

```bash
# one-time
cat >> ~/.bashrc <<'EOF'
export DTN_ROOT=/home/path/to/your/dir
export CSPCL_DIR=$DTN_ROOT/cspcl
export INTEG_DIR=$CSPCL_DIR/unibo-integration
export UNIBO_BP_BIN=$DTN_ROOT/unibo-dtn/unibo-bp/build/Unibo-BP/bin
export UNIBO_BP_LIB=$DTN_ROOT/unibo-dtn/unibo-bp/build/Unibo-BP/lib
export LIBCSP_BUILD=$DTN_ROOT/libcsp/build
EOF

source ~/.bashrc
export CSPCL_C_SRC=$CSPCL_DIR/rust-bindings/cspcl-sys/c_src

# optional tracing (recommended while debugging convergence-layer failures)
export CSPCLA_TRACE_PAYLOAD=1
export CSPCLA_TRACE_BYTES=64

# required to enable ASABR routing provider in unibo-bp-cspcl
# adapter binary should be the cspcl-asabr-adapter executable
export CSPCL_ASABR_ADAPTER_BIN=$CSPCL_DIR/rust-bindings/target/release/cspcl-asabr-adapter

# use an epoch-time-compatible contact plan for live tests
# (the example plan in A-SABR/examples is relative-time and may return NO_ROUTE)
cat > /tmp/asabr-dtn-dynamic3.cp <<'EOF'
node 0 n0
node 1 n1
node 2 n2
contact 1 2 0 4000000000 evl 1000000 1
EOF
export CSPCL_ASABR_CONTACT_PLAN_PATH=/tmp/asabr-dtn-dynamic3.cp

# build adapter binary if not already built
cd "$CSPCL_DIR/rust-bindings"
cargo build --release -p cspcl-asabr-adapter

# optional: direct adapter sanity check before launching daemons
$CSPCL_ASABR_ADAPTER_BIN query \
  --cp "$CSPCL_ASABR_CONTACT_PLAN_PATH" \
  --source 1 --dest 2 --priority 0 --size 64 \
  --expiration $(($(date +%s)+3600)) --current-time $(date +%s) --timeout-ms 5000
```

## 1) Clean reset

```bash
pkill -9 -f 'unibo-bp-cspcl|zmqhub_broker.py|unibo-bp-sink' || true

cd /tmp/unibo-node1 && $UNIBO_BP_BIN/unibo-bp-admin stop || true
cd /tmp/unibo-node2 && $UNIBO_BP_BIN/unibo-bp-admin stop || true

rm -rf /tmp/unibo-node1 /tmp/unibo-node2
mkdir -p /tmp/unibo-node1 /tmp/unibo-node2

pgrep -fa 'unibo-bp-cspcl|zmqhub_broker.py|unibo-bp-sink' || true
```

## 2) Build CSPCLA daemon

```bash
cd "$INTEG_DIR"
mkdir -p build

gcc -O2 -Wall -Wextra \
  src/cspcl_daemon.c $CSPCL_C_SRC/cspcl.c \
  $CSPCL_C_SRC/cspcl_route_bridge.c $CSPCL_C_SRC/cspcl_asabr_process_provider.c \
  -o build/unibo-bp-cspcl \
  -I$CSPCL_C_SRC \
  -I$DTN_ROOT/unibo-dtn/unibo-bp/include \
  -I$DTN_ROOT/libcsp/include \
  -I$DTN_ROOT/libcsp/build/include \
  -L$UNIBO_BP_LIB \
  -Wl,-rpath,$UNIBO_BP_LIB \
  -lunibo-bp-api \
  $LIBCSP_BUILD/libcsp.a \
  -lzmq -lpthread -lm -lsocketcan
```

If your libcsp build does not include SocketCAN objects, `-lsocketcan` is optional.

Or if you want to build for CAN:

```bash
gcc -O2 -Wall -Wextra \
  src/cspcl_daemon.c $CSPCL_C_SRC/cspcl.c \
  $CSPCL_C_SRC/cspcl_route_bridge.c $CSPCL_C_SRC/cspcl_asabr_process_provider.c \
  -o build/unibo-bp-cspcl \
  -I$CSPCL_C_SRC \
  -I$DTN_ROOT/unibo-dtn/unibo-bp/include \
  -I$DTN_ROOT/libcsp/include \
  -I$DTN_ROOT/libcsp/build/include \
  -L$UNIBO_BP_LIB \
  -Wl,-rpath,$UNIBO_BP_LIB \
  -lunibo-bp-api \
  $LIBCSP_BUILD/libcsp.a \
  -lzmq -lpthread -lm \
  -lsocketcan
```

## 3) Start components (separate terminals)

### Terminal 1: broker

```bash
cd "$CSPCL_DIR"
python3 tools/zmqhub_broker.py -v
```

### Terminal 2: node1 core

```bash
cd /tmp/unibo-node1
$UNIBO_BP_BIN/unibo-bp start \
  --set-storage-size 50000000 \
  --dtn-admin dtn://a.dtn/ \
  --ipn-admin ipn:1.0 \
  --daemon
```

### Terminal 2: node2 core

```bash
cd /tmp/unibo-node2
$UNIBO_BP_BIN/unibo-bp start \
  --set-storage-size 50000000 \
  --dtn-admin dtn://b.dtn/ \
  --ipn-admin ipn:2.0 \
  --daemon
```

## 4) Configure nodes

### Terminal 3

```bash
# node1
cd /tmp/unibo-node1
$UNIBO_BP_BIN/unibo-bp-admin region home --register-node ipn:1.0 || true
$UNIBO_BP_BIN/unibo-bp-admin region home --register-node ipn:2.0 || true

REFERENCE_TIME=$($UNIBO_BP_BIN/unibo-bp-utility --get-utc-time +0)
echo "REFERENCE_TIME=$REFERENCE_TIME"

$UNIBO_BP_BIN/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:2.0 --owlt 0 --reference-time "$REFERENCE_TIME" || true
$UNIBO_BP_BIN/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:1.0 --owlt 0 --reference-time "$REFERENCE_TIME" || true

$UNIBO_BP_BIN/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:2.0 --xmit-rate 1000000 --reference-time "$REFERENCE_TIME" || true
$UNIBO_BP_BIN/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:1.0 --xmit-rate 1000000 --reference-time "$REFERENCE_TIME" || true

$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:2.55 --gateway ipn:2.0 || true
$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:2.0 --gateway ipn:2.0 || true

# node2
cd /tmp/unibo-node2
$UNIBO_BP_BIN/unibo-bp-admin region home --register-node ipn:1.0 || true
$UNIBO_BP_BIN/unibo-bp-admin region home --register-node ipn:2.0 || true

$UNIBO_BP_BIN/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:2.0 --owlt 0 --reference-time "$REFERENCE_TIME" || true
$UNIBO_BP_BIN/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:1.0 --owlt 0 --reference-time "$REFERENCE_TIME" || true

$UNIBO_BP_BIN/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:2.0 --xmit-rate 1000000 --reference-time "$REFERENCE_TIME" || true
$UNIBO_BP_BIN/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:1.0 --xmit-rate 1000000 --reference-time "$REFERENCE_TIME" || true

$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:1.55 --gateway ipn:1.0 || true
$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:1.0 --gateway ipn:1.0 || true
```

## 5) Start CSPCLA daemons + sink

On both terminals, replace `zmqhub` with `can` if you built for CAN instead of ZMQ.

### Terminal 4: CSPCLA node1

```bash
cd "$INTEG_DIR"
stdbuf -oL -eL env \
  CSPCL_ASABR_ADAPTER_BIN="$CSPCL_ASABR_ADAPTER_BIN" \
  CSPCL_ASABR_CONTACT_PLAN_PATH="$CSPCL_ASABR_CONTACT_PLAN_PATH" \
  ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1 2>&1 | tee /tmp/cspcl-node1.log
```

### Terminal 5: CSPCLA node2

```bash
cd "$INTEG_DIR"
stdbuf -oL -eL env \
  CSPCL_ASABR_ADAPTER_BIN="$CSPCL_ASABR_ADAPTER_BIN" \
  CSPCL_ASABR_CONTACT_PLAN_PATH="$CSPCL_ASABR_CONTACT_PLAN_PATH" \
  ./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2 2>&1 | tee /tmp/cspcl-node2.log
```

Sanity (any terminal):

```bash
pgrep -fa 'unibo-bp-cspcl'
# expected: exactly 2 lines

# ASABR must be enabled on both daemons
grep -E 'asabr provider enabled' /tmp/cspcl-node1.log /tmp/cspcl-node2.log
```

### Terminal 6: sink

```bash
cd /tmp/unibo-node2
$UNIBO_BP_BIN/unibo-bp-sink ipn:2.55
```

If sink throws a Unix socket "No such file or directory" error, ensure it is launched from
the node working directory (`/tmp/unibo-node2`) after `unibo-bp start` is running.

## 6) Send

### Terminal 7

```bash
cd /tmp/unibo-node1
$UNIBO_BP_BIN/unibo-bp-send --source ipn:1.55 --destination ipn:2.55 --lifetime 600000 --payload-string 'Hello via CSPCL!'
```

Expected:

- sink prints `Received ... bytes from ipn:1.xxxxx`
- payload is visible
- sender log proves ASABR route selection, for example:
  `asabr route tx_id=... diag=asabr-route-found mode=1 next_hop=2 ...`

Quick proof command:

```bash
grep -E 'asabr route|tx success' /tmp/cspcl-node1.log
```

## 7) If bundles buffer but do not send

```bash
# fallback: restart only CSPCLA daemons (keep broker + unibo-bp cores alive)
pkill -9 -f 'unibo-bp-cspcl' || true

cd "$INTEG_DIR"
nohup stdbuf -oL -eL ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1 > /tmp/cspcl-node1.log 2>&1 &
nohup stdbuf -oL -eL ./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2 > /tmp/cspcl-node2.log 2>&1 &

sleep 1
pgrep -fa 'unibo-bp-cspcl'
```

Then resend from node1.

## 9) Cleanup

```bash
pkill -9 -f 'unibo-bp-cspcl|unibo-bp-sink|zmqhub_broker.py' || true
cd /tmp/unibo-node1 && $UNIBO_BP_BIN/unibo-bp-admin stop || true
cd /tmp/unibo-node2 && $UNIBO_BP_BIN/unibo-bp-admin stop || true
```

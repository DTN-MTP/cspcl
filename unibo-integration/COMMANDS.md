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

# optional tracing (recommended while debugging convergence-layer failures)
export CSPCLA_TRACE_PAYLOAD=1
export CSPCLA_TRACE_BYTES=64
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
  src/cspcl_daemon.c ../src/cspcl.c \
  -o build/unibo-bp-cspcl \
  -I../src \
  -I$DTN_ROOT/unibo-dtn/unibo-bp/include \
  -I$DTN_ROOT/libcsp/include \
  -I$DTN_ROOT/libcsp/build/include \
  -L$UNIBO_BP_LIB \
  -Wl,-rpath,$UNIBO_BP_LIB \
  -lunibo-bp-api \
  $LIBCSP_BUILD/libcsp.a \
  -lzmq -lpthread -lm
```

Or if you want to build for CAN:

```bash
gcc -O2 -Wall -Wextra \
  src/cspcl_daemon.c ../src/cspcl.c \
  -o build/unibo-bp-cspcl \
  -I../src \
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
stdbuf -oL -eL ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1 2>&1 | tee /tmp/cspcl-node1.log
```

### Terminal 5: CSPCLA node2

```bash
cd "$INTEG_DIR"
stdbuf -oL -eL ./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2 2>&1 | tee /tmp/cspcl-node2.log
```

Sanity (any terminal):

```bash
pgrep -fa 'unibo-bp-cspcl'
# expected: exactly 2 lines
```

### Terminal 6: sink

```bash
cd /tmp/unibo-node2
$UNIBO_BP_BIN/unibo-bp-sink ipn:2.55
```

## 6) Send

### Terminal 7

```bash
cd /tmp/unibo-node1
$UNIBO_BP_BIN/unibo-bp-send --source ipn:1.55 --destination ipn:2.55 --lifetime 600000 --payload-string 'Hello via CSPCL!'
```

Expected:

- sink prints `Received ... bytes from ipn:1.xxxxx`
- payload is visible

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

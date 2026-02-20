# CSPCL + Unibo-BP Known-Good Commands

This is the cleaned, working sequence to run end-to-end bundle transfer over CSPCL.

## 0) Paths

```bash
# One-time: append variables directly to ~/.bashrc
cat >> ~/.bashrc <<'EOF'

# DTN / CSPCL / Unibo-BP environment
export DTN_ROOT=/home/light/Documents/DO5/DTN
export CSPCL_DIR=$DTN_ROOT/cspcl
export INTEG_DIR=$CSPCL_DIR/unibo-integration
export UNIBO_BP_BIN=$DTN_ROOT/unibo-dtn/unibo-bp/build/Unibo-BP/bin
export UNIBO_BP_LIB=$DTN_ROOT/unibo-dtn/unibo-bp/build/Unibo-BP/lib
export LIBCSP_BUILD=$DTN_ROOT/libcsp/build
EOF

# Apply now in current terminal
source ~/.bashrc
```

## 1) Clean restart

```bash
pkill -f 'unibo-bp-cspcl' || true
pkill -9 -f 'unibo-bp-cspcl' || true
pkill -f 'zmqhub_broker.py' || true

cd /tmp/unibo-node1 && $UNIBO_BP_BIN/unibo-bp-admin stop || true
cd /tmp/unibo-node2 && $UNIBO_BP_BIN/unibo-bp-admin stop || true

rm -rf /tmp/unibo-node1 /tmp/unibo-node2
mkdir -p /tmp/unibo-node1 /tmp/unibo-node2

# sanity: no stale CSPCLA daemons should remain here
pgrep -fa 'unibo-bp-cspcl' || true
```

## 2) Build executable

```bash
cd "$INTEG_DIR"
mkdir -p build

gcc -O2 -Wall -Wextra -DCSPCLA_STANDALONE_MAIN \
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

## 3) Terminal 1 - Start ZMQ broker

```bash
cd "$CSPCL_DIR"
python3 tools/zmqhub_broker.py -v
```

## 4) Terminal 2 - Start node1 core

```bash
cd /tmp/unibo-node1
$UNIBO_BP_BIN/unibo-bp start \
  --set-storage-size 50000000 \
  --dtn-admin dtn://a.dtn/ \
  --ipn-admin ipn:1.0 \
  --daemon
```

## 5) Terminal 3 - Start node2 core

```bash
cd /tmp/unibo-node2
$UNIBO_BP_BIN/unibo-bp start \
  --set-storage-size 50000000 \
  --dtn-admin dtn://b.dtn/ \
  --ipn-admin ipn:2.0 \
  --daemon
```

## 6) Terminal 4 - Start CSPCLA on node1

```bash
cd "$INTEG_DIR"
stdbuf -oL -eL ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1 2>&1 | tee /tmp/cspcl-node1.log
```

## 7) Terminal 5 - Start CSPCLA on node2

```bash
cd "$INTEG_DIR"
stdbuf -oL -eL ./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2 2>&1 | tee /tmp/cspcl-node2.log
```

After both are started, quick sanity in another terminal:

```bash
pgrep -fa 'unibo-bp-cspcl'
# expected: exactly 2 lines (node1 + node2)
```

## 8) Terminal 6 - Configure node1 and node2, then run sink on node2

```bash
# --- node1 config ---
cd /tmp/unibo-node1

$UNIBO_BP_BIN/unibo-bp-admin region home --register-node ipn:1.0
$UNIBO_BP_BIN/unibo-bp-admin region home --register-node ipn:2.0

REFERENCE_TIME=$($UNIBO_BP_BIN/unibo-bp-utility --get-utc-time +0)

echo "node1 REFERENCE_TIME=$REFERENCE_TIME"

$UNIBO_BP_BIN/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:2.0 --owlt 0 --reference-time "$REFERENCE_TIME"
$UNIBO_BP_BIN/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:1.0 --owlt 0 --reference-time "$REFERENCE_TIME"

$UNIBO_BP_BIN/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:2.0 --xmit-rate 1000000 --reference-time "$REFERENCE_TIME"
$UNIBO_BP_BIN/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:1.0 --xmit-rate 1000000 --reference-time "$REFERENCE_TIME"

$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:2.55 --gateway ipn:2.0
$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:2.0 --gateway ipn:2.0

# --- node2 config ---
cd /tmp/unibo-node2

$UNIBO_BP_BIN/unibo-bp-admin region home --register-node ipn:1.0
$UNIBO_BP_BIN/unibo-bp-admin region home --register-node ipn:2.0

echo "node2 REFERENCE_TIME=$REFERENCE_TIME"

$UNIBO_BP_BIN/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:2.0 --owlt 0 --reference-time "$REFERENCE_TIME"
$UNIBO_BP_BIN/unibo-bp-admin range add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:1.0 --owlt 0 --reference-time "$REFERENCE_TIME"

$UNIBO_BP_BIN/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:1.0 --receiver ipn:2.0 --xmit-rate 1000000 --reference-time "$REFERENCE_TIME"
$UNIBO_BP_BIN/unibo-bp-admin contact add --start-time +0 --end-time +3600 --sender ipn:2.0 --receiver ipn:1.0 --xmit-rate 1000000 --reference-time "$REFERENCE_TIME"


$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:1.55 --gateway ipn:1.0 || true
$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:1.0 --gateway ipn:1.0 || true

# receiver (keep this running)
$UNIBO_BP_BIN/unibo-bp-sink ipn:2.55
```

## 9) Terminal 7 - Send from node1

```bash
cd /tmp/unibo-node1
$UNIBO_BP_BIN/unibo-bp-send --destination ipn:2.55 --payload-string 'Hello via CSPCL!'
```

Expected on sink terminal:
- `Received ... bytes from ipn:1.xxxxx`
- payload string (e.g. `Hello via CSPCL!`)

## Quick checks (if needed)

```bash
pgrep -fa 'zmqhub_broker.py|unibo-bp-cspcl|/unibo-bp( |$)|unibo-bp-sink'
ss -tlnp | grep -E '(:6000|:7000)'
tail -n 120 /tmp/cspcl-node1.log
tail -n 120 /tmp/cspcl-node2.log
```

## Troubleshooting: bundles buffered but not sent/received

Symptom:
- `unibo-bp-send` returns immediately
- sink receives nothing
- storage used bytes on node1 grows
- CSPCL logs show startup/contact/range lines, but no `outbound_pdu callback ...`

Most common causes:
- contact/range windows not active (old `REFERENCE_TIME`)
- route mismatch to sink EID (`ipn:2.55` / `ipn:1.55`)
- stale CSPCLA daemon binary still running

### Fast diagnostics

```bash
# process + ports
pgrep -fa 'zmqhub_broker.py|unibo-bp-cspcl|/unibo-bp( |$)|unibo-bp-sink'
ss -tlnp | grep -E '(:6000|:7000)'

# routes actually loaded
cd /tmp/unibo-node1 && $UNIBO_BP_BIN/unibo-bp-admin routing static get --destination-table
cd /tmp/unibo-node2 && $UNIBO_BP_BIN/unibo-bp-admin routing static get --destination-table

# contact timing check (example exact start time)
cd /tmp/unibo-node1
$UNIBO_BP_BIN/unibo-bp-admin contact get \
  --sender ipn:1.0 --receiver ipn:2.0 --type scheduled \
  --start-time 2026-02-20T08:09:35Z --print-utc-time

# buffered indicator
cd /tmp/unibo-node1 && $UNIBO_BP_BIN/unibo-bp-admin storage --get-storage-info

# cspcla logs
tail -n 200 /tmp/cspcl-node1.log
tail -n 200 /tmp/cspcl-node2.log
```

If `unibo-bp-send` succeeds but node1 log still has no `outbound_pdu callback`, do this lightweight recovery:

```bash
# restart only CSPCLA daemons (keep broker + unibo-bp cores running)
pkill -9 -f 'unibo-bp-cspcl' || true

cd "$INTEG_DIR"
nohup stdbuf -oL -eL ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1 > /tmp/cspcl-node1.log 2>&1 &
nohup stdbuf -oL -eL ./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2 > /tmp/cspcl-node2.log 2>&1 &

sleep 1
pgrep -fa 'unibo-bp-cspcl'
```

Then resend and verify again.

### Exact restart/retest sequence used in debug session

```bash
# 1) stop old cspcla processes
pkill -f 'unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1' || true
pkill -f 'unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2' || true

# 2) rebuild current daemon
cd "$INTEG_DIR"
gcc -O2 -Wall -Wextra -DCSPCLA_STANDALONE_MAIN \
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

# 3) relaunch cspcla daemons with log capture
stdbuf -oL -eL ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1 > /tmp/cspcl-node1.log 2>&1 &
stdbuf -oL -eL ./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2 > /tmp/cspcl-node2.log 2>&1 &

# 4) send test bundle
cd /tmp/unibo-node1
$UNIBO_BP_BIN/unibo-bp-send \
  --source ipn:1.55 \
  --destination ipn:2.55 \
  --lifetime 600000 \
  --payload-string 'after-contact-open-fix'

# 5) verify TX and RX in cspcla logs
tail -n 260 /tmp/cspcl-node1.log
tail -n 260 /tmp/cspcl-node2.log
```

Expected after fix:
- node1: `outbound_pdu callback ...` and `tx success ...`
- node2: `rx: bundle received ...` and `rx: delivered inbound pdu ...`

## Cleanup

```bash
pkill -f 'unibo-bp-cspcl' || true
cd /tmp/unibo-node1 && $UNIBO_BP_BIN/unibo-bp-admin stop || true
cd /tmp/unibo-node2 && $UNIBO_BP_BIN/unibo-bp-admin stop || true
pkill -f 'zmqhub_broker.py' || true
```
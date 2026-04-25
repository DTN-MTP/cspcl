#!/usr/bin/env python3
"""
zmqhub_broker.py - Simple ZMQ Hub broker for libcsp ZMQHUB interface

This broker acts as a central routing point for CSP packets between
processes using the ZMQHUB interface. All CSP nodes connect to this
broker and it forwards packets to the appropriate destinations.

Usage:
    python3 zmqhub_broker.py [--pub-port 6000] [--sub-port 6001]

The broker uses two ZMQ sockets:
- XSUB socket (default port 6000): Receives packets from CSP nodes
- XPUB socket (default port 6001): Forwards packets to CSP nodes

CSP nodes connect their PUB socket to the XSUB port and their
SUB socket to the XPUB port.
"""

import argparse
import signal
import sys

try:
    import zmq
except ImportError:
    print("ERROR: pyzmq not installed. Install with: pip install pyzmq")
    sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description="ZMQ Hub broker for CSP")
    parser.add_argument(
        "--sub-port",
        type=int,
        default=6000,
        help="Port for XSUB socket (receives from nodes, libcsp default)",
    )
    parser.add_argument(
        "--pub-port",
        type=int,
        default=7000,
        help="Port for XPUB socket (sends to nodes, libcsp default)",
    )
    parser.add_argument(
        "--bind-addr",
        default="*",
        help="Address to bind (default: * for all interfaces)",
    )
    parser.add_argument(
        "-v", "--verbose", action="store_true", help="Enable verbose output"
    )
    args = parser.parse_args()
    context = zmq.Context()
    # XSUB socket receives messages from publishers (CSP nodes sending)
    # Clients connect their PUB socket to this port
    xsub = context.socket(zmq.XSUB)
    xsub_addr = f"tcp://{args.bind_addr}:{args.sub_port}"
    xsub.bind(xsub_addr)

    # XPUB socket sends messages to subscribers (CSP nodes receiving)
    # Clients connect their SUB socket to this port
    xpub = context.socket(zmq.XPUB)
    xpub_addr = f"tcp://{args.bind_addr}:{args.pub_port}"
    xpub.bind(xpub_addr)

    print(f"ZMQ Hub Broker started (libcsp zmqproxy compatible)")
    print(
        f"  XSUB (receive from nodes): {xsub_addr} (libcsp CSP_ZMQPROXY_SUBSCRIBE_PORT)"
    )
    print(
        f"  XPUB (send to nodes):      {xpub_addr} (libcsp CSP_ZMQPROXY_PUBLISH_PORT)"
    )
    print()
    print("CSP nodes should connect to:")
    print(f"  PUB  -> tcp://localhost:{args.sub_port}  (publish endpoint)")
    print(f"  SUB  <- tcp://localhost:{args.pub_port}  (subscribe endpoint)")
    print()
    print("Press Ctrl+C to stop")
    print()

    # Set up signal handler for clean shutdown
    def signal_handler(sig, frame):
        print("\nShutting down...")
        xsub.close()
        xpub.close()
        context.term()
        sys.exit(0)

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    # Create proxy (forwards messages between XSUB and XPUB)
    packet_count = 0
    try:
        # Use a poller for non-blocking operation with verbose output
        if args.verbose:
            poller = zmq.Poller()
            poller.register(xsub, zmq.POLLIN)
            poller.register(xpub, zmq.POLLIN)

            while True:
                socks = dict(poller.poll(1000))

                if xsub in socks:
                    msg = xsub.recv_multipart()
                    xpub.send_multipart(msg)
                    packet_count += 1
                    if len(msg) > 0:
                        topic = msg[0][:4] if len(msg[0]) >= 4 else msg[0]
                        print(
                            f"[{packet_count}] XSUB->XPUB: {len(msg)} parts, topic={topic.hex()}"
                        )

                if xpub in socks:
                    msg = xpub.recv_multipart()
                    xsub.send_multipart(msg)
                    if args.verbose:
                        print(f"[{packet_count}] XPUB->XSUB: subscription: {msg}")
        else:
            # Use built-in proxy for efficiency
            zmq.proxy(xsub, xpub)

    except KeyboardInterrupt:
        signal_handler(None, None)


if __name__ == "__main__":
    main()

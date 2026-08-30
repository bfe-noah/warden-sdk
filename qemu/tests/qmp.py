#!/usr/bin/env python3
"""Tiny QMP client for the qemu/ device-sim tests.

    qmp.py <socket> screendump <out.ppm>
    qmp.py <socket> tap <x> <y>      # absolute 0..32767 (virtio-tablet)
    qmp.py <socket> quit
"""
import json
import socket
import sys
import time


def rpc(sock, obj):
    sock.sendall((json.dumps(obj) + "\n").encode())
    while True:
        line = sock_file.readline()
        if not line:
            raise RuntimeError("QMP connection closed")
        msg = json.loads(line)
        if "return" in msg:
            return msg["return"]
        if "error" in msg:
            raise RuntimeError(f"QMP error: {msg['error']}")
        # asynchronous events are interleaved; skip them


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    path, cmd = sys.argv[1], sys.argv[2]
    global sock_file
    s = socket.socket(socket.AF_UNIX)
    s.connect(path)
    sock_file = s.makefile("r")
    sock_file.readline()  # greeting banner
    rpc(s, {"execute": "qmp_capabilities"})

    if cmd == "screendump":
        rpc(s, {"execute": "screendump", "arguments": {"filename": sys.argv[3]}})
    elif cmd == "tap":
        x, y = int(sys.argv[3]), int(sys.argv[4])
        press = [
            {"type": "abs", "data": {"axis": "x", "value": x}},
            {"type": "abs", "data": {"axis": "y", "value": y}},
            {"type": "btn", "data": {"down": True, "button": "left"}},
        ]
        release = [{"type": "btn", "data": {"down": False, "button": "left"}}]
        rpc(s, {"execute": "input-send-event", "arguments": {"events": press}})
        # Hold the press across several LVGL indev poll periods (33 ms each):
        # an instantaneous press+release lands inside one poll and no click
        # is ever registered.
        time.sleep(0.2)
        rpc(s, {"execute": "input-send-event", "arguments": {"events": release}})
    elif cmd == "quit":
        s.sendall(b'{"execute":"quit"}\n')
    else:
        sys.exit(f"unknown command {cmd}")


if __name__ == "__main__":
    main()

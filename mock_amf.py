#!/usr/bin/env python3
import socket
import struct
import time

print("Mock AMF starting on 127.0.0.1:38412...")
server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("127.0.0.1", 38412))
server.listen(1)
print("Waiting for gNB connection...")

while True:
    try:
        conn, addr = server.accept()
        print(f"gNB connected from {addr}")
        # Keep connection alive
        while True:
            data = conn.recv(1024)
            if not data:
                break
            print(f"Received {len(data)} bytes from gNB")
    except Exception as e:
        print(f"Error: {e}")
        time.sleep(1)

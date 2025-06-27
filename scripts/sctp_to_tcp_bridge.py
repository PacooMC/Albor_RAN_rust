#!/usr/bin/env python3
"""
SCTP to TCP Bridge for Open5GS AMF
This creates a bridge that accepts TCP connections and forwards to SCTP
Works around Docker --privileged limitations by running TCP listener
"""

import socket
import select
import threading
import struct
import time
import sys
import os
from datetime import datetime

class SCTPBridge:
    def __init__(self, tcp_addr='127.0.0.4', tcp_port=38412, 
                 sctp_addr='127.0.0.5', sctp_port=38413):
        self.tcp_addr = tcp_addr
        self.tcp_port = tcp_port
        self.sctp_addr = sctp_addr
        self.sctp_port = sctp_port
        self.running = False
        self.connections = {}
        
    def log(self, level, msg):
        timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
        print(f"[{timestamp}] [{level}] {msg}")
        
    def try_sctp_connection(self):
        """Try to create SCTP connection to real AMF"""
        try:
            # Try to import sctp module
            import sctp
            sock = sctp.sctpsocket()
            sock.connect((self.sctp_addr, self.sctp_port))
            self.log("INFO", f"Connected to real AMF via SCTP at {self.sctp_addr}:{self.sctp_port}")
            return sock
        except ImportError:
            self.log("WARN", "SCTP module not available")
            return None
        except Exception as e:
            self.log("WARN", f"Cannot connect to real AMF via SCTP: {e}")
            return None
            
    def handle_tcp_to_sctp(self, tcp_conn, tcp_addr):
        """Bridge TCP connection to SCTP"""
        conn_id = f"{tcp_addr[0]}:{tcp_addr[1]}"
        self.log("INFO", f"New TCP connection from {conn_id}")
        
        # Try to establish SCTP connection
        sctp_sock = self.try_sctp_connection()
        
        if sctp_sock:
            # Bridge mode: Forward between TCP and SCTP
            self.log("INFO", f"Bridging {conn_id} to SCTP AMF")
            
            try:
                while self.running:
                    # Use select to monitor both sockets
                    readable, _, exceptional = select.select(
                        [tcp_conn, sctp_sock], [], [tcp_conn, sctp_sock], 1.0
                    )
                    
                    if tcp_conn in readable:
                        # Forward TCP -> SCTP
                        data = tcp_conn.recv(4096)
                        if not data:
                            break
                        sctp_sock.send(data)
                        self.log("DEBUG", f"Forwarded {len(data)} bytes TCP->SCTP")
                        
                    if sctp_sock in readable:
                        # Forward SCTP -> TCP
                        data = sctp_sock.recv(4096)
                        if not data:
                            break
                        tcp_conn.send(data)
                        self.log("DEBUG", f"Forwarded {len(data)} bytes SCTP->TCP")
                        
                    if exceptional:
                        self.log("WARN", "Socket exception detected")
                        break
                        
            except Exception as e:
                self.log("ERROR", f"Bridge error: {e}")
            finally:
                sctp_sock.close()
                
        else:
            # Fallback: Act as mock AMF
            self.log("WARN", f"No SCTP available, using mock AMF for {conn_id}")
            self.handle_mock_amf(tcp_conn, tcp_addr)
            
        tcp_conn.close()
        self.log("INFO", f"Connection from {conn_id} closed")
        
    def handle_mock_amf(self, conn, addr):
        """Fallback mock AMF handler"""
        try:
            # Wait for NG Setup Request
            data = conn.recv(4096)
            if data:
                self.log("INFO", f"Received {len(data)} bytes (mock mode)")
                
                # Send mock NG Setup Response
                # This is simplified - real response would be ASN.1 encoded
                response = self.create_mock_ng_setup_response()
                conn.send(response)
                self.log("INFO", "Sent mock NG Setup Response")
                
                # Keep connection alive
                while self.running:
                    try:
                        data = conn.recv(4096)
                        if not data:
                            break
                        self.log("DEBUG", f"Received {len(data)} bytes in mock mode")
                    except socket.timeout:
                        pass
                    except:
                        break
                        
        except Exception as e:
            self.log("ERROR", f"Mock AMF error: {e}")
            
    def create_mock_ng_setup_response(self):
        """Create a basic mock NG Setup Response"""
        # Simplified response indicating success
        return b'\x20\x15\x00\x20' + b'\x00' * 32  # Mock response
        
    def start(self):
        """Start the SCTP to TCP bridge"""
        self.running = True
        
        # Create TCP listening socket
        tcp_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        tcp_sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        
        try:
            tcp_sock.bind((self.tcp_addr, self.tcp_port))
            tcp_sock.listen(5)
            self.log("INFO", f"SCTP-TCP Bridge listening on {self.tcp_addr}:{self.tcp_port}")
            
            # Check if we can connect to real AMF
            test_sock = self.try_sctp_connection()
            if test_sock:
                test_sock.close()
                self.log("INFO", "Bridge mode: Will forward to real AMF via SCTP")
            else:
                self.log("WARN", "Mock mode: No SCTP available, will act as mock AMF")
                
            while self.running:
                try:
                    tcp_sock.settimeout(1.0)
                    conn, addr = tcp_sock.accept()
                    
                    # Handle each connection in a thread
                    thread = threading.Thread(
                        target=self.handle_tcp_to_sctp,
                        args=(conn, addr)
                    )
                    thread.daemon = True
                    thread.start()
                    
                except socket.timeout:
                    continue
                except Exception as e:
                    if self.running:
                        self.log("ERROR", f"Accept error: {e}")
                        
        except Exception as e:
            self.log("ERROR", f"Failed to start bridge: {e}")
        finally:
            tcp_sock.close()
            self.log("INFO", "Bridge stopped")
            
    def stop(self):
        """Stop the bridge"""
        self.log("INFO", "Stopping SCTP-TCP Bridge...")
        self.running = False

def main():
    """Main entry point"""
    import signal
    
    # Default addresses
    tcp_addr = os.environ.get('BRIDGE_TCP_ADDR', '127.0.0.4')
    tcp_port = int(os.environ.get('BRIDGE_TCP_PORT', '38412'))
    sctp_addr = os.environ.get('BRIDGE_SCTP_ADDR', '127.0.0.5')
    sctp_port = int(os.environ.get('BRIDGE_SCTP_PORT', '38413'))
    
    # Parse command line
    if len(sys.argv) > 1:
        tcp_addr = sys.argv[1]
    if len(sys.argv) > 2:
        tcp_port = int(sys.argv[2])
    if len(sys.argv) > 3:
        sctp_addr = sys.argv[3]
    if len(sys.argv) > 4:
        sctp_port = int(sys.argv[4])
        
    bridge = SCTPBridge(tcp_addr, tcp_port, sctp_addr, sctp_port)
    
    # Handle signals
    def signal_handler(sig, frame):
        bridge.stop()
        sys.exit(0)
        
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    print("=" * 60)
    print("SCTP to TCP Bridge for Docker Environments")
    print("=" * 60)
    print(f"TCP Listen: {tcp_addr}:{tcp_port}")
    print(f"SCTP Target: {sctp_addr}:{sctp_port}")
    print("=" * 60)
    print("Press Ctrl+C to stop")
    print("")
    
    bridge.start()

if __name__ == "__main__":
    main()
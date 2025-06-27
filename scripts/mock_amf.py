#!/usr/bin/env python3
"""
Mock AMF for testing gNodeB NGAP connectivity
This provides a TCP-based mock AMF that responds to basic NGAP messages
Used when SCTP is not available in Docker environment
"""

import socket
import struct
import threading
import time
import sys
import signal
from datetime import datetime

class NGAPMessage:
    """Basic NGAP message handling"""
    
    # NGAP Procedure Codes
    NG_SETUP_REQUEST = 21
    NG_SETUP_RESPONSE = 21
    NG_SETUP_FAILURE = 21
    
    @staticmethod
    def decode_header(data):
        """Decode basic NGAP header"""
        if len(data) < 4:
            return None
        # Simplified - real NGAP uses ASN.1 encoding
        return {
            'procedure_code': data[0],
            'criticality': data[1],
            'length': struct.unpack('>H', data[2:4])[0]
        }
    
    @staticmethod
    def create_ng_setup_response():
        """Create a basic NG Setup Response message"""
        # This is a simplified response - real NGAP uses ASN.1 encoding
        # Message includes:
        # - AMF Name: "mock-amf"
        # - Relative AMF Capacity: 255
        # - Served GUAMI List: MCC=001, MNC=01, AMF Region=2, AMF Set=1, AMF Pointer=1
        response = bytearray([
            0x20, 0x15,  # successfulOutcome, NG Setup Response
            0x00, 0x2e,  # Length
            0x00, 0x00, 0x04,  # Protocol IE Container
            0x00, 0x01,  # AMF Name
            0x00, 0x0a,  # Length
            0x00, 0x08, 0x6d, 0x6f, 0x63, 0x6b, 0x2d, 0x61, 0x6d, 0x66,  # "mock-amf"
            0x00, 0x60,  # Served GUAMI List
            0x00, 0x0e,  # Length
            0x00, 0x0c,
            0x00, 0x00, 0x01,  # MCC=001
            0x01,              # MNC=01
            0x02,              # AMF Region ID=2
            0x00, 0x01,        # AMF Set ID=1
            0x00,              # AMF Pointer=0
            0x00, 0x56,  # Relative AMF Capacity
            0x40, 0x01,  # Length
            0xff         # Capacity=255
        ])
        return bytes(response)
    
    @staticmethod
    def create_ng_setup_failure(cause="Unknown PLMN"):
        """Create NG Setup Failure message"""
        cause_bytes = cause.encode('utf-8')
        response = bytearray([
            0x40, 0x15,  # unsuccessfulOutcome, NG Setup Failure
            0x00, len(cause_bytes) + 8,  # Length
            0x00, 0x00, 0x01,  # Protocol IE Container
            0x00, 0x0f,  # Cause
            0x40, len(cause_bytes) + 1,  # Length
            0x00  # Cause choice: misc
        ])
        response.extend(cause_bytes)
        return bytes(response)

class MockAMF:
    def __init__(self, addr='127.0.0.4', port=38412):
        self.addr = addr
        self.port = port
        self.running = False
        self.connections = {}
        self.lock = threading.Lock()
        
    def log(self, level, message):
        """Log with timestamp"""
        timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S.%f')[:-3]
        print(f"[{timestamp}] [{level}] {message}")
        
    def handle_ng_setup_request(self, conn, data):
        """Handle NG Setup Request from gNodeB"""
        self.log("INFO", "Processing NG Setup Request")
        
        # Parse request (simplified - real parsing would use ASN.1)
        # For now, just check if it looks like NG Setup Request
        if len(data) > 4 and data[0] == 0x00 and data[1] == 0x15:
            # Send NG Setup Response
            response = NGAPMessage.create_ng_setup_response()
            conn.send(response)
            self.log("INFO", f"Sent NG Setup Response ({len(response)} bytes)")
            return True
        else:
            # Send NG Setup Failure
            response = NGAPMessage.create_ng_setup_failure()
            conn.send(response)
            self.log("WARN", f"Sent NG Setup Failure ({len(response)} bytes)")
            return False
            
    def handle_ngap_connection(self, conn, addr):
        """Handle NGAP connection from gNodeB"""
        conn_id = f"{addr[0]}:{addr[1]}"
        self.log("INFO", f"New NGAP connection from {conn_id}")
        
        with self.lock:
            self.connections[conn_id] = conn
            
        ng_setup_complete = False
        
        try:
            while self.running:
                # Set timeout for receive
                conn.settimeout(5.0)
                
                try:
                    # Receive data
                    data = conn.recv(4096)
                    if not data:
                        self.log("INFO", f"Connection closed by {conn_id}")
                        break
                        
                    self.log("DEBUG", f"Received {len(data)} bytes from {conn_id}")
                    
                    # Handle NG Setup Request
                    if not ng_setup_complete and len(data) > 4:
                        header = NGAPMessage.decode_header(data)
                        if header and header['procedure_code'] == NGAPMessage.NG_SETUP_REQUEST:
                            ng_setup_complete = self.handle_ng_setup_request(conn, data)
                            if ng_setup_complete:
                                self.log("INFO", f"NG Setup completed with {conn_id}")
                    else:
                        # Echo keepalive or other messages
                        self.log("DEBUG", f"Echoing message from {conn_id}")
                        
                except socket.timeout:
                    # Send keepalive if needed
                    if ng_setup_complete:
                        # Simple keepalive - not real NGAP
                        conn.send(b'\x00\x00\x00\x00')
                except Exception as e:
                    self.log("ERROR", f"Error handling data from {conn_id}: {e}")
                    break
                    
        except Exception as e:
            self.log("ERROR", f"Connection error with {conn_id}: {e}")
        finally:
            with self.lock:
                if conn_id in self.connections:
                    del self.connections[conn_id]
            conn.close()
            self.log("INFO", f"Connection from {conn_id} closed")
            
    def start(self):
        """Start the mock AMF server"""
        self.running = True
        
        # Create TCP socket (not SCTP)
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        
        try:
            sock.bind((self.addr, self.port))
            sock.listen(5)
            self.log("INFO", f"Mock AMF listening on {self.addr}:{self.port} (TCP)")
            self.log("WARN", "This is a TCP mock - real AMF uses SCTP")
            
            while self.running:
                try:
                    sock.settimeout(1.0)
                    conn, addr = sock.accept()
                    
                    # Handle connection in separate thread
                    thread = threading.Thread(
                        target=self.handle_ngap_connection, 
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
            self.log("ERROR", f"Failed to start server: {e}")
        finally:
            sock.close()
            self.log("INFO", "Mock AMF stopped")
            
    def stop(self):
        """Stop the mock AMF server"""
        self.log("INFO", "Stopping Mock AMF...")
        self.running = False
        
        # Close all connections
        with self.lock:
            for conn_id, conn in self.connections.items():
                try:
                    conn.close()
                except:
                    pass
            self.connections.clear()

def signal_handler(signum, frame):
    """Handle shutdown signals"""
    print("\nReceived signal, shutting down...")
    if 'mock_amf' in globals():
        mock_amf.stop()
    sys.exit(0)

def main():
    """Main entry point"""
    # Parse command line arguments
    addr = '127.0.0.4'
    port = 38412
    
    if len(sys.argv) > 1:
        addr = sys.argv[1]
    if len(sys.argv) > 2:
        port = int(sys.argv[2])
        
    # Set up signal handlers
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    # Create and start mock AMF
    global mock_amf
    mock_amf = MockAMF(addr, port)
    
    print("=" * 60)
    print("Mock AMF for NGAP Testing")
    print("=" * 60)
    print(f"Address: {addr}")
    print(f"Port: {port}")
    print("Protocol: TCP (SCTP not available)")
    print("=" * 60)
    print("Press Ctrl+C to stop")
    print("")
    
    try:
        mock_amf.start()
    except KeyboardInterrupt:
        pass
    finally:
        mock_amf.stop()

if __name__ == "__main__":
    main()
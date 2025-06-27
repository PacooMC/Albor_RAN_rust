#!/bin/bash
# Test AMF connection

echo "Testing AMF connection..."

# Try TCP connection
if timeout 2 bash -c "echo > /dev/tcp/127.0.0.4/38412" 2>/dev/null; then
    echo "✓ TCP connection to 127.0.0.4:38412 successful"
else
    echo "✗ TCP connection failed"
fi

# Check processes
echo ""
echo "AMF-related processes:"
ps aux | grep -E "(amf|mock|bridge)" | grep -v grep

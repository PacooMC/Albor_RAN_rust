#!/bin/bash
# Build script for srsRAN Project gNodeB
# This script is executed inside the container

set -e

echo "[INFO] Building srsRAN Project gNodeB..."

# Navigate to srsRAN Project directory
cd /workspace/external_integrations/srsRAN_Project

# Create build directory if it doesn't exist
if [ ! -d "build" ]; then
    echo "[INFO] Creating build directory..."
    mkdir -p build
fi

cd build

# Configure with CMake if not already configured
if [ ! -f "CMakeCache.txt" ]; then
    echo "[INFO] Running CMake configuration..."
    cmake .. \
        -DCMAKE_BUILD_TYPE=Release \
        -DASSERT_LEVEL=PARANOID \
        -DENABLE_EXPORT=ON \
        -DENABLE_POSITION_INDEPENDENT_CODE=ON \
        -DENABLE_ZEROMQ=ON \
        -DENABLE_5G_FEATURES=ON
else
    echo "[INFO] CMake already configured, skipping..."
fi

# Build the gNodeB binary
echo "[INFO] Building gNodeB binary..."
make -j$(nproc) gnb

# Check if binary exists
if [ -f "apps/gnb/gnb" ]; then
    echo "[INFO] gNodeB binary built successfully at: $(pwd)/apps/gnb/gnb"
    
    # Create the expected directory structure for test scripts
    echo "[INFO] Creating /opt/srsran_project directory structure..."
    sudo mkdir -p /opt/srsran_project/bin
    
    # Copy the binary to the expected location
    echo "[INFO] Copying gNodeB binary to /opt/srsran_project/bin/gnb"
    sudo cp apps/gnb/gnb /opt/srsran_project/bin/gnb
    sudo chmod +x /opt/srsran_project/bin/gnb
    
    echo "[SUCCESS] srsRAN Project gNodeB ready at /opt/srsran_project/bin/gnb"
else
    echo "[ERROR] Failed to build gNodeB binary"
    exit 1
fi
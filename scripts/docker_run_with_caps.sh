#!/bin/bash
# docker_run_with_caps.sh - Run container with proper capabilities for Open5GS

# Check if container exists
if docker ps -a | grep -q albor-gnb-dev; then
    # Check if it's running
    if docker ps | grep -q albor-gnb-dev; then
        echo "Container albor-gnb-dev is already running"
    else
        echo "Starting existing container albor-gnb-dev..."
        docker start albor-gnb-dev
    fi
else
    echo "Creating new container albor-gnb-dev with capabilities..."
    docker run -d \
        --name albor-gnb-dev \
        --cap-add=NET_ADMIN \
        --cap-add=SYS_ADMIN \
        --cap-add=NET_RAW \
        --cap-add=NET_BIND_SERVICE \
        --privileged \
        -v $(pwd):/workspace \
        -w /workspace \
        albor-gnb-dev \
        tail -f /dev/null
fi

# Ensure the container has the required capabilities
docker exec albor-gnb-dev capsh --print | grep -q "cap_net_admin" || {
    echo "ERROR: Container doesn't have NET_ADMIN capability!"
    echo "Recreating container with proper capabilities..."
    docker stop albor-gnb-dev
    docker rm albor-gnb-dev
    docker run -d \
        --name albor-gnb-dev \
        --cap-add=NET_ADMIN \
        --cap-add=SYS_ADMIN \
        --cap-add=NET_RAW \
        --cap-add=NET_BIND_SERVICE \
        --privileged \
        -v $(pwd):/workspace \
        -w /workspace \
        albor-gnb-dev \
        tail -f /dev/null
}

echo "Container albor-gnb-dev is ready with proper capabilities"

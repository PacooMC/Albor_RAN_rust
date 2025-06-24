# syntax=docker/dockerfile:1
# Albor Space 5G GNodeB Development Container - BuildKit Optimized
# Uses BuildKit features for optimal caching and parallel builds

FROM ubuntu:22.04 AS base

# Set environment variables
ENV DEBIAN_FRONTEND=noninteractive \
    RUST_VERSION=stable \
    CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:/opt/srsran/bin:/opt/srsran_project/bin:/opt/open5gs/bin:$PATH \
    CCACHE_DIR=/ccache \
    CC=/usr/lib/ccache/gcc \
    CXX=/usr/lib/ccache/g++ \
    LANG=en_US.UTF-8 \
    LANGUAGE=en_US:en \
    LC_ALL=en_US.UTF-8

# Install all dependencies with cache mount for apt
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    # Build essentials
    build-essential cmake pkg-config ninja-build git curl wget vim nano \
    ca-certificates ccache locales sudo \
    # srsRAN dependencies
    libfftw3-dev libmbedtls-dev libboost-program-options-dev \
    libboost-thread-dev libboost-system-dev libzmq3-dev libconfig++-dev \
    libsctp-dev libpcsclite-dev libblas-dev liblapack-dev libyaml-cpp-dev \
    libgtest-dev libfftw3-3 libmbedtls14 libboost-program-options1.74.0 \
    libboost-thread1.74.0 libzmq5 libconfig++9v5 libsctp1 libpcsclite1 \
    libblas3 liblapack3 \
    # Tools and debugging
    gdb tcpdump iproute2 iputils-ping python3 python3-pip net-tools \
    # Open5GS dependencies
    gnupg python3-setuptools python3-wheel flex bison meson \
    libgnutls28-dev libgcrypt-dev libssl-dev libmongoc-dev libbson-dev \
    libyaml-dev libnghttp2-dev libmicrohttpd-dev libcurl4-gnutls-dev \
    libtins-dev libtalloc-dev libidn11-dev \
    # MongoDB dependencies
    lsb-release \
    && locale-gen en_US.UTF-8

# Configure ccache with proper cache mount
RUN --mount=type=cache,target=/ccache,sharing=locked \
    mkdir -p /ccache && \
    ccache -M 2G && \
    ccache -s

# Stage for building srsRAN 4G UE
FROM base AS srsran-4g-builder
RUN --mount=type=cache,target=/ccache,sharing=locked \
    cd /tmp && \
    echo "=== Building srsRAN 4G UE ===" && \
    git clone --depth 1 --single-branch --branch release_23_04 https://github.com/srsran/srsRAN_4G.git && \
    cd srsRAN_4G && mkdir build && cd build && \
    cmake .. \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_C_FLAGS_RELEASE="-O3 -march=native -DNDEBUG" \
        -DCMAKE_CXX_FLAGS_RELEASE="-O3 -march=native -DNDEBUG" \
        -DENABLE_ZEROMQ=ON \
        -DENABLE_5GNR=ON \
        -DENABLE_SRSENB=OFF \
        -DENABLE_SRSEPC=OFF \
        -DENABLE_SRSLOG_TRACING=OFF \
        -DENABLE_GUI=OFF \
        -DBUILD_STATIC=OFF \
        -DENABLE_ASAN=OFF \
        -DENABLE_TSAN=OFF \
        -DENABLE_UHD=OFF \
        -DENABLE_BLADERF=OFF \
        -DENABLE_SOAPYSDR=OFF \
        -DCMAKE_INSTALL_PREFIX=/opt/srsran && \
    make -j$(nproc) srsue && \
    make install/fast && \
    strip /opt/srsran/bin/srsue

# Stage for building srsRAN Project gNodeB
FROM base AS srsran-project-builder
RUN --mount=type=cache,target=/ccache,sharing=locked \
    cd /tmp && \
    echo "=== Building srsRAN Project gNodeB ===" && \
    git clone --depth 1 --single-branch https://github.com/srsran/srsRAN_Project.git && \
    cd srsRAN_Project && mkdir build && cd build && \
    cmake .. \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_C_FLAGS_RELEASE="-O3 -march=native -DNDEBUG" \
        -DCMAKE_CXX_FLAGS_RELEASE="-O3 -march=native -DNDEBUG" \
        -DENABLE_EXPORT=ON \
        -DENABLE_ZEROMQ=ON \
        -DENABLE_TESTS=OFF \
        -DENABLE_EXAMPLES=OFF \
        -DENABLE_FAPI=OFF \
        -DENABLE_UHD=OFF \
        -DENABLE_OFAGENT=OFF \
        -DENABLE_DPDK=OFF \
        -DBUILD_TYPE=Release \
        -GNinja && \
    ninja -j$(nproc) gnb && \
    mkdir -p /opt/srsran_project/bin && \
    cp apps/gnb/gnb /opt/srsran_project/bin/ && \
    strip /opt/srsran_project/bin/gnb && \
    chmod +x /opt/srsran_project/bin/gnb

# Stage for building Open5GS
FROM base AS open5gs-builder
RUN --mount=type=cache,target=/ccache,sharing=locked \
    cd /tmp && \
    echo "=== Building Open5GS ===" && \
    git clone --depth 1 https://github.com/open5gs/open5gs.git && \
    cd open5gs && \
    meson build --prefix=/opt/open5gs && \
    ninja -j$(nproc) -C build && \
    ninja -C build install

# Final stage - combine all builds
FROM base AS final

# Install MongoDB with cache mount
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    curl -fsSL https://pgp.mongodb.com/server-8.0.asc | \
    gpg -o /usr/share/keyrings/mongodb-server-8.0.gpg --dearmor && \
    echo "deb [ arch=amd64,arm64 signed-by=/usr/share/keyrings/mongodb-server-8.0.gpg] https://repo.mongodb.org/apt/ubuntu jammy/mongodb-org/8.0 multiverse" | \
    tee /etc/apt/sources.list.d/mongodb-org-8.0.list && \
    apt-get update && \
    apt-get install -y mongodb-org

# Install Rust with cache mount
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    for i in 1 2 3; do \
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_VERSION} && break || \
        (echo "Retry $i/3 failed, waiting..." && sleep 10); \
    done && \
    rustup component add rustfmt clippy rust-analyzer && \
    cargo install cargo-watch cargo-edit

# Copy built artifacts from parallel stages
COPY --from=srsran-4g-builder /opt/srsran /opt/srsran
COPY --from=srsran-project-builder /opt/srsran_project /opt/srsran_project
COPY --from=open5gs-builder /opt/open5gs /opt/open5gs

# Configure TUN device for Open5GS
RUN mkdir -p /etc/systemd/network && \
    echo -e "[NetDev]\nName=ogstun\nKind=tun" > /etc/systemd/network/99-open5gs.netdev && \
    echo -e "[Match]\nName=ogstun\n[Network]\nAddress=10.45.0.1/16\nAddress=2001:db8:cafe::1/48" > /etc/systemd/network/99-open5gs.network && \
    echo 'net.ipv6.conf.ogstun.disable_ipv6=0' > /etc/sysctl.d/30-open5gs.conf

# Install Python packages with cache mount
RUN --mount=type=cache,target=/root/.cache/pip,sharing=locked \
    pip3 install pyzmq numpy pyyaml

# Setup workspace and directories
WORKDIR /workspace
RUN mkdir -p /opt/reference-ue/config \
    /opt/open5gs/var/log/open5gs && \
    ldconfig

# Create non-root user
RUN groupadd -g 1000 developer && \
    useradd -m -u 1000 -g developer -s /bin/bash developer && \
    echo "developer ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers && \
    chown -R developer:developer /workspace \
    /opt/open5gs/var/log/open5gs && \
    chmod -R a+w /usr/local/cargo /usr/local/rustup

# Verify installations
RUN echo "=== Verifying installations ===" && \
    echo "srsue: $(which srsue)" && \
    echo "gnb: $(which gnb)" && \
    echo "rustc: $(which rustc)" && \
    echo "cargo: $(which cargo)" && \
    echo "open5gs-amfd: $(which open5gs-amfd)" && \
    echo "mongod: $(which mongod)" && \
    echo "Binary sizes:" && \
    ls -lh /opt/srsran/bin/srsue /opt/srsran_project/bin/gnb && \
    echo "Open5GS components:" && \
    ls /opt/open5gs/bin/

# Switch to non-root user
USER developer
SHELL ["/bin/bash", "-c"]
CMD ["/bin/bash"]
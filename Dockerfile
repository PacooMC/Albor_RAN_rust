# Albor Space 5G GNodeB Development Container - Optimized Single Stage Build
# Optimized to prevent timeouts with strategic parallelization and caching

FROM ubuntu:22.04

# Set environment variables
ENV DEBIAN_FRONTEND=noninteractive
ENV RUST_VERSION=stable
ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH=/usr/local/cargo/bin:/opt/srsran/bin:/opt/srsran_project/bin:/opt/open5gs/bin:$PATH
ENV CCACHE_DIR=/ccache
ENV CC=/usr/lib/ccache/gcc
ENV CXX=/usr/lib/ccache/g++

# Install all dependencies including ccache and Open5GS dependencies in one layer
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential cmake pkg-config ninja-build git curl wget vim nano \
    ca-certificates libfftw3-dev libmbedtls-dev libboost-program-options-dev \
    libboost-thread-dev libboost-system-dev libzmq3-dev libconfig++-dev \
    libsctp-dev libpcsclite-dev libblas-dev liblapack-dev libyaml-cpp-dev \
    libgtest-dev libfftw3-3 libmbedtls14 libboost-program-options1.74.0 \
    libboost-thread1.74.0 libzmq5 libconfig++9v5 libsctp1 libpcsclite1 \
    libblas3 liblapack3 gdb tcpdump iproute2 iputils-ping python3 python3-pip \
    sudo locales ccache \
    # Open5GS dependencies
    gnupg python3-setuptools python3-wheel flex bison meson \
    libgnutls28-dev libgcrypt-dev libssl-dev libmongoc-dev libbson-dev \
    libyaml-dev libnghttp2-dev libmicrohttpd-dev libcurl4-gnutls-dev \
    libtins-dev libtalloc-dev libidn11-dev net-tools \
    # MongoDB dependencies
    lsb-release && rm -rf /var/lib/apt/lists/*

# Generate locale
RUN locale-gen en_US.UTF-8
ENV LANG=en_US.UTF-8
ENV LANGUAGE=en_US:en
ENV LC_ALL=en_US.UTF-8

# Configure ccache
RUN mkdir -p /ccache && \
    ccache -M 2G && \
    ccache -s

# Build srsRAN 4G UE with optimized parallelization
RUN cd /tmp \
    && echo "=== Building srsRAN 4G UE ===" \
    && git clone --depth 1 --single-branch --branch release_23_04 https://github.com/srsran/srsRAN_4G.git \
    && cd srsRAN_4G && mkdir build && cd build \
    && cmake .. \
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
        -DCMAKE_INSTALL_PREFIX=/opt/srsran \
    && echo "Building srsue with optimized parallelization..." \
    && make -j4 srsue \
    && echo "Installing srsue..." \
    && make install/fast \
    && strip /opt/srsran/bin/srsue \
    && cd / \
    && rm -rf /tmp/srsRAN_4G \
    && ccache -s \
    && echo "srsue installed successfully"

# Build srsRAN Project gNodeB with git clone and optimized parallelization
RUN cd /tmp \
    && echo "=== Building srsRAN Project gNodeB ===" \
    && git clone --depth 1 --single-branch https://github.com/srsran/srsRAN_Project.git \
    && cd srsRAN_Project && mkdir build && cd build \
    && cmake .. \
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
        -GNinja \
    && echo "Building gnb binary with optimized parallelization..." \
    && ninja -j4 gnb \
    && mkdir -p /opt/srsran_project/bin \
    && cp apps/gnb/gnb /opt/srsran_project/bin/ \
    && strip /opt/srsran_project/bin/gnb \
    && chmod +x /opt/srsran_project/bin/gnb \
    && cd / \
    && rm -rf /tmp/srsRAN_Project \
    && ccache -s \
    && echo "gnb installed successfully"

# Install Rust toolchain and tools
RUN for i in 1 2 3; do \
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_VERSION} && break || \
        (echo "Retry $i/3 failed, waiting..." && sleep 10); \
    done \
    && rustup component add rustfmt clippy rust-analyzer \
    && cargo install cargo-watch cargo-edit

# Install MongoDB
RUN curl -fsSL https://pgp.mongodb.com/server-8.0.asc | \
    gpg -o /usr/share/keyrings/mongodb-server-8.0.gpg --dearmor \
    && echo "deb [ arch=amd64,arm64 signed-by=/usr/share/keyrings/mongodb-server-8.0.gpg] https://repo.mongodb.org/apt/ubuntu jammy/mongodb-org/8.0 multiverse" | \
    tee /etc/apt/sources.list.d/mongodb-org-8.0.list \
    && apt-get update \
    && apt-get install -y mongodb-org \
    && rm -rf /var/lib/apt/lists/*

# Build and install Open5GS
RUN cd /tmp \
    && echo "=== Building Open5GS ===" \
    && git clone --depth 1 https://github.com/open5gs/open5gs.git \
    && cd open5gs \
    && meson build --prefix=/opt/open5gs \
    && ninja -j4 -C build \
    && ninja -C build install \
    && ldconfig \
    && cd / \
    && rm -rf /tmp/open5gs \
    && echo "Open5GS installed successfully"

# Configure TUN device for Open5GS
RUN mkdir -p /etc/systemd/network \
    && echo -e "[NetDev]\nName=ogstun\nKind=tun" > /etc/systemd/network/99-open5gs.netdev \
    && echo -e "[Match]\nName=ogstun\n[Network]\nAddress=10.45.0.1/16\nAddress=2001:db8:cafe::1/48" > /etc/systemd/network/99-open5gs.network \
    && echo 'net.ipv6.conf.ogstun.disable_ipv6=0' > /etc/sysctl.d/30-open5gs.conf

# Create workspace and install Python packages
WORKDIR /workspace
RUN pip3 install --no-cache-dir pyzmq numpy pyyaml

# Clean up ccache to reduce image size
RUN ccache -C && rm -rf /ccache

# Setup and verify installations
RUN mkdir -p /opt/reference-ue/config \
    && mkdir -p /opt/open5gs/var/log/open5gs \
    && echo "=== Verifying installations ===" \
    && echo "srsue: $(which srsue)" \
    && echo "gnb: $(which gnb)" \
    && echo "rustc: $(which rustc)" \
    && echo "cargo: $(which cargo)" \
    && echo "open5gs-amfd: $(which open5gs-amfd)" \
    && echo "mongod: $(which mongod)" \
    && ldconfig \
    && echo "Binary sizes:" \
    && ls -lh /opt/srsran/bin/srsue /opt/srsran_project/bin/gnb \
    && echo "Open5GS components:" \
    && ls /opt/open5gs/bin/

# Create non-root user
RUN groupadd -g 1000 developer \
    && useradd -m -u 1000 -g developer -s /bin/bash developer \
    && echo "developer ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers \
    && chown -R developer:developer /workspace \
    && chown -R developer:developer /opt/open5gs/var/log/open5gs \
    && chmod -R a+w /usr/local/cargo /usr/local/rustup

# Switch to non-root user
USER developer
SHELL ["/bin/bash", "-c"]
CMD ["/bin/bash"]
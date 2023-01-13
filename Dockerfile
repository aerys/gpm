FROM registry.aerys.in/aerys/infrastructure/vendor/rust-docker/x86_64-unknown-linux-musl:1.66.0-1

ENV UPX_VERSION=4.0.0

# Install UPX
RUN cd /tmp && \
    curl -fLO https://github.com/upx/upx/releases/download/v${UPX_VERSION}/upx-${UPX_VERSION}-amd64_linux.tar.xz && \
    tar -xf upx-${UPX_VERSION}-amd64_linux.tar.xz && \
    rm -rf upx-${UPX_VERSION}-amd64_linux.tar.xz && \
    cd upx-${UPX_VERSION}-amd64_linux && \
    chmod +x upx && \
    cp ./upx /usr/bin/upx

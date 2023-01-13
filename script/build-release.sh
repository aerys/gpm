#!/bin/bash
#
# Usage: ./build-release.sh
#

set -euo pipefail

PROJECT_NAME="gpm"

case `uname -s` in
    Linux)
        echo "Building a static binary"
        docker build -t build-${PROJECT_NAME}-image .
        docker run --rm \
            -v ${PWD}:${PWD} -w ${PWD} \
            build-${PROJECT_NAME}-image \
            bash -c "
                cargo build --release --target=x86_64-unknown-linux-musl
                strip target/x86_64-unknown-linux-musl/release/${PROJECT_NAME}
                upx --best --lzma target/x86_64-unknown-linux-musl/release/${PROJECT_NAME}
            "
        docker rmi build-${PROJECT_NAME}-image
        ;;
    *)
        echo "Building standard release binaries"
        cargo build --release
        ;;
esac

#!/bin/bash

# Static Build Script for Jellyfish Client

set -e

echo "Building statically linked binary..."

# Method 1: Build with musl target (recommended for Linux)
echo "Building with musl target..."
rustup target add x86_64-unknown-linux-musl

# Set environment variables for static linking
export RUSTFLAGS="-C target-feature=+crt-static"
export CC="musl-gcc"

# Build with musl target
cargo build --target x86_64-unknown-linux-musl --release

# Method 2: Alternative build for glibc static linking
echo "Building with glibc static linking..."
RUSTFLAGS="-C target-feature=+crt-static" cargo build --target x86_64-unknown-linux-gnu --release

# Show binary sizes
echo "Binary sizes:"
if [ -f "target/x86_64-unknown-linux-musl/release/jellyfish-client" ]; then
    echo "Musl binary size: $(du -h target/x86_64-unknown-linux-musl/release/jellyfish-client | cut -f1)"
    echo "Musl binary dependencies:"
    ldd target/x86_64-unknown-linux-musl/release/jellyfish-client 2>/dev/null || echo "Statically linked (no dependencies)"
fi

if [ -f "target/x86_64-unknown-linux-gnu/release/jellyfish-client" ]; then
    echo "GNU binary size: $(du -h target/x86_64-unknown-linux-gnu/release/jellyfish-client | cut -f1)"
    echo "GNU binary dependencies:"
    ldd target/x86_64-unknown-linux-gnu/release/jellyfish-client 2>/dev/null || echo "Statically linked (no dependencies)"
fi

# Optional: Further compress with UPX
echo "Compressing with UPX..."
if command -v upx &> /dev/null; then
    if [ -f "target/x86_64-unknown-linux-musl/release/jellyfish-client" ]; then
        cp target/x86_64-unknown-linux-musl/release/jellyfish-client jellyfish-client-musl
        upx --best jellyfish-client-musl
        echo "UPX compressed musl binary size: $(du -h jellyfish-client-musl | cut -f1)"
    fi

    if [ -f "target/x86_64-unknown-linux-gnu/release/jellyfish-client" ]; then
        cp target/x86_64-unknown-linux-gnu/release/jellyfish-client jellyfish-client-gnu
        upx --best jellyfish-client-gnu
        echo "UPX compressed gnu binary size: $(du -h jellyfish-client-gnu | cut -f1)"
    fi
else
    echo "UPX not found, skipping compression"
fi

echo "Build complete!"
echo "Recommended binary for deployment: target/x86_64-unknown-linux-musl/release/jellyfish-client"
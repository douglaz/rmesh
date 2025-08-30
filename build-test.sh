#!/usr/bin/env bash
set -e

echo "🔨 Building rmesh..."

# Build in debug mode first (faster)
echo "Building debug version..."
cargo build

# Test if the binary exists and can show help
echo "Testing binary..."
./target/debug/rmesh --help

echo "✅ Build successful!"
echo ""
echo "To build the optimized static binary, run:"
echo "  cargo build --release --target x86_64-unknown-linux-musl"
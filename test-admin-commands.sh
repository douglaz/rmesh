#!/usr/bin/env bash
# Test script for rmesh admin commands
# Run this outside of Claude CLI to avoid hanging issues

set -e

PORT="${PORT:-/dev/ttyACM0}"
RMESH="./target/release/rmesh"

echo "Testing rmesh admin commands on port: $PORT"
echo "========================================"

# Build if needed
if [ ! -f "$RMESH" ]; then
    echo "Building rmesh..."
    cargo build --release
fi

echo ""
echo "1. Testing config list..."
echo "-------------------------"
$RMESH --port $PORT config list --json | jq '.' || echo "Failed"

echo ""
echo "2. Testing config get (lora.region)..."
echo "--------------------------------------"
$RMESH --port $PORT config get --key lora.region --json | jq '.' || echo "Failed"

echo ""
echo "3. Testing channel list..."
echo "--------------------------"
$RMESH --port $PORT channel list --json | jq '.' || echo "Failed"

echo ""
echo "4. Testing info metrics (with wait and request)..."
echo "--------------------------------------------------"
$RMESH --port $PORT info metrics --wait 3 --request --json 2>/dev/null | jq '.' || echo "Failed"

echo ""
echo "5. Testing info nodes..."
echo "------------------------"
$RMESH --port $PORT info nodes --json | jq '.' || echo "Failed"

echo ""
echo "All tests completed!"
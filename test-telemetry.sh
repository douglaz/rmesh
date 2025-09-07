#!/usr/bin/env bash
# Test telemetry/metrics collection

set -e

PORT="${PORT:-/dev/ttyACM0}"
RMESH="./target/release/rmesh"

echo "Testing telemetry collection..."
echo "==============================="

echo ""
echo "1. Get my node info to see local node number..."
$RMESH --port $PORT info radio --json | jq '.node_num'

echo ""
echo "2. Try requesting telemetry with verbose..."
$RMESH --port $PORT --verbose info metrics --wait 5 --request 2>&1

echo ""
echo "3. Check device state (nodes list)..."
$RMESH --port $PORT info nodes --json | jq -r '.[] | select(.id == "5c15c784") | .id + " (node " + (.num | tostring) + ")"'

echo ""
echo "4. Try without request, just wait for broadcasts..."
$RMESH --port $PORT info metrics --wait 10 --json
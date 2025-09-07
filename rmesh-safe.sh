#!/usr/bin/env bash
# Safe wrapper for rmesh that avoids Claude CLI hanging
# Uses background execution with output capture

set -e

# Create temp file for output
OUTPUT_FILE=$(mktemp /tmp/rmesh-output.XXXXXX)
ERROR_FILE=$(mktemp /tmp/rmesh-error.XXXXXX)

# Cleanup function
cleanup() {
    rm -f "$OUTPUT_FILE" "$ERROR_FILE"
}
trap cleanup EXIT

# Build the rmesh binary path
RMESH_BIN="${RMESH_BIN:-./target/release/rmesh}"

# Check if rmesh binary exists
if [ ! -f "$RMESH_BIN" ]; then
    # Build it first
    cargo build --release >/dev/null 2>&1
fi

# Run rmesh in background with output to file
"$RMESH_BIN" "$@" >"$OUTPUT_FILE" 2>"$ERROR_FILE" &
PID=$!

# Wait for process with timeout
TIMEOUT=5
COUNTER=0
while kill -0 $PID 2>/dev/null; do
    sleep 0.1
    COUNTER=$((COUNTER + 1))
    if [ $COUNTER -ge $((TIMEOUT * 10)) ]; then
        kill -9 $PID 2>/dev/null || true
        echo "Command timed out" >&2
        exit 1
    fi
done

# Output the result
cat "$OUTPUT_FILE"

# Check for errors
if [ -s "$ERROR_FILE" ]; then
    cat "$ERROR_FILE" >&2
fi
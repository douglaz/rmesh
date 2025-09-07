#!/usr/bin/env bash
# Wrapper script to run rmesh commands with timeout and proper cleanup
# This helps prevent Claude CLI from hanging

set -e

# Default timeout in seconds
TIMEOUT=${TIMEOUT:-10}

# Build the rmesh binary path
RMESH_BIN="${RMESH_BIN:-./target/release/rmesh}"

# Check if rmesh binary exists, if not use cargo run
if [ ! -f "$RMESH_BIN" ]; then
    RMESH_BIN="cargo run --release --bin rmesh --"
else
    RMESH_BIN="$RMESH_BIN"
fi

# Run the command with timeout and redirect stderr to /dev/null for cleaner output
# Use exec to replace the shell process entirely
exec timeout --preserve-status --kill-after=2 "$TIMEOUT" $RMESH_BIN "$@" 2>/dev/null
#!/usr/bin/env bash
# Ultimate isolation wrapper for rmesh to prevent Claude CLI hanging
# Uses maximum isolation techniques to completely detach from terminal

set -e

# Create temp file for output
OUTPUT_FILE=$(mktemp /tmp/rmesh-output.XXXXXX)
LOCK_FILE=$(mktemp /tmp/rmesh-lock.XXXXXX)

# Don't cleanup OUTPUT_FILE so user can read it later
cleanup() {
    rm -f "$LOCK_FILE"
}
trap cleanup EXIT

# Build if needed
RMESH_BIN="./target/release/rmesh"
if [ ! -f "$RMESH_BIN" ]; then
    cargo build --release >/dev/null 2>&1
fi

# Run with COMPLETE isolation
# 1. nohup - ignore HUP signals
# 2. setsid - new session
# 3. Close all file descriptors
# 4. Double fork to orphan process
# 5. Everything to /dev/null except saved output
(
    # First fork
    nohup setsid bash -c '
        # Close all file descriptors above 2
        for fd in /proc/$$/fd/*; do
            fd_num=$(basename "$fd")
            if [ "$fd_num" -gt 2 ] 2>/dev/null; then
                eval "exec $fd_num>&-" 2>/dev/null || true
            fi
        done
        
        # Second fork to fully orphan
        (
            # Redirect everything to /dev/null
            exec 0</dev/null
            exec 2>/dev/null
            
            # Run rmesh and save output
            '"$RMESH_BIN"' "$@" >'"$OUTPUT_FILE"' 2>&1
            
            # Signal completion
            rm -f '"$LOCK_FILE"'
        ) &
    ' -- "$@" >/dev/null 2>&1 &
) >/dev/null 2>&1

# Wait for completion with timeout (longer for complex commands)
TIMEOUT=15
COUNTER=0
while [ -f "$LOCK_FILE" ]; do
    sleep 0.1
    COUNTER=$((COUNTER + 1))
    if [ $COUNTER -ge $((TIMEOUT * 10)) ]; then
        # Try to kill any rmesh processes
        pkill -f "$RMESH_BIN" 2>/dev/null || true
        echo "Command timed out" >&2
        exit 1
    fi
done

# Don't cat the output to avoid hanging
# Just print where the output was saved
echo "Output saved to: $OUTPUT_FILE"
echo "To view: cat $OUTPUT_FILE"

for i in {0..5}; do
sleep 1
date
done

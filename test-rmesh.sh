#!/usr/bin/env bash
# Wrapper script to test rmesh without freezing terminal

# Parse arguments
RMESH_ARGS="$@"
OUTPUT_FILE="/tmp/rmesh-test-$$.log"
PID_FILE="/tmp/rmesh-test-$$.pid"

# Function to cleanup on exit
cleanup() {
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if kill -0 "$PID" 2>/dev/null; then
            echo "Stopping rmesh process (PID: $PID)..."
            kill "$PID" 2>/dev/null
        fi
        rm -f "$PID_FILE"
    fi
    rm -f "$OUTPUT_FILE"
}

# Set trap for cleanup
trap cleanup EXIT INT TERM

# Start rmesh in background with nohup
echo "Starting rmesh with args: $RMESH_ARGS"
echo "Output file: $OUTPUT_FILE"
nohup cargo run --bin rmesh -- $RMESH_ARGS > "$OUTPUT_FILE" 2>&1 &
RMESH_PID=$!
echo $RMESH_PID > "$PID_FILE"

echo "rmesh started with PID: $RMESH_PID"
echo "Watching output (press Ctrl+C to stop)..."
echo "---"

# Give it a moment to start
sleep 0.5

# Tail the output file
tail -f "$OUTPUT_FILE" &
TAIL_PID=$!

# Wait for a specific duration or until interrupted
if [[ "$1" == "--timeout" ]]; then
    TIMEOUT="$2"
    shift 2
    sleep "$TIMEOUT"
    echo "Timeout reached, stopping..."
else
    # Wait for user interrupt
    wait $TAIL_PID
fi

# Kill tail if still running
kill $TAIL_PID 2>/dev/null

# The trap will handle cleanup
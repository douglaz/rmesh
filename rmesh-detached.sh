#!/usr/bin/env bash
# Fully detached wrapper that closes all file descriptors
# This should prevent any interaction issues with Claude CLI

OUTPUT_FILE=$(mktemp /tmp/rmesh-output.XXXXXX)

# Cleanup on exit
trap "rm -f $OUTPUT_FILE" EXIT

# Build if needed
if [ ! -f ./target/release/rmesh ]; then
    cargo build --release >/dev/null 2>&1
fi

# Run completely detached with setsid and closed file descriptors
setsid ./target/release/rmesh "$@" >$OUTPUT_FILE 2>/dev/null </dev/null &

# Wait briefly for completion
sleep 1

# Output the result
if [ -f "$OUTPUT_FILE" ]; then
    cat "$OUTPUT_FILE"
fi
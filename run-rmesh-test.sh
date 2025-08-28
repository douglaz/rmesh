#!/usr/bin/env bash

# rmesh Test Runner
# Safe execution wrapper that handles terminal issues

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
BINARY="$SCRIPT_DIR/target/debug/rmesh-test"

# Check if binary exists
if [ ! -f "$BINARY" ]; then
    echo "Error: rmesh-test binary not found."
    echo "Please run: cargo build --bin rmesh-test"
    exit 1
fi

# Default options
PORT=""
VERBOSE=""
QUIET=""
NON_INTERACTIVE=""
OUTPUT=""
FORMAT="human"
USE_NOHUP=false
LOG_FILE=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -p|--port)
            PORT="--port $2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE="--verbose"
            shift
            ;;
        -q|--quiet)
            QUIET="--quiet"
            shift
            ;;
        --non-interactive)
            NON_INTERACTIVE="--non-interactive"
            shift
            ;;
        -o|--output)
            OUTPUT="--output $2"
            shift 2
            ;;
        -f|--format)
            FORMAT="$2"
            shift 2
            ;;
        --nohup)
            USE_NOHUP=true
            shift
            ;;
        --log)
            LOG_FILE="$2"
            shift 2
            ;;
        -h|--help)
            echo "rmesh Test Runner"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -p, --port <PORT>         Serial port (e.g., /dev/ttyACM0)"
            echo "  -v, --verbose             Enable verbose output"
            echo "  -q, --quiet               Suppress non-critical errors"
            echo "  --non-interactive         Disable progress bars (auto-enabled with nohup)"
            echo "  -o, --output <FILE>       Save output to file"
            echo "  -f, --format <FORMAT>     Output format (human|json|markdown)"
            echo "  --nohup                   Run with nohup in background"
            echo "  --log <FILE>              Log file for nohup output (default: hardware-test.log)"
            echo "  -h, --help                Show this help message"
            echo ""
            echo "Examples:"
            echo "  # Run interactively"
            echo "  $0 -p /dev/ttyACM0"
            echo ""
            echo "  # Run in background with nohup"
            echo "  $0 -p /dev/ttyACM0 --nohup --log test.log"
            echo ""
            echo "  # Run with JSON output"
            echo "  $0 -p /dev/ttyACM0 -f json -o results.json"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use -h or --help for usage information"
            exit 1
            ;;
    esac
done

# Build the command
CMD="$BINARY $PORT $VERBOSE $QUIET $OUTPUT --format $FORMAT"

# If using nohup or not connected to TTY, force non-interactive mode
if [ "$USE_NOHUP" = true ] || [ ! -t 0 ]; then
    CMD="$CMD --non-interactive"
    
    if [ "$USE_NOHUP" = true ]; then
        # Set default log file if not specified
        if [ -z "$LOG_FILE" ]; then
            LOG_FILE="rmesh-test-$(date +%Y%m%d-%H%M%S).log"
        fi
        
        echo "Starting hardware test in background..."
        echo "Output will be logged to: $LOG_FILE"
        echo "To monitor: tail -f $LOG_FILE"
        echo "To stop: pkill -f rmesh-test"
        echo ""
        
        # Run with nohup
        nohup $CMD > "$LOG_FILE" 2>&1 &
        PID=$!
        echo "Process started with PID: $PID"
        
        # Wait a moment to check if process started successfully
        sleep 2
        if ps -p $PID > /dev/null; then
            echo "Test is running successfully in background."
        else
            echo "Error: Process failed to start. Check $LOG_FILE for details."
            tail -10 "$LOG_FILE"
            exit 1
        fi
    else
        # Running without TTY but not with nohup
        $CMD
    fi
else
    # Normal interactive execution
    $CMD
fi
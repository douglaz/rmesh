#!/usr/bin/env python3
"""
Isolated wrapper for rmesh that prevents Claude CLI hanging.
Uses subprocess with proper cleanup and timeout.
"""

import sys
import subprocess
import signal
import os
import time

def run_rmesh(args):
    # Build the command
    rmesh_bin = "./target/release/rmesh"
    
    # Check if binary exists
    if not os.path.exists(rmesh_bin):
        # Build it
        subprocess.run(["cargo", "build", "--release"], 
                      stdout=subprocess.DEVNULL, 
                      stderr=subprocess.DEVNULL)
    
    # Run with complete isolation
    try:
        # Start the process with new session
        proc = subprocess.Popen(
            [rmesh_bin] + args,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            stdin=subprocess.DEVNULL,
            preexec_fn=os.setsid,
            text=True
        )
        
        # Wait with timeout
        try:
            output, _ = proc.communicate(timeout=5)
            print(output, end='')
        except subprocess.TimeoutExpired:
            # Kill the entire process group
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            print("Command timed out", file=sys.stderr)
            sys.exit(1)
            
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    run_rmesh(sys.argv[1:])
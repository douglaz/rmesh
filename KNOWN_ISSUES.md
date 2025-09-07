# Known Issues

## Claude CLI Hanging Issue

### Problem
When running rmesh commands through Claude CLI, the CLI occasionally hangs and requires `pkill -9 -f claude` to recover. This is NOT an issue with rmesh itself, but rather an interaction between Claude CLI's process management and the serial port handling.

### Symptoms
- Claude CLI becomes unresponsive after running rmesh commands
- The rmesh process completes successfully but Claude CLI doesn't return
- Occurs intermittently (not every time)
- Affects all rmesh commands that interact with the serial port

### Attempted Solutions
1. ✗ Timeout wrappers - Still hangs
2. ✗ Background execution - Still hangs  
3. ✗ Process detachment with setsid - Still hangs
4. ✗ Python subprocess isolation - Still hangs
5. ✗ Closing all file descriptors - Still hangs

### Root Cause
The issue appears to be related to how Claude CLI handles processes that interact with serial ports. The serial port file descriptor or terminal settings may be interfering with Claude CLI's process management.

### Workaround
Run rmesh commands directly in your terminal outside of Claude CLI:

```bash
# Test admin commands
./test-admin-commands.sh

# Or run individual commands
./target/release/rmesh --port /dev/ttyACM0 config list --json
./target/release/rmesh --port /dev/ttyACM0 channel list --json
```

### Impact
- Development and testing can continue normally outside Claude CLI
- The rmesh application itself works correctly
- This only affects interactive use within Claude CLI

### Status
- **Severity**: Medium (development inconvenience only)
- **Component**: Claude CLI interaction
- **Resolution**: Pending (may require Claude CLI update)
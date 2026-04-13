# User Testing

## Validation Surface

**Primary surface:** CLI/TUI in terminal
- Tool: `tuistory` skill for interactive TUI validation
- All TUI assertions can be tested by launching the binary with test flags or interactively

**Secondary surface:** Shell script output
- Tool: shell commands (bash -n, eval, direct execution)
- All script assertions testable by capturing stdout and pattern matching

## Test Infrastructure Flags
The `lab` binary supports these flags for deterministic testing:
- `--and-exit`: Render one TUI frame to stderr, exit 1
- `--and-keys=<sequence>`: Inject key sequence (symbolic: DOWN,ENTER,CTRL-D or raw escape codes)
- `--and-type=<text>`: Pre-fill search input buffer
- `--and-confirm=<text>`: Pre-fill confirmation dialog text
- `LAB_WIDTH=N LAB_HEIGHT=N`: Override terminal dimensions

## Spec Test Suite
Copied from Ruby repo and adapted (try→lab, TRY_→LAB_). Located at `spec/tests/`.
Runner: `bash spec/tests/runner.sh ./target/debug/lab`

## Validation Concurrency

**Surface: tuistory (CLI/TUI)**
- Machine: 48GB RAM, 12 CPU cores
- Per instance overhead: ~50MB (CLI binary + tuistory)
- Max concurrent validators: **5**
- Rationale: Lightweight CLI tool, minimal resource usage per instance

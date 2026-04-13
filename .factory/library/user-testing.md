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

**Surface: shell-command (CLI scripts/spec checks)**
- Typical workload: short-lived `lab` invocations, `bash` checks, grep/sed-like assertions
- Per instance overhead: low (<150MB including spawned shell + lab process)
- Max concurrent validators: **4**
- Rationale: Host has active background apps; limiting to 4 keeps contention low while still parallelizing.

**Surface: cargo-test (unit assertion checks)**
- Typical workload: `cargo test` subsets, single-threaded assertion verification commands
- Per instance overhead: medium (build cache + rust test process)
- Max concurrent validators: **2**
- Rationale: Cargo test processes can compete on CPU/disk; keep low concurrency for stable runtimes.

## Flow Validator Guidance: shell-command

- Use only repo paths under `/Users/js/code/rust/lab` and mission evidence path under `/Users/js/.factory/missions/4e064f1d-55c5-4e80-9c43-b2e62ac80846/evidence/cli-foundation/<group-id>/`.
- Isolate with unique temp directories per assertion/group (`mktemp -d`) and avoid writing to shared fixed `/tmp` paths.
- Do not mutate global shell rc files outside temp HOME overrides unless the assertion explicitly requires install behavior; for install checks, set `HOME` to a group-specific temp directory.
- Capture command outputs and exit codes in each flow report with assertion-level mapping.
- Avoid destructive commands outside test temp directories.

## Flow Validator Guidance: cargo-test

- Run targeted test selectors for assigned assertions; avoid full-suite reruns unless needed for triage.
- Reuse repo build artifacts (`target/`) but do not modify source files.
- Record exact test command, failing/passing cases, and exit code per assertion.
- Keep execution within `/Users/js/code/rust/lab`; no network or external service dependencies are required.

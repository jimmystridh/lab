---
name: rust-worker
description: Implements Rust features for the lab CLI/TUI tool with TDD and spec test verification
---

# Rust Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the WORK PROCEDURE.

## When to Use This Skill

All implementation features for the `lab` Rust CLI/TUI tool — CLI commands, fuzzy matching, TUI rendering, TUI actions, and spec test adaptation.

## Precedence Rules

1. When the validation contract assertions conflict with Ruby reference behavior, implement to the **validation contract** first. Note any divergence in your handoff.
2. For scaffolding/setup features where spec tests are expected to fail initially, you may skip spec test runs but must explicitly note "spec tests skipped — expected to fail at this stage" in verification.

## Required Skills

None. All verification uses cargo commands and shell scripts.

## Work Procedure

### 1. Understand the Feature

Read the feature description, preconditions, expectedBehavior, and verificationSteps from features.json. Read the referenced validation contract assertions (fulfills IDs) to understand the exact behavioral contract.

Read `AGENTS.md` for coding conventions and boundaries. Read `.factory/library/architecture.md` for the module layout and design decisions.

### 2. Reference the Ruby Implementation

For behavioral clarity, read the relevant Ruby source files:
- `/Users/js/code/3rd/try/try.rb` — main CLI, command dispatch, TUI selector
- `/Users/js/code/3rd/try/lib/tui.rb` — TUI rendering framework
- `/Users/js/code/3rd/try/lib/fuzzy.rb` — fuzzy matching engine
- `/Users/js/code/3rd/try/spec/` — specs and test files

Understand the EXACT behavior before implementing. The Ruby code is the ground truth.

### 3. Write Tests First (TDD)

**Rust unit tests:** Write `#[cfg(test)] mod tests` in the implementation file with test cases covering:
- Happy path
- Edge cases (empty input, boundary values, special characters)
- Error paths

Run `cargo test` to confirm tests fail (red).

### 4. Implement

Write the Rust implementation. Follow the module layout in architecture.md. Key principles:
- Use `?` for error propagation, minimize `unwrap()`
- TUI output to stderr, scripts to stdout
- Match Ruby behavior exactly (same output format, same exit codes, same edge case handling)
- Use ratatui widgets and crossterm events for TUI features

### 5. Make Tests Pass (Green)

Run `cargo test` — all tests should pass. Fix implementation until they do.

### 6. Run Validators

```bash
cargo check           # typecheck
cargo clippy -- -D warnings  # lint
cargo test            # unit tests
```

Fix any errors or warnings before proceeding.

### 7. Verify with Spec Tests

If the feature has corresponding spec tests in `spec/tests/`, run only the spec tests listed in that feature's `verificationSteps`.

Use the full `bash spec/tests/runner.sh ./target/debug/lab` suite only when the feature explicitly calls for it or when the milestone should already satisfy all earlier assertions. Failures from later-milestone features should not block a correctly scoped feature.

If the feature has corresponding spec tests and the verificationSteps explicitly call for the full runner:
```bash
cargo build && bash spec/tests/runner.sh ./target/debug/lab
```

If spec tests don't exist yet (early features), or the full runner is intentionally not applicable yet, note this explicitly in the handoff.

### 8. Manual Verification

For CLI features: Run the command manually and verify output matches expected format.
For TUI features: Run with `--and-exit` and/or `--and-keys` to verify rendering and behavior.

Example:
```bash
# Verify help output
./target/debug/lab --help 2>&1
# Verify TUI renders
LAB_PATH=/tmp/test-labs ./target/debug/lab --and-exit exec 2>&1
# Verify key injection
LAB_PATH=/tmp/test-labs ./target/debug/lab --and-keys="DOWN,ENTER" exec 2>/dev/null
```

## Example Handoff

```json
{
  "salientSummary": "Implemented fuzzy matching engine with subsequence matching, word-boundary/proximity/density/length scoring. 14 unit tests pass covering all scoring components. Spec test_10_fuzzy.sh passes (6/6 assertions). Manually verified case-insensitive matching and score ordering with --and-exit.",
  "whatWasImplemented": "src/fuzzy.rs: Fuzzy struct with calculate_match() returning (score, highlight_positions). Supports case-insensitive subsequence matching with +1.0 per char, +1.0 word boundary, +2.0/sqrt(gap+1) proximity, density and length multipliers. Precomputed sqrt table for gaps 0-63. Integrated with entries.rs for base_score calculation.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      { "command": "cargo test -- fuzzy", "exitCode": 0, "observation": "14 tests pass including edge cases" },
      { "command": "cargo clippy -- -D warnings", "exitCode": 0, "observation": "No warnings" },
      { "command": "bash spec/tests/runner.sh ./target/debug/lab", "exitCode": 0, "observation": "test_10_fuzzy: 6 pass, 0 fail" }
    ],
    "interactiveChecks": [
      { "action": "Run lab --and-exit with test dirs containing alpha/beta/gamma", "observed": "All 3 entries displayed sorted by recency, scores shown as N.N format" },
      { "action": "Run lab --and-keys='bet' --and-exit to test fuzzy filter", "observed": "Only beta entry shown, 'bet' characters highlighted in bold yellow" }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "src/fuzzy.rs",
        "cases": [
          { "name": "test_case_insensitive", "verifies": "BETA matches beta entry" },
          { "name": "test_subsequence_match", "verifies": "gam matches gamma" },
          { "name": "test_no_match", "verifies": "xyz returns None" },
          { "name": "test_word_boundary_bonus", "verifies": "position 0 and after-hyphen get +1.0" },
          { "name": "test_proximity_bonus", "verifies": "consecutive matches score higher" }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- Feature depends on a module/struct that doesn't exist yet and isn't part of this feature's scope
- The Ruby behavior is genuinely ambiguous and cannot be determined from code reading
- Spec tests reveal a design conflict between Ruby behavior and the Rust architecture
- A dependency (crate) is needed that isn't in Cargo.toml and wasn't anticipated

# Environment

## Required Tools
- Rust toolchain: rustc 1.94.0-nightly, cargo
- bash 5.3.3 (for spec tests)
- zsh 5.9 (for shell eval tests)
- tmux 3.5a (for tmux-based TUI tests)
- fish: NOT installed (skip fish-specific tests)

## Environment Variables
- `LAB_PATH`: Root directory for labs (default: ~/src/labs)
- `LAB_PROJECTS`: Graduate destination (default: parent of LAB_PATH)
- `NO_COLOR`: Disable ANSI colors when set to any non-empty value
- `LAB_WIDTH`: Override terminal width (for testing)
- `LAB_HEIGHT`: Override terminal height (for testing)

## Reference Implementation
The Ruby source at `/Users/js/code/3rd/try/` is the canonical reference:
- `try.rb`: Main CLI/TUI (single file)
- `lib/tui.rb`: TUI rendering framework
- `lib/fuzzy.rb`: Fuzzy matching engine
- `spec/`: Specs and shell-based test suite

Workers should read the Ruby source when behavior is ambiguous. The Ruby implementation is the ground truth.

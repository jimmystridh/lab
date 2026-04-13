# Basic compliance tests: --help, --version
# Spec: command_line.md (Global Options)

section "basic"

# Test --help
output=$(lab_run --help 2>&1)
if echo "$output" | grep -q "ephemeral workspace manager"; then
    pass
else
    fail "--help missing expected text" "contains 'ephemeral workspace manager'" "$output" "command_line.md"
fi

# Test -h
output=$(lab_run -h 2>&1)
if echo "$output" | grep -q "ephemeral workspace manager"; then
    pass
else
    fail "-h missing expected text" "contains 'ephemeral workspace manager'" "$output" "command_line.md"
fi

# Test --version
output=$(lab_run --version 2>&1)
if echo "$output" | grep -qE "^lab [0-9]+\.[0-9]+"; then
    pass
else
    fail "--version format incorrect" "lab X.Y.Z" "$output" "command_line.md"
fi

# Test -v
output=$(lab_run -v 2>&1)
if echo "$output" | grep -qE "^lab [0-9]+\.[0-9]+"; then
    pass
else
    fail "-v format incorrect" "lab X.Y.Z" "$output" "command_line.md"
fi

# Test unknown argument is treated as search query (opens TUI)
# This matches "lab [query]" behavior - any non-command is a search term
output=$(lab_run --and-exit unknownquery 2>&1)
if echo "$output" | grep -qi "selector\|search\|cancelled"; then
    pass
else
    fail "unknown arg should open TUI as search query" "TUI output or Cancelled" "$output" "command_line.md"
fi

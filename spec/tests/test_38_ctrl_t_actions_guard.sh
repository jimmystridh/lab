# Ctrl-T immediate create and create-new action guard tests

section "ctrl-t-guards"

TODAY=$(date +%Y-%m-%d)

# Test: Ctrl-T uses the current search text immediately and normalizes spaces.
expected_path="$TEST_LABS/$TODAY-quick-test"
output=$(lab_run --path="$TEST_LABS" --and-keys="TYPE=quick test,CTRL-T" exec 2>/dev/null)
if echo "$output" | grep -Fq "mkdir -p '$expected_path'" &&
   echo "$output" | grep -Fq "cd '$expected_path'"; then
    pass
else
    fail "Ctrl-T should emit mkdir + cd using the current search text" "mkdir -p '$expected_path' ... cd '$expected_path'" "$output" "test_spec.md"
fi

# Test: Ctrl-T always uses plain mkdir, even when the labs root is git-backed.
git_labs=$(mktemp -d)
mkdir -p "$git_labs/.git"
expected_git_path="$git_labs/$TODAY-quicktest"
output=$(lab_run --path="$git_labs" --and-keys="TYPE=quicktest,CTRL-T" exec 2>/dev/null)
if echo "$output" | grep -Fq "mkdir -p '$expected_git_path'" &&
   ! echo "$output" | grep -q "worktree add"; then
    pass
else
    fail "Ctrl-T in a git-backed labs root should still use plain mkdir" "mkdir -p '$expected_git_path' without worktree add" "$output" "test_spec.md"
fi

# Test helpers for Ctrl-R / Ctrl-G / Ctrl-D on the create-new row.
guard_dir=$(mktemp -d)
mkdir -p "$guard_dir/2025-11-01-alpha"

assert_create_new_guard() {
    local key_sequence="$1"
    local blocked_text="$2"
    local label="$3"
    local output

    output=$(lab_run --path="$guard_dir" --and-exit --and-keys="TYPE=alp,DOWN,${key_sequence},TYPE=z" exec 2>&1)
    if echo "$output" | grep -Fq "Create new: $TODAY-alpz" &&
       ! echo "$output" | grep -Fq "$blocked_text"; then
        pass
    else
        fail "$label on create-new should do nothing" "Create new: $TODAY-alpz without $blocked_text" "$output" "test_spec.md"
    fi
}

# Test: Ctrl-D on create-new does not enter delete mode.
assert_create_new_guard "CTRL-D" "DELETE MODE" "Ctrl-D"

# Test: Ctrl-R on create-new does not open rename mode.
assert_create_new_guard "CTRL-R" "Rename directory" "Ctrl-R"

# Test: Ctrl-G on create-new does not open graduate mode.
assert_create_new_guard "CTRL-G" "Graduate lab to project" "Ctrl-G"

rm -rf "$git_labs" "$guard_dir"

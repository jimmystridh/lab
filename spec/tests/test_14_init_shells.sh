# Init command shell function tests
# Spec: init_spec.md

section "init-shells"

# Test: init with bash shell emits bash function
output=$(SHELL=/bin/bash lab_run init "$TEST_LABS" 2>&1)
if echo "$output" | grep -q "lab() {"; then
    pass
else
    fail "init should emit bash function" "lab() {" "$output" "init_spec.md"
fi

# Test: bash function includes --path argument with the specified path
if echo "$output" | grep -qF -- "--path '$TEST_LABS'"; then
    pass
else
    fail "bash function should include --path with specified path" "--path '$TEST_LABS'" "$output" "init_spec.md"
fi

# Test: init with fish shell emits fish function
output=$(SHELL=/usr/bin/fish lab_run init "$TEST_LABS" 2>&1)
if echo "$output" | grep -q "function lab"; then
    pass
else
    fail "init with fish should emit fish function" "function lab" "$output" "init_spec.md"
fi

# Test: init output contains the real, full path to lab binary
output=$(SHELL=/bin/bash lab_run init "$TEST_LABS" 2>&1)
if echo "$output" | grep -qF "$LAB_BIN_PATH"; then
    pass
else
    fail "init should contain real, full path to lab binary" "$LAB_BIN_PATH" "$output" "init_spec.md"
fi

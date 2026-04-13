# Vim-style navigation tests
# Spec: tui_spec.md (Keyboard Input)
#
# In lab, Ctrl-J and Ctrl-N move down while Ctrl-P moves up.
# Ctrl-K is reserved for kill-to-end line editing and is covered elsewhere.

section "vim-nav"

# Test: Ctrl-J navigates down (vim-style)
first=$(lab_run --path="$TEST_LABS" --and-keys='ENTER' exec 2>/dev/null | grep "^[[:space:]]*cd '" | head -1)
ctrl_j=$(lab_run --path="$TEST_LABS" --and-keys='CTRL-J,ENTER' exec 2>/dev/null | grep "^[[:space:]]*cd '" | head -1)
if [ -n "$ctrl_j" ] && [ "$first" != "$ctrl_j" ]; then
    pass
else
    fail "Ctrl-J should navigate down" "different cd path than the default selection" "first: $first, ctrl_j: $ctrl_j" "tui_spec.md#keyboard-input"
fi

# Test: Ctrl-N navigates down (emacs-style)
ctrl_n=$(lab_run --path="$TEST_LABS" --and-keys='CTRL-N,ENTER' exec 2>/dev/null | grep "^[[:space:]]*cd '" | head -1)
if [ -n "$ctrl_n" ] && [ "$first" != "$ctrl_n" ]; then
    pass
else
    fail "Ctrl-N should navigate down" "different cd path than the default selection" "first: $first, ctrl_n: $ctrl_n" "tui_spec.md#keyboard-input"
fi

# Test: Ctrl-J and Ctrl-N should land on the same item
if [ "$ctrl_j" = "$ctrl_n" ] && [ -n "$ctrl_j" ]; then
    pass
else
    fail "Ctrl-J and Ctrl-N should be equivalent" "same cd path" "ctrl_j: $ctrl_j, ctrl_n: $ctrl_n" "tui_spec.md#keyboard-input"
fi

# Test: Ctrl-J then Ctrl-P returns to the starting selection
round_trip_j=$(lab_run --path="$TEST_LABS" --and-keys='CTRL-J,CTRL-P,ENTER' exec 2>/dev/null | grep "^[[:space:]]*cd '" | head -1)
if [ "$first" = "$round_trip_j" ]; then
    pass
else
    fail "Ctrl-P should navigate up after Ctrl-J" "same cd path as the default selection" "first: $first, round_trip_j: $round_trip_j" "tui_spec.md#keyboard-input"
fi

# Test: Ctrl-N then Ctrl-P returns to the starting selection
round_trip_n=$(lab_run --path="$TEST_LABS" --and-keys='CTRL-N,CTRL-P,ENTER' exec 2>/dev/null | grep "^[[:space:]]*cd '" | head -1)
if [ "$first" = "$round_trip_n" ]; then
    pass
else
    fail "Ctrl-P should navigate up after Ctrl-N" "same cd path as the default selection" "first: $first, round_trip_n: $round_trip_n" "tui_spec.md#keyboard-input"
fi

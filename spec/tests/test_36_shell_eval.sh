# Shell eval integration tests
# Verify that `lab init` output can be evaluated by real shells
# and that the resulting `lab` function actually works end-to-end.
# Requires: nix-shell (for fish)

section "shell-eval"

EVAL_DIR=$(mktemp -d)
mkdir -p "$EVAL_DIR/2025-01-01-hello"

# --- bash ---

# Test: bash can eval the init output and defines lab function
bash_out=$(bash -c "
  eval \"\$('$LAB_BIN_PATH' init --path '$EVAL_DIR')\"
  type lab
" 2>&1)
if echo "$bash_out" | grep -q "lab.*function"; then
    pass
else
    fail "bash should define lab function from init" "lab is a function" "$bash_out" "init_spec.md"
fi

# Test: bash can eval a lab exec script (select first item)
bash_out=$(bash -c "
  script=\$('$LAB_BIN_PATH' exec --path '$EVAL_DIR' --and-keys 'ENTER' 2>/dev/null)
  echo \"\$script\"
" 2>&1)
if echo "$bash_out" | grep -q "2025-01-01-hello"; then
    pass
else
    fail "bash lab exec should produce script with directory" "2025-01-01-hello" "$bash_out" "init_spec.md"
fi

# --- zsh ---

if command -v zsh >/dev/null 2>&1; then
    # Test: zsh can eval the init output and defines lab function
    zsh_out=$(zsh -c "
      eval \"\$('$LAB_BIN_PATH' init --path '$EVAL_DIR')\"
      whence -w lab
    " 2>&1)
    if echo "$zsh_out" | grep -q "lab.*function"; then
        pass
    else
        fail "zsh should define lab function from init" "lab: function" "$zsh_out" "init_spec.md"
    fi

    # Test: zsh can eval a lab exec script (select first item)
    zsh_out=$(zsh -c "
      script=\$('$LAB_BIN_PATH' exec --path '$EVAL_DIR' --and-keys 'ENTER' 2>/dev/null)
      echo \"\$script\"
    " 2>&1)
    if echo "$zsh_out" | grep -q "2025-01-01-hello"; then
        pass
    else
        fail "zsh lab exec should produce script with directory" "2025-01-01-hello" "$zsh_out" "init_spec.md"
    fi
else
    pass  # skip
    pass  # skip
fi

# --- fish (via nix-shell) ---

if command -v nix-shell >/dev/null 2>&1; then
    # Test: fish can eval the init output and defines lab function
    fish_out=$(nix-shell -p fish --run "SHELL=fish fish -c 'eval ($LAB_BIN_PATH init --path $EVAL_DIR | string collect); type lab'" 2>&1)
    if echo "$fish_out" | grep -qi "lab is a function\|function lab"; then
        pass
    else
        fail "fish should define lab function from init" "lab is a function" "$fish_out" "init_spec.md"
    fi

    # Test: fish can capture a lab exec script (select first item)
    fish_out=$(nix-shell -p fish --run "SHELL=fish fish -c 'set out ($LAB_BIN_PATH exec --path $EVAL_DIR --and-keys ENTER 2>/dev/null | string collect); echo \$out'" 2>&1)
    if echo "$fish_out" | grep -q "2025-01-01-hello"; then
        pass
    else
        fail "fish lab exec should produce script with directory" "2025-01-01-hello" "$fish_out" "init_spec.md"
    fi

    # Test: fish init output is valid fish syntax (no parse errors)
    fish_syntax=$(SHELL=fish "$LAB_BIN_PATH" init --path "$EVAL_DIR" 2>&1)
    fish_parse=$(echo "$fish_syntax" | nix-shell -p fish --run "fish --no-execute" 2>&1)
    if [ $? -eq 0 ]; then
        pass
    else
        fail "fish init output should be valid fish syntax" "no syntax errors" "$fish_parse" "init_spec.md"
    fi

    # Test: fish init does NOT contain bash-isms like $() or $?
    if echo "$fish_syntax" | grep -qE '\$\(|\$\?'; then
        fail "fish init should not contain bash-isms (\$() or \$?)" "no \$() or \$?" "$fish_syntax" "init_spec.md"
    else
        pass
    fi
else
    pass  # skip
    pass  # skip
    pass  # skip
    pass  # skip
fi

rm -rf "$EVAL_DIR"

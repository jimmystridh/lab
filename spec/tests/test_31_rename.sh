# Rename mode tests
# Spec: Ctrl-R renames the selected entry

section "rename"

strip_ansi() {
    sed 's/\x1b\[[0-9;]*[a-zA-Z]//g' | sed 's/\x1b\[[?][0-9]*[a-zA-Z]//g'
}

REN_TEST_DIR=$(mktemp -d)
mkdir -p "$REN_TEST_DIR/2025-11-01-myproject"
mkdir -p "$REN_TEST_DIR/2025-11-02-coolproject"
mkdir -p "$REN_TEST_DIR/nodate-project"
touch -t 202511010000 "$REN_TEST_DIR/2025-11-01-myproject"
touch -t 202511020000 "$REN_TEST_DIR/2025-11-02-coolproject"
touch "$REN_TEST_DIR/nodate-project"

# Footer still advertises rename in the normal selector view
output=$(lab_run --path="$REN_TEST_DIR" --and-exit exec 2>&1)
if echo "$output" | strip_ansi | grep -qE '(\^R|Ctrl-R).*Rename'; then
    pass
else
    fail "Footer should show rename hint" "^R: Rename or Ctrl-R: Rename" "$output" "rename"
fi

# Esc inside rename cancels without emitting an mv script
output=$(lab_run --path="$REN_TEST_DIR" --and-keys='CTRL-R,ESC' exec 2>&1)
if [ -z "$output" ] || ! echo "$output" | grep -q "mv "; then
    pass
else
    fail "Ctrl-R then Esc should cancel rename" "no mv command" "$output" "rename"
fi

# Same-name rename is a no-op
output=$(lab_run --path="$REN_TEST_DIR" --and-keys='DOWN,CTRL-R,ENTER' exec 2>&1)
if [ -z "$output" ] || ! echo "$output" | grep -q "mv "; then
    pass
else
    fail "Rename with same name should exit without mv" "no mv command" "$output" "rename"
fi

# Clearing the prefilled input and typing a new name emits the rename script
output=$(lab_run --path="$REN_TEST_DIR" --and-keys='CTRL-R,CTRL-A,CTRL-K,TYPE=newname,ENTER' exec 2>&1)
if echo "$output" | grep -q "mv " &&
   echo "$output" | grep -q "nodate-project" &&
   echo "$output" | grep -q "newname" &&
   echo "$output" | grep -q "cd '$REN_TEST_DIR'" &&
   echo "$output" | grep -q "echo '$REN_TEST_DIR/newname'" &&
   echo "$output" | grep -q "cd '$REN_TEST_DIR/newname'"; then
    pass
else
    fail "Rename script should cd into base, mv old->new, echo new path, and cd new path" "rename script for nodate-project -> newname" "$output" "rename"
fi

# Slash is allowed into the buffer but rejected on submit
output=$(lab_run --path="$REN_TEST_DIR" --and-keys='CTRL-R,CTRL-A,CTRL-K,TYPE=../etc,ENTER,ESC' exec 2>&1)
if [ -z "$output" ] || ! echo "$output" | grep -q "mv "; then
    pass
else
    fail "Rename should reject slash-containing names" "no mv command" "$output" "rename"
fi

# Backspace edits the prefilled rename input at the cursor end
output=$(lab_run --path="$REN_TEST_DIR" --and-keys='DOWN,CTRL-R,BACKSPACE,n,e,w,ENTER' exec 2>&1)
if echo "$output" | grep -q "mv '2025-11-02-coolproject' '2025-11-02-coolprojecnew'"; then
    pass
else
    fail "Backspace should edit the prefilled rename input" "mv old name to coolprojecnew" "$output" "rename"
fi

# Navigation before rename targets the selected entry
output=$(lab_run --path="$REN_TEST_DIR" --and-keys='DOWN,CTRL-R,CTRL-A,CTRL-K,TYPE=renamed,ENTER' exec 2>&1)
if echo "$output" | grep -q "mv '2025-11-02-coolproject' 'renamed'"; then
    pass
else
    fail "Rename should target the navigated entry" "mv coolproject to renamed" "$output" "rename"
fi

rm -rf "$REN_TEST_DIR"

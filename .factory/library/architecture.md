# Architecture: lab (Rust port of try)

## Overview

`lab` is a single-binary Rust CLI/TUI tool for managing ephemeral workspace directories ("labs"). It replaces the Ruby `try` tool with identical behavior but renamed environment variables and branding.

## Binary Structure

Single binary: `lab`. All functionality in one crate, no workspace.

## Module Layout

```
src/
├── main.rs           # Entry point, CLI dispatch (match on command)
├── cli.rs            # clap argument definitions
├── commands/
│   ├── mod.rs
│   ├── init.rs       # `lab init` - emit shell wrapper function
│   ├── install.rs    # `lab install` - append init to RC file
│   ├── clone.rs      # `lab clone` - git clone into dated dir
│   ├── worktree.rs   # `lab worktree` / `lab .` - git worktree creation
│   └── cd.rs         # `lab cd` / `lab exec cd` - TUI selector entry point
├── entries.rs        # Entry struct, directory loading, base score calculation
├── fuzzy.rs          # Fuzzy matching engine (subsequence, scoring, highlighting)
├── script.rs         # Shell script emission (warning, chaining, quoting)
├── git.rs            # Git URI parsing, worktree detection, is_git_uri?
├── shell.rs          # Shell detection, init snippet generation per shell
├── tui/
│   ├── mod.rs        # TUI main loop, event handling
│   ├── app.rs        # App state (entries, cursor, scroll, input, mode)
│   ├── render.rs     # ratatui rendering (header, body, footer widgets)
│   ├── input.rs      # Key event → action mapping, input field logic
│   ├── dialogs.rs    # Delete confirm, rename, graduate dialog state+render
│   └── test_keys.rs  # --and-keys/--and-exit/--and-type injection
└── util.rs           # Path quoting, date formatting, name resolution
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| clap | CLI argument parsing with derive |
| ratatui | TUI rendering framework |
| crossterm | Terminal backend (raw mode, events, alt screen) |
| chrono | Date formatting (YYYY-MM-DD prefix) |
| dirs | Home directory resolution |

## Data Flow

1. **CLI dispatch** (`main.rs`): Parse args via clap → match command → invoke handler
2. **Non-interactive commands** (init/install/clone/worktree): Generate shell script → write to stdout → exit
3. **Interactive selector** (`cd`/`exec`):
   - Load entries from LAB_PATH (skip hidden, skip files, handle symlinks)
   - Calculate base scores (date prefix +2.0, recency via mtime)
   - Enter raw mode + alt screen
   - Main loop: read event → update state → render → repeat
   - On selection: exit raw mode → write script to stdout
   - On cancel: exit raw mode → exit 1

## IO Discipline (CRITICAL)

- **stdout**: Shell scripts only. The shell wrapper evals this.
- **stderr**: TUI rendering (via ratatui writing to stderr). Redirected to /dev/tty by shell wrapper.
- This separation is fundamental. Never write user-visible text to stdout. Never write scripts to stderr.

## TUI Architecture

### State (`app.rs`)
- `entries: Vec<Entry>` — loaded directories with base scores
- `filtered: Vec<(usize, f64, Vec<usize>)>` — (index, score, highlight_positions)
- `input: String` + `cursor_pos: usize` — search field
- `list_pos: usize` + `scroll_offset: usize` — list navigation
- `mode: Mode` — Normal | DeleteConfirm | Rename | Graduate
- `marks: HashSet<usize>` — marked for deletion

### Render (`render.rs`)
ratatui renders to a `Frame`. Layout splits into header (3 lines), body (remaining), footer (2 lines). Each entry is a `Line` with styled `Span`s. Metadata right-aligned via ratatui's `Alignment::Right` or manual padding.

### Event Loop (`mod.rs`)
```
loop {
    terminal.draw(|f| render(f, &app))?;
    let event = read_event(&mut key_source)?;  // crossterm or injected
    match handle_event(&mut app, event) {
        Action::Continue => continue,
        Action::Select(script) => { print!("{}", script); break Ok(0); }
        Action::Cancel => break Ok(1),
    }
}
```

### Test Key Injection (`test_keys.rs`)
When `--and-keys` or `--and-exit` is set, events come from a `VecDeque<KeyEvent>` instead of crossterm. When queue exhausts, ESC is auto-sent. This enables deterministic testing without a real terminal.

## Scoring Algorithm

```
base_score = 3.0 / sqrt(hours_since_mtime + 1)
if name matches /^\d{4}-\d{2}-\d{2}-/ { base_score += 2.0 }

fuzzy_score (per match):
  +1.0 per matched char
  +1.0 word boundary (pos 0 or after non-[a-z0-9])
  +2.0/sqrt(gap+1) proximity bonus
  *= query_len / (last_match_pos + 1)   // density
  *= 10.0 / (name.len() + 10.0)         // length penalty

total = fuzzy_score + base_score
```

## Script Output Format

All scripts follow this pattern:
```
# if you can read this, you didn't launch lab from an alias. run lab --help.
command1 'arg' && \
  command2 'arg' && \
  cd 'path'
```

Path quoting: single quotes, internal `'` escaped as `'"'"'`.

## Name Collision Resolution

When creating a new directory and the name already exists:
- If name ends with digits (e.g., `feature1`): increment → `feature2`
- Otherwise: append `-2`, then `-3`, etc.

## Environment Variables

| Variable | Default | Maps from Ruby |
|----------|---------|---------------|
| LAB_PATH | ~/src/labs | TRY_PATH |
| LAB_PROJECTS | parent of LAB_PATH | TRY_PROJECTS |
| NO_COLOR | unset | NO_COLOR |
| LAB_WIDTH | terminal width | TRY_WIDTH |
| LAB_HEIGHT | terminal height | TRY_HEIGHT |

# Command Line Specification

## Synopsis

```
lab [options] [command] [args...]
lab exec [options] [command] [args...]
```

## Description

`lab` is an ephemeral workspace manager that helps organize project directories with date-prefixed naming. It provides an interactive selector for navigating between workspaces and commands for creating new ones.

## Global Options

| Option | Description |
|--------|-------------|
| `--help`, `-h` | Show help text |
| `--version`, `-v` | Show version number |
| `--path <dir>` | Override labs directory (default: `~/src/labs`) |
| `--no-colors` | Disable ANSI color codes in output |

## Commands

### cd (default)

Interactive directory selector with fuzzy search.

```
lab cd [query]
lab exec cd [query]
lab exec [query]        # equivalent to: lab exec cd [query]
```

**Arguments:**
- `query` (optional): Initial filter text for fuzzy search

**Behavior:**
- Opens interactive TUI for directory selection
- Filters directories by query if provided
- Returns shell script to cd into selected directory

**Actions:**
- Select existing directory → touch and cd
- Select "[new]" entry → mkdir, git init, and cd (creates `YYYY-MM-DD-query`)
- Press Esc → cancel (exit 1)

### clone

Clone a git repository into a dated directory.

```
lab clone <url> [name]
lab exec clone <url> [name]
lab <url> [name]            # URL shorthand (same as clone)
```

**Arguments:**
- `url` (required): Git repository URL
- `name` (optional): Custom name suffix (default: extracted from URL)

**Behavior:**
- Creates directory named `YYYY-MM-DD-<user>-<repo>` (extracted from URL)
- Clones repository into that directory
- Returns shell script to cd into cloned directory

**Examples:**
```
lab clone https://github.com/tobi/lab.git
# Creates: 2025-11-30-tobi-lab

lab clone https://github.com/user/repo myproject
# Creates: 2025-11-30-myproject (custom name overrides)

lab https://github.com/tobi/lab.git
# URL shorthand (same as first example)

lab clone git@github.com:tobi/lab.git
# SSH URL also works: 2025-11-30-tobi-lab
```

### worktree

Create a git worktree in a dated directory.

```
lab worktree <name>
lab exec worktree <name>
lab . <name>              # Shorthand (requires name)
```

**Arguments:**
- `name` (required): Branch or worktree name

**Behavior:**
- Must be run from within a git repository
- Creates worktree in `YYYY-MM-DD-<name>`
- Returns shell script to cd into worktree
- `lab .` without a name is NOT supported (too easy to invoke accidentally)

### init

Output shell function definition for shell integration.

```
lab init [path]
```

**Arguments:**
- `path` (optional): Override default labs directory

**Behavior:**
- Detects current shell (bash/zsh or fish)
- Outputs appropriate function definition to stdout
- Function wraps `lab exec` and evals output

**Usage:**
```bash
# bash/zsh
eval "$(lab init ~/src/labs)"

# fish
eval (lab init ~/src/labs | string collect)
```

## Execution Modes

### Direct Mode

When `lab` is invoked without `exec`:

- Commands execute immediately
- Cannot change parent shell's directory
- Prints cd hint for user to copy/paste

```
$ lab clone https://github.com/user/repo
Cloning into '/home/user/src/labs/2025-11-30-repo'...
cd '/home/user/src/labs/2025-11-30-repo'
```

### Exec Mode

When `lab exec` is used (typically via shell alias):

- Returns shell script to stdout
- Exit code 0: alias evals output (performs cd)
- Exit code 1: alias prints output (error/cancel message)

```
$ lab exec clone https://github.com/user/repo
# if you can read this, you didn't launch lab from an alias. run lab --help.
git clone 'https://github.com/user/repo' '/home/user/src/labs/2025-11-30-repo' && \
  cd '/home/user/src/labs/2025-11-30-repo'
```

## Script Output Format

All exec mode commands output shell scripts with each command on its own line:

```bash
# if you can read this, you didn't launch lab from an alias. run lab --help.
<command> && \
  cd '<path>'
```

Commands are chained with `&& \` for readability, with 2-space indent on continuation lines. The warning comment helps users who accidentally run `lab exec` directly.

## Exit Codes

| Code | Meaning | Alias Action |
|------|---------|--------------|
| 0 | Success | Eval output |
| 1 | Error or cancelled | Print output |

## Environment

| Variable | Description |
|----------|-------------|
| `HOME` | Used to resolve default labs path (`$HOME/src/labs`) |
| `SHELL` | Used by `init` to detect shell type |
| `NO_COLOR` | If set, disables colors (equivalent to `--no-colors`) |

## Defaults

- **Labs directory**: `~/src/labs`
- **Date format**: `YYYY-MM-DD`
- **Directory naming**: `YYYY-MM-DD-<name>`

## Color Output

By default, `lab` uses ANSI color codes for syntax highlighting and visual formatting in the TUI and help output.

### Disabling Colors

Colors can be disabled in two ways:

1. **Command-line flag**: `--no-colors`
2. **Environment variable**: `NO_COLOR=1` (any non-empty value)

The `NO_COLOR` environment variable follows the [no-color.org](https://no-color.org/) standard, which is supported by many command-line tools.

**Examples:**
```bash
# Using the flag
lab --no-colors --help

# Using the environment variable
NO_COLOR=1 lab --help

# Set globally in shell config
export NO_COLOR=1
```

**Behavior:**
- Styling codes (bold, colors, dim, reset) are suppressed
- Cursor control sequences for the TUI still function normally
- Useful for piping output, accessibility, or terminals without color support

---

## Testing

For test framework documentation including `--and-exit`, `--and-keys`, and test writing guidelines, see [test_spec.md](test_spec.md).

---

## Examples

```bash
# Set up shell integration
eval "$(lab init)"

# Interactive selector
lab

# Selector with initial filter
lab project

# Clone a repository
lab clone https://github.com/user/repo

# Clone with custom name
lab clone https://github.com/user/repo my-fork

# Create git worktree (from within a repo)
lab worktree feature-branch

# Show version
lab --version

# Show help
lab --help
```

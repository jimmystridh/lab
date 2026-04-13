# lab

`lab` is a Rust CLI/TUI for managing ephemeral workspaces. It keeps labs under a single root (`~/src/labs` by default), uses date-prefixed directory names, and lets you jump into them with an interactive selector plus `git clone` and `git worktree` helpers.

> In the examples below, `lab ...` means the compiled binary. From this repo checkout you can run the same commands as `cargo run -- ...`.

## Build

```bash
cargo build
```

The debug binary is written to `./target/debug/lab`.

## Run

Use `cargo run -- <args>` while working from the source tree:

```bash
cargo run -- --help
cargo run -- init ~/src/labs
cargo run -- clone https://github.com/user/repo
cargo run -- worktree feature-name
```

For day-to-day use, install shell integration and then run `lab ...` directly.

## Shell integration

`lab` needs a shell wrapper so it can change your current shell directory after a selection.

Preview the wrapper with `lab init`:

```bash
# bash / zsh
eval "$(lab init ~/src/labs)"

# fish
eval (lab init ~/src/labs | string collect)
```

Install it permanently with `lab install`:

```bash
lab install ~/src/labs
```

`lab install` detects your current shell and appends the same `lab init` snippet to the appropriate shell config file.

## Key commands

| Command | Description |
| --- | --- |
| `lab`, `lab <query>`, or `lab cd [query]` | Open the interactive selector, optionally with an initial search query. |
| `lab clone <url> [name]` | Clone a repository into a dated lab such as `YYYY-MM-DD-user-repo` or `YYYY-MM-DD-name`. |
| `lab worktree <name>` | Create a dated worktree from the current Git repo; outside Git it falls back to creating a normal directory. |
| `lab . <name>` | Dot shorthand that uses the current directory as the worktree source with the same Git/fallback behavior. |

## Environment variables

| Variable | Default | Purpose |
| --- | --- | --- |
| `LAB_PATH` | `~/src/labs` | Root directory that `lab` manages. The directory is created automatically if it does not exist. |
| `LAB_PROJECTS` | parent of `LAB_PATH` | Destination used by the graduate action when promoting a lab into a project. |
| `NO_COLOR` | unset | Disable ANSI styling. |
| `LAB_WIDTH` | current terminal width | Override terminal width, mainly for deterministic tests. |
| `LAB_HEIGHT` | current terminal height | Override terminal height, mainly for deterministic tests. |

## Tests

Run the Rust unit tests:

```bash
cargo test
```

Run the shell-based spec suite against a built binary:

```bash
cargo build
bash spec/tests/runner.sh ./target/debug/lab
```

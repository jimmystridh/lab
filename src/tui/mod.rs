//! TUI (Terminal User Interface) main module.
//!
//! Implements the ratatui/crossterm selector shell used by `lab exec`.
//! Rendering is always sent to stderr, while scripts remain on stdout.

pub mod app;
pub mod input;
pub mod render;
pub mod test_keys;

use self::{
    app::{App, TerminalSize},
    input::handle_key,
    test_keys::TestKeySource,
};
use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal,
};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal, TerminalOptions, Viewport};
use std::{
    io::{self, IsTerminal, Stderr, Write},
    path::PathBuf,
};

/// Options for a TUI run.
#[derive(Debug, Clone, Copy)]
pub struct RunOptions<'a> {
    /// Render one frame and exit after processing any injected keys.
    pub and_exit: bool,
    /// Optional injected test key sequence.
    pub and_keys: Option<&'a str>,
    /// Whether test-key infrastructure should be used.
    pub use_test_source: bool,
}

/// Outcome produced by the selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiOutcome {
    /// Existing entry selected.
    Selected(PathBuf),
    /// Virtual create-new entry selected.
    Create(PathBuf),
    /// Selector cancelled. `emit_message` controls whether stdout prints "Cancelled."
    Cancelled { emit_message: bool },
}

type TuiTerminal = Terminal<CrosstermBackend<Stderr>>;

const ALT_SCREEN_ON: &str = "\x1b[?1049h";
const ALT_SCREEN_OFF: &str = "\x1b[?1049l";
const HIDE_CURSOR: &str = "\x1b[?25l";
const SHOW_CURSOR: &str = "\x1b[?25h";

/// Run the TUI selector.
pub fn run_tui(app: &mut App, options: RunOptions<'_>) -> io::Result<TuiOutcome> {
    let interactive = !options.use_test_source;
    if interactive && (!io::stdin().is_terminal() || !io::stderr().is_terminal()) {
        eprintln!("Error: lab requires an interactive terminal");
        return Ok(TuiOutcome::Cancelled {
            emit_message: false,
        });
    }

    if options.use_test_source {
        if options.and_exit {
            if options.and_keys.is_some() {
                let mut key_source = TestKeySource::new(options.and_keys);
                let _ = drive_test_mode(app, &mut key_source);
            }
            io::stderr().write_all(render::render_snapshot(app).as_bytes())?;
            return Ok(TuiOutcome::Cancelled {
                emit_message: false,
            });
        }

        let mut key_source = TestKeySource::new(options.and_keys);
        let outcome = drive_test_mode(app, &mut key_source);
        return Ok(outcome);
    }

    let mut session = TerminalSession::enter(app.terminal_size, interactive)?;
    interactive_loop(&mut session.terminal, app)
}
struct TerminalSession {
    terminal: TuiTerminal,
    raw_mode_enabled: bool,
}

impl TerminalSession {
    fn enter(size: TerminalSize, enable_raw_mode: bool) -> io::Result<Self> {
        {
            let mut stderr = io::stderr();
            stderr.write_all(ALT_SCREEN_ON.as_bytes())?;
            stderr.write_all(HIDE_CURSOR.as_bytes())?;
            stderr.flush()?;
        }

        if enable_raw_mode {
            if let Err(error) = terminal::enable_raw_mode() {
                let mut stderr = io::stderr();
                let _ = stderr.write_all(SHOW_CURSOR.as_bytes());
                let _ = stderr.write_all(ALT_SCREEN_OFF.as_bytes());
                let _ = stderr.flush();
                return Err(error);
            }
        }

        let backend = CrosstermBackend::new(io::stderr());
        let viewport = Viewport::Fixed(Rect::new(0, 0, size.width, size.height));
        let mut terminal = Terminal::with_options(backend, TerminalOptions { viewport })?;
        terminal.clear()?;

        Ok(Self {
            terminal,
            raw_mode_enabled: enable_raw_mode,
        })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.raw_mode_enabled {
            let _ = terminal::disable_raw_mode();
        }

        let mut stderr = io::stderr();
        let _ = stderr.write_all(SHOW_CURSOR.as_bytes());
        let _ = stderr.write_all(ALT_SCREEN_OFF.as_bytes());
        let _ = stderr.flush();
    }
}

fn interactive_loop(terminal: &mut TuiTerminal, app: &mut App) -> io::Result<TuiOutcome> {
    loop {
        draw_app(terminal, app)?;

        match event::read()? {
            Event::Key(key) if is_press_or_repeat(key.kind) => {
                if let Some(outcome) = handle_key(app, key) {
                    return Ok(outcome);
                }
            }
            Event::Resize(width, height) => {
                let size = TerminalSize::new(width, height);
                app.set_terminal_size(size);
                terminal.resize(Rect::new(0, 0, width.max(1), height.max(1)))?;
            }
            _ => {}
        }
    }
}

fn drive_test_mode(app: &mut App, key_source: &mut TestKeySource) -> TuiOutcome {
    loop {
        if let Some(outcome) = handle_key(app, key_source.next_key_event()) {
            return outcome;
        }
    }
}

fn draw_app(terminal: &mut TuiTerminal, app: &App) -> io::Result<()> {
    terminal.draw(|frame| render::render(frame, app))?;
    Ok(())
}

fn is_press_or_repeat(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

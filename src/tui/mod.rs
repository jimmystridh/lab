//! TUI (Terminal User Interface) main module.
//!
//! Implements the ratatui/crossterm selector shell used by `lab exec`.
//! Rendering is always sent to stderr, while scripts remain on stdout.

pub mod app;
pub mod render;
pub mod test_keys;

use self::{
    app::{App, Selection, TerminalSize},
    test_keys::TestKeySource,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
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

    let mut session = TerminalSession::enter(app.terminal_size, interactive)?;

    if options.and_exit && options.and_keys.is_none() {
        draw_app(&mut session.terminal, app)?;
        return Ok(TuiOutcome::Cancelled {
            emit_message: false,
        });
    }

    if options.use_test_source {
        let mut key_source = TestKeySource::new(options.and_keys);
        let outcome = drive_test_mode(app, &mut key_source);
        if options.and_exit {
            draw_app(&mut session.terminal, app)?;
        }
        return Ok(outcome);
    }

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

fn handle_key(app: &mut App, key: KeyEvent) -> Option<TuiOutcome> {
    match key.code {
        KeyCode::Enter => Some(selection_outcome(app.current_selection())),
        KeyCode::Esc => Some(TuiOutcome::Cancelled { emit_message: true }),
        KeyCode::Backspace => {
            app.backspace();
            None
        }
        KeyCode::Up => {
            app.move_up();
            None
        }
        KeyCode::Down => {
            app.move_down();
            None
        }
        KeyCode::Home => {
            app.move_to_top();
            None
        }
        KeyCode::End => {
            app.move_to_bottom();
            None
        }
        KeyCode::PageUp => {
            app.page_up();
            None
        }
        KeyCode::PageDown => {
            app.page_down();
            None
        }
        KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_control_key(app, character)
        }
        KeyCode::Char(character) => {
            app.insert_char(character);
            None
        }
        _ => None,
    }
}

fn handle_control_key(app: &mut App, character: char) -> Option<TuiOutcome> {
    match character.to_ascii_lowercase() {
        'a' => {
            app.move_input_to_start();
            None
        }
        'b' => {
            app.move_input_back();
            None
        }
        'c' => Some(TuiOutcome::Cancelled { emit_message: true }),
        'e' => {
            app.move_input_to_end();
            None
        }
        'f' => {
            app.move_input_forward();
            None
        }
        'j' | 'n' => {
            app.move_down();
            None
        }
        'k' => {
            app.kill_to_end();
            None
        }
        'p' => {
            app.move_up();
            None
        }
        'w' => {
            app.delete_word_backward();
            None
        }
        _ => None,
    }
}

fn selection_outcome(selection: Option<Selection>) -> TuiOutcome {
    match selection {
        Some(Selection::Existing(path)) => TuiOutcome::Selected(path),
        Some(Selection::Create(path)) => TuiOutcome::Create(path),
        None => TuiOutcome::Cancelled {
            emit_message: false,
        },
    }
}

fn is_press_or_repeat(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

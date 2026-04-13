//! Test-only key injection support for deterministic TUI validation.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;

/// Deterministic key source backed by a queue of injected events.
#[derive(Debug, Clone)]
pub struct TestKeySource {
    events: VecDeque<KeyEvent>,
}

impl TestKeySource {
    /// Create a new key source from an optional `--and-keys` string.
    pub fn new(and_keys: Option<&str>) -> Self {
        let events = and_keys
            .map(parse_and_keys)
            .unwrap_or_default()
            .into_iter()
            .collect();

        Self { events }
    }

    /// Return the next queued key event, auto-sending ESC after exhaustion.
    pub fn next_key_event(&mut self) -> KeyEvent {
        self.events
            .pop_front()
            .unwrap_or_else(|| KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
    }
}

/// Parse a `--and-keys` payload into a sequence of crossterm key events.
pub fn parse_and_keys(keys: &str) -> Vec<KeyEvent> {
    if keys.contains(',') {
        let mut events = Vec::new();
        for token in keys.split(',') {
            parse_token(token, &mut events);
        }
        events
    } else {
        let mut events = Vec::new();
        parse_token(keys, &mut events);
        events
    }
}

fn parse_token(token: &str, events: &mut Vec<KeyEvent>) {
    if token.is_empty() {
        return;
    }

    let trimmed = token.trim();
    if !trimmed.is_empty() {
        if let Some(event) = parse_symbolic_key(trimmed) {
            events.push(event);
            return;
        }

        if trimmed.len() >= 5 && trimmed[..5].eq_ignore_ascii_case("TYPE=") {
            events.extend(trimmed[5..].chars().map(char_key));
            return;
        }
    }

    events.extend(parse_raw_or_literal(token));
}

fn parse_symbolic_key(token: &str) -> Option<KeyEvent> {
    let upper = token.trim().to_ascii_uppercase();
    match upper.as_str() {
        "UP" => Some(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
        "DOWN" => Some(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        "LEFT" => Some(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        "RIGHT" => Some(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        "HOME" => Some(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
        "END" => Some(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
        "PAGEUP" => Some(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
        "PAGEDOWN" => Some(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
        "ENTER" | "RETURN" | "CR" => Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        "ESC" | "ESCAPE" => Some(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        "BACKSPACE" | "BS" => Some(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
        "TAB" => Some(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        "SHIFT-TAB" => Some(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        "SPACE" => Some(char_key(' ')),
        _ => parse_ctrl_symbolic(&upper),
    }
}

fn parse_ctrl_symbolic(token: &str) -> Option<KeyEvent> {
    let ctrl = token.strip_prefix("CTRL-")?;
    let mut chars = ctrl.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || !ch.is_ascii_uppercase() {
        return None;
    }

    if ch == 'H' {
        return Some(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    }

    Some(KeyEvent::new(
        KeyCode::Char(ch.to_ascii_lowercase()),
        KeyModifiers::CONTROL,
    ))
}

fn parse_raw_or_literal(input: &str) -> Vec<KeyEvent> {
    let bytes = input.as_bytes();
    let mut events = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                events.push(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                index += 1;
            }
            b'\t' => {
                events.push(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
                index += 1;
            }
            0x08 | 0x7f => {
                events.push(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
                index += 1;
            }
            0x01..=0x1a => {
                let ch = ((bytes[index] - 1) + b'a') as char;
                events.push(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL));
                index += 1;
            }
            0x1b => {
                let (event, consumed) = parse_escape_sequence(bytes, index);
                if let Some(event) = event {
                    events.push(event);
                }
                index += consumed;
            }
            byte if (0x20..=0x7e).contains(&byte) => {
                events.push(char_key(byte as char));
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    events
}

fn parse_escape_sequence(bytes: &[u8], start: usize) -> (Option<KeyEvent>, usize) {
    if start + 2 < bytes.len() && bytes[start + 1] == b'[' {
        match bytes[start + 2] {
            b'A' => return (Some(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)), 3),
            b'B' => return (Some(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)), 3),
            b'C' => return (Some(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)), 3),
            b'D' => return (Some(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)), 3),
            b'H' => return (Some(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)), 3),
            b'F' => return (Some(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)), 3),
            b'Z' => {
                return (
                    Some(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
                    3,
                )
            }
            _ => {}
        }

        let mut end = start + 2;
        while end < bytes.len() && !matches!(bytes[end], b'~' | b'A'..=b'Z' | b'a'..=b'z') {
            end += 1;
        }

        if end < bytes.len() {
            let sequence = &bytes[start + 2..=end];
            let event = match sequence {
                b"1~" | b"7~" => Some(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
                b"4~" | b"8~" => Some(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
                b"5~" => Some(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
                b"6~" => Some(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
                _ => None,
            };
            return (event, end - start + 1);
        }
    }

    if start + 2 < bytes.len() && bytes[start + 1] == b'O' {
        let event = match bytes[start + 2] {
            b'H' => Some(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
            b'F' => Some(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            _ => None,
        };
        return (event, 3);
    }

    (Some(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)), 1)
}

fn char_key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_symbolic_named_keys() {
        let events = parse_and_keys(
            "UP,DOWN,LEFT,RIGHT,HOME,END,PAGEUP,PAGEDOWN,TAB,SHIFT-TAB,SPACE,RETURN,ESC,BACKSPACE",
        );

        assert_eq!(
            events,
            vec![
                KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            ]
        );
    }

    #[test]
    fn test_parse_symbolic_ctrl_keys() {
        let events = parse_and_keys("CTRL-A,CTRL-D,CTRL-Z");

        assert_eq!(
            events,
            vec![
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
                KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            ]
        );
    }

    #[test]
    fn test_parse_type_token_expands_literal_text() {
        let events = parse_and_keys("CTRL-A,TYPE=hello world,ENTER");

        assert_eq!(
            events[0],
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
        );
        assert_eq!(
            events.last(),
            Some(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        );
        assert!(events.contains(&char_key('h')));
        assert!(events.contains(&char_key(' ')));
    }

    #[test]
    fn test_parse_raw_sequences_and_printable_text() {
        let events = parse_and_keys("\x1b[A\x1b[Babc\r\x7f");

        assert_eq!(
            events,
            vec![
                KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                char_key('a'),
                char_key('b'),
                char_key('c'),
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            ]
        );
    }

    #[test]
    fn test_parse_mixed_symbolic_and_raw_tokens() {
        let events = parse_and_keys("TYPE=go,\x1b[B,ENTER");

        assert_eq!(
            events,
            vec![
                char_key('g'),
                char_key('o'),
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            ]
        );
    }

    #[test]
    fn test_parse_ctrl_h_as_backspace() {
        let events = parse_and_keys("CTRL-H");

        assert_eq!(
            events,
            vec![KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)]
        );
    }

    #[test]
    fn test_test_key_source_auto_sends_escape_when_queue_is_empty() {
        let mut key_source = TestKeySource::new(Some("ENTER"));
        assert_eq!(
            key_source.next_key_event(),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
        );
        assert_eq!(
            key_source.next_key_event(),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
        );
    }
}

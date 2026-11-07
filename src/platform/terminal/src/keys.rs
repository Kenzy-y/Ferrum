//! Key presses as the bytes a terminal program reads (xterm encoding).
//!
//! Keys are named like GPUI keystrokes (`enter`, `left`, `f5`, `a`), so
//! this module stays free of UI types. Plain text is not encoded here: it
//! arrives through the platform text input (with IME) and is written as is.

/// Modifier keys held with a key.
// Each flag is one physical modifier, not a hidden state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Ctrl.
    pub control: bool,
    /// Alt (Option on macOS).
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// Cmd on macOS, the Windows key elsewhere.
    pub platform: bool,
}

/// Terminal modes that change what keys send.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyModes {
    /// Application cursor keys (DECCKM): arrows, Home and End send `ESC O`.
    pub app_cursor: bool,
}

const ESC: u8 = 0x1b;

/// The bytes for `key` with `modifiers`, or `None` when the key is plain
/// text (left to the text input) or a shortcut of the IDE: anything with
/// Cmd, Ctrl with Shift on a character, and keys a terminal has no
/// sequence for.
pub fn encode(key: &str, modifiers: Modifiers, modes: KeyModes) -> Option<Vec<u8>> {
    if modifiers.platform {
        return None;
    }
    if let Some(bytes) = named_key(key, modifiers, modes) {
        return Some(bytes);
    }
    let mut chars = key.chars();
    let (Some(character), None) = (chars.next(), chars.next()) else {
        return None;
    };
    character_key(character, modifiers)
}

/// Whether the key, if typed in a terminal, means something to the
/// program rather than to the IDE: Ctrl or Alt with a character
/// (`Ctrl C`, `Ctrl R`, `Alt B`). Such keys go to the shell even when the
/// keymap binds them globally; named keys (F-keys, Tab) and Ctrl Shift
/// combinations stay with the keymap.
pub fn belongs_to_program(key: &str, modifiers: Modifiers) -> bool {
    if modifiers.platform || !(modifiers.control || modifiers.alt) {
        return false;
    }
    if modifiers.control && modifiers.shift {
        return false;
    }
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(character), None) => character_key(character, modifiers).is_some(),
        _ => matches!(key, "space" | "backspace"),
    }
}

/// xterm's modifier parameter: 1 plus Shift 1, Alt 2, Ctrl 4.
fn modifier_parameter(modifiers: Modifiers) -> u8 {
    1 + u8::from(modifiers.shift) + 2 * u8::from(modifiers.alt) + 4 * u8::from(modifiers.control)
}

fn has_modifiers(modifiers: Modifiers) -> bool {
    modifiers.shift || modifiers.alt || modifiers.control
}

/// Prefixes `bytes` with Escape when Alt is held (the "meta sends
/// escape" convention).
fn with_alt(modifiers: Modifiers, bytes: &[u8]) -> Vec<u8> {
    let mut sequence = Vec::with_capacity(bytes.len() + 1);
    if modifiers.alt {
        sequence.push(ESC);
    }
    sequence.extend_from_slice(bytes);
    sequence
}

/// `ESC [ 1 ; m <final>` with modifiers, `ESC [ <final>` or `ESC O
/// <final>` (application mode) without.
fn cursor_key(final_byte: u8, modifiers: Modifiers, modes: KeyModes) -> Vec<u8> {
    if has_modifiers(modifiers) {
        format!(
            "\x1b[1;{}{}",
            modifier_parameter(modifiers),
            final_byte as char
        )
        .into_bytes()
    } else if modes.app_cursor {
        vec![ESC, b'O', final_byte]
    } else {
        vec![ESC, b'[', final_byte]
    }
}

/// `ESC [ n ~`, or `ESC [ n ; m ~` with modifiers.
fn tilde_key(number: u8, modifiers: Modifiers) -> Vec<u8> {
    if has_modifiers(modifiers) {
        format!("\x1b[{number};{}~", modifier_parameter(modifiers)).into_bytes()
    } else {
        format!("\x1b[{number}~").into_bytes()
    }
}

/// F1 to F4: `ESC O P..S`, or `ESC [ 1 ; m P..S` with modifiers.
fn function_key_low(final_byte: u8, modifiers: Modifiers) -> Vec<u8> {
    if has_modifiers(modifiers) {
        format!(
            "\x1b[1;{}{}",
            modifier_parameter(modifiers),
            final_byte as char
        )
        .into_bytes()
    } else {
        vec![ESC, b'O', final_byte]
    }
}

fn named_key(key: &str, modifiers: Modifiers, modes: KeyModes) -> Option<Vec<u8>> {
    Some(match key {
        "enter" => with_alt(modifiers, b"\r"),
        "tab" if modifiers.shift => b"\x1b[Z".to_vec(),
        "tab" if modifiers.control => return None,
        "tab" => with_alt(modifiers, b"\t"),
        "escape" => with_alt(modifiers, &[ESC]),
        // Ctrl Backspace deletes a word, like Ctrl W in shells.
        "backspace" if modifiers.control => vec![0x17],
        "backspace" => with_alt(modifiers, &[0x7f]),
        "space" if modifiers.control => with_alt(modifiers, &[0]),
        "space" => with_alt(modifiers, b" "),
        "up" => cursor_key(b'A', modifiers, modes),
        "down" => cursor_key(b'B', modifiers, modes),
        "right" => cursor_key(b'C', modifiers, modes),
        "left" => cursor_key(b'D', modifiers, modes),
        "home" => cursor_key(b'H', modifiers, modes),
        "end" => cursor_key(b'F', modifiers, modes),
        "insert" => tilde_key(2, modifiers),
        "delete" => tilde_key(3, modifiers),
        "pageup" => tilde_key(5, modifiers),
        "pagedown" => tilde_key(6, modifiers),
        "f1" => function_key_low(b'P', modifiers),
        "f2" => function_key_low(b'Q', modifiers),
        "f3" => function_key_low(b'R', modifiers),
        "f4" => function_key_low(b'S', modifiers),
        "f5" => tilde_key(15, modifiers),
        "f6" => tilde_key(17, modifiers),
        "f7" => tilde_key(18, modifiers),
        "f8" => tilde_key(19, modifiers),
        "f9" => tilde_key(20, modifiers),
        "f10" => tilde_key(21, modifiers),
        "f11" => tilde_key(23, modifiers),
        "f12" => tilde_key(24, modifiers),
        _ => return None,
    })
}

/// A character key with Ctrl and/or Alt. Plain and Shift characters are
/// text; Ctrl Shift is left to the IDE.
fn character_key(character: char, modifiers: Modifiers) -> Option<Vec<u8>> {
    if !(modifiers.control || modifiers.alt) {
        return None;
    }
    if modifiers.control && modifiers.shift {
        return None;
    }
    if modifiers.control {
        let control = control_character(character)?;
        return Some(with_alt(modifiers, &[control]));
    }
    // Alt with a character: Escape, then the character.
    let mut buffer = [0; 4];
    let text = character.encode_utf8(&mut buffer);
    Some(with_alt(modifiers, text.as_bytes()))
}

/// The control character Ctrl produces with `character`, as in xterm.
fn control_character(character: char) -> Option<u8> {
    let character = character.to_ascii_lowercase();
    Some(match character {
        // `a`..`z` are 1..26; the cast cannot truncate an ASCII letter.
        'a'..='z' => character as u8 - b'a' + 1,
        '@' | '2' => 0,
        '[' | '3' => 0x1b,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' => 0x1e,
        '_' | '-' | '7' => 0x1f,
        '?' | '8' => 0x7f,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Modifiers = Modifiers {
        control: false,
        alt: false,
        shift: false,
        platform: false,
    };
    const CTRL: Modifiers = Modifiers {
        control: true,
        ..NONE
    };
    const ALT: Modifiers = Modifiers { alt: true, ..NONE };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        ..NONE
    };
    const CTRL_SHIFT: Modifiers = Modifiers {
        control: true,
        shift: true,
        ..NONE
    };

    fn key(key: &str, modifiers: Modifiers) -> Option<Vec<u8>> {
        encode(key, modifiers, KeyModes::default())
    }

    #[test]
    fn editing_keys() {
        assert_eq!(key("enter", NONE).unwrap(), b"\r");
        assert_eq!(key("backspace", NONE).unwrap(), [0x7f]);
        assert_eq!(key("backspace", CTRL).unwrap(), [0x17]);
        assert_eq!(key("backspace", ALT).unwrap(), [0x1b, 0x7f]);
        assert_eq!(key("tab", NONE).unwrap(), b"\t");
        assert_eq!(key("tab", SHIFT).unwrap(), b"\x1b[Z");
        assert_eq!(key("tab", CTRL), None);
        assert_eq!(key("escape", NONE).unwrap(), [0x1b]);
        assert_eq!(key("delete", NONE).unwrap(), b"\x1b[3~");
    }

    #[test]
    fn cursor_keys_follow_the_application_mode() {
        assert_eq!(key("up", NONE).unwrap(), b"\x1b[A");
        let app = KeyModes { app_cursor: true };
        assert_eq!(encode("up", NONE, app).unwrap(), b"\x1bOA");
        assert_eq!(encode("left", CTRL, app).unwrap(), b"\x1b[1;5D");
        assert_eq!(key("home", SHIFT).unwrap(), b"\x1b[1;2H");
    }

    #[test]
    fn function_keys() {
        assert_eq!(key("f1", NONE).unwrap(), b"\x1bOP");
        assert_eq!(key("f1", SHIFT).unwrap(), b"\x1b[1;2P");
        assert_eq!(key("f5", NONE).unwrap(), b"\x1b[15~");
        assert_eq!(key("f12", CTRL).unwrap(), b"\x1b[24;5~");
    }

    #[test]
    fn control_and_alt_characters() {
        assert_eq!(key("c", CTRL).unwrap(), [3]);
        assert_eq!(key("C", CTRL).unwrap(), [3]);
        assert_eq!(key("[", CTRL).unwrap(), [0x1b]);
        assert_eq!(key("space", CTRL).unwrap(), [0]);
        assert_eq!(key("b", ALT).unwrap(), b"\x1bb");
        assert_eq!(
            key("c", Modifiers { alt: true, ..CTRL }).unwrap(),
            [0x1b, 3]
        );
    }

    #[test]
    fn text_and_ide_shortcuts_are_not_encoded() {
        assert_eq!(key("a", NONE), None);
        assert_eq!(key("A", SHIFT), None);
        assert_eq!(key("p", CTRL_SHIFT), None);
        assert_eq!(key("`", CTRL), None);
        let cmd = Modifiers {
            platform: true,
            ..NONE
        };
        assert_eq!(key("c", cmd), None);
        assert_eq!(key("left", cmd), None);
    }

    #[test]
    fn program_keys_win_over_global_bindings() {
        assert!(belongs_to_program("c", CTRL));
        assert!(belongs_to_program("w", CTRL));
        assert!(belongs_to_program("b", ALT));
        assert!(!belongs_to_program("p", CTRL_SHIFT));
        assert!(!belongs_to_program("`", CTRL));
        assert!(!belongs_to_program("tab", CTRL));
        assert!(!belongs_to_program("f1", NONE));
        assert!(!belongs_to_program("a", NONE));
    }
}

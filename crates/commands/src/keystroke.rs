//! Keystrokes and key sequences as written in keymaps.

use std::fmt;

/// Desktop platform, used to pick default keybindings and to resolve the
/// `mod` modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Microsoft Windows.
    Windows,
    /// Apple macOS.
    MacOs,
    /// Linux and other Unix-like desktops.
    Linux,
}

impl Platform {
    /// The platform this binary was built for.
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// Modifier keys held during a keystroke.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[allow(clippy::struct_excessive_bools)] // One flag per physical modifier key.
pub struct Modifiers {
    /// Control.
    pub control: bool,
    /// Alt (Option on macOS).
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// Command on macOS, the Windows key on Windows, Super on Linux.
    pub platform: bool,
}

/// One key press with modifiers, e.g. `ctrl-shift-p`.
///
/// The key is stored lowercase: `a`, `f4`, `escape`, `enter`, `-`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Keystroke {
    /// Held modifiers.
    pub modifiers: Modifiers,
    /// The pressed key.
    pub key: String,
}

/// A keystroke string that cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeystrokeError {
    /// The string has no key, e.g. `ctrl-`.
    #[error("`{0}` has no key")]
    MissingKey(String),
    /// A modifier name is unknown or repeated.
    #[error("`{modifier}` in `{keystroke}` is not a known modifier or is repeated")]
    BadModifier {
        /// The whole keystroke.
        keystroke: String,
        /// The offending part.
        modifier: String,
    },
}

impl Keystroke {
    /// Parses `ctrl-shift-p`-style text. `mod` means Cmd on macOS and Ctrl
    /// elsewhere; `cmd`, `super` and `win` all mean the platform key.
    ///
    /// # Errors
    ///
    /// Fails on unknown or repeated modifiers and on a missing key.
    pub fn parse(text: &str, platform: Platform) -> Result<Self, KeystrokeError> {
        let text = text.trim();
        // A trailing `-` after a separator is the minus key itself: `ctrl--`.
        let (modifier_part, key) = if let Some(prefix) = text.strip_suffix("--") {
            (Some(prefix), "-")
        } else if text == "-" {
            (None, "-")
        } else {
            match text.rsplit_once('-') {
                Some((prefix, key)) => (Some(prefix), key),
                None => (None, text),
            }
        };
        if key.is_empty() {
            return Err(KeystrokeError::MissingKey(text.to_owned()));
        }

        let mut modifiers = Modifiers::default();
        for modifier in modifier_part.into_iter().flat_map(|part| part.split('-')) {
            let flag = match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => &mut modifiers.control,
                // `mod` is Ctrl on Windows and Linux, Cmd on macOS.
                "mod" if platform != Platform::MacOs => &mut modifiers.control,
                "alt" | "option" => &mut modifiers.alt,
                "shift" => &mut modifiers.shift,
                "cmd" | "super" | "win" | "mod" => &mut modifiers.platform,
                _ => return Err(bad_modifier(text, modifier)),
            };
            if *flag {
                return Err(bad_modifier(text, modifier));
            }
            *flag = true;
        }

        Ok(Self {
            modifiers,
            key: key.to_ascii_lowercase(),
        })
    }
}

fn bad_modifier(keystroke: &str, modifier: &str) -> KeystrokeError {
    KeystrokeError::BadModifier {
        keystroke: keystroke.to_owned(),
        modifier: modifier.to_owned(),
    }
}

impl fmt::Display for Keystroke {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Modifiers {
            control,
            alt,
            shift,
            platform,
        } = self.modifiers;
        for (held, name) in [
            (control, "ctrl"),
            (alt, "alt"),
            (shift, "shift"),
            (platform, "cmd"),
        ] {
            if held {
                write!(formatter, "{name}-")?;
            }
        }
        formatter.write_str(&self.key)
    }
}

/// Keystrokes pressed one after another, e.g. `ctrl-k ctrl-s`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySequence(pub Vec<Keystroke>);

impl KeySequence {
    /// Parses space-separated keystrokes.
    ///
    /// # Errors
    ///
    /// Fails if any keystroke is invalid or the text is empty.
    pub fn parse(text: &str, platform: Platform) -> Result<Self, KeystrokeError> {
        let keystrokes = text
            .split_whitespace()
            .map(|keystroke| Keystroke::parse(keystroke, platform))
            .collect::<Result<Vec<_>, _>>()?;
        if keystrokes.is_empty() {
            return Err(KeystrokeError::MissingKey(text.to_owned()));
        }
        Ok(Self(keystrokes))
    }

    /// Whether `prefix` is a strict prefix of this sequence.
    pub fn starts_with(&self, prefix: &[Keystroke]) -> bool {
        self.0.len() > prefix.len() && self.0.starts_with(prefix)
    }
}

impl fmt::Display for KeySequence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, keystroke) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str(" ")?;
            }
            write!(formatter, "{keystroke}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Keystroke {
        Keystroke::parse(text, Platform::Linux).unwrap()
    }

    #[test]
    fn parses_modifiers_and_key() {
        let keystroke = parse("Ctrl-Shift-P");
        assert!(keystroke.modifiers.control && keystroke.modifiers.shift);
        assert!(!keystroke.modifiers.alt && !keystroke.modifiers.platform);
        assert_eq!(keystroke.key, "p");
        assert_eq!(keystroke.to_string(), "ctrl-shift-p");
    }

    #[test]
    fn mod_depends_on_platform() {
        let linux = Keystroke::parse("mod-s", Platform::Linux).unwrap();
        let windows = Keystroke::parse("mod-s", Platform::Windows).unwrap();
        let macos = Keystroke::parse("mod-s", Platform::MacOs).unwrap();
        assert!(linux.modifiers.control && windows.modifiers.control);
        assert!(macos.modifiers.platform && !macos.modifiers.control);
    }

    #[test]
    fn minus_key() {
        assert_eq!(parse("ctrl--").key, "-");
        assert!(parse("ctrl--").modifiers.control);
        assert_eq!(parse("-").key, "-");
    }

    #[test]
    fn rejects_bad_keystrokes() {
        for text in ["", "ctrl-", "hyper-a", "ctrl-ctrl-a", "ctrl-mod-a"] {
            assert!(Keystroke::parse(text, Platform::Linux).is_err(), "{text}");
        }
    }

    #[test]
    fn sequences() {
        let sequence = KeySequence::parse("ctrl-k  ctrl-s", Platform::Linux).unwrap();
        assert_eq!(sequence.to_string(), "ctrl-k ctrl-s");
        assert!(sequence.starts_with(&[parse("ctrl-k")]));
        assert!(!sequence.starts_with(&sequence.0));
        assert!(KeySequence::parse("  ", Platform::Linux).is_err());
    }
}

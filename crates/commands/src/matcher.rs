//! Turning keystrokes into commands.

use crate::{CommandId, GLOBAL_CONTEXT, Keymap, Keystroke};

/// Outcome of feeding a keystroke to a [`KeymapMatcher`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchResult {
    /// A binding matched: run this command.
    Command(CommandId),
    /// The keys so far are the beginning of a longer binding: wait for more.
    Pending,
    /// No binding matches; the keystroke should go to the focused element.
    NoMatch,
}

/// Matches keystrokes, including multi-key sequences, against a [`Keymap`].
///
/// Contexts are given per keystroke, innermost first (for example
/// `["editor", "workbench"]`); `global` is always active and comes last.
/// A binding in an inner context wins over one in an outer context.
///
/// A sequence that is both a complete binding and the beginning of a longer
/// one waits for the next keystroke.
#[derive(Debug, Clone, Default)]
pub struct KeymapMatcher {
    pending: Vec<Keystroke>,
}

impl KeymapMatcher {
    /// Creates a matcher with no pending keystrokes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Keystrokes waiting for the rest of a sequence.
    pub fn pending(&self) -> &[Keystroke] {
        &self.pending
    }

    /// Drops pending keystrokes (for example when focus changes).
    pub fn reset(&mut self) {
        self.pending.clear();
    }

    /// The command bound to exactly the pending keystrokes, if any. Used
    /// when a sequence is both complete and the beginning of a longer one:
    /// after a timeout the application runs this command.
    pub fn pending_command(&self, keymap: &Keymap, contexts: &[&str]) -> Option<CommandId> {
        if self.pending.is_empty() {
            return None;
        }
        contexts
            .iter()
            .copied()
            .chain(std::iter::once(GLOBAL_CONTEXT))
            .find_map(|context| {
                keymap
                    .bindings()
                    .iter()
                    .find(|binding| {
                        binding.context.as_str() == context && binding.keys.0 == self.pending
                    })
                    .map(|binding| binding.command.clone())
            })
    }

    /// Feeds one keystroke.
    pub fn push(
        &mut self,
        keymap: &Keymap,
        keystroke: Keystroke,
        contexts: &[&str],
    ) -> MatchResult {
        self.pending.push(keystroke);
        let active = || {
            contexts
                .iter()
                .copied()
                .chain(std::iter::once(GLOBAL_CONTEXT))
        };

        let is_prefix = keymap.bindings().iter().any(|binding| {
            binding.keys.starts_with(&self.pending)
                && active().any(|context| binding.context.as_str() == context)
        });
        if is_prefix {
            return MatchResult::Pending;
        }

        let command = active().find_map(|context| {
            keymap
                .bindings()
                .iter()
                .find(|binding| {
                    binding.context.as_str() == context && binding.keys.0 == self.pending
                })
                .map(|binding| binding.command.clone())
        });
        self.pending.clear();
        command.map_or(MatchResult::NoMatch, MatchResult::Command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Platform;

    fn keymap() -> Keymap {
        let mut keymap = Keymap::default();
        keymap
            .apply(
                r#"
                [global]
                "ctrl-q" = "app.quit"
                "ctrl-s" = "workbench.saveAll"
                "ctrl-k ctrl-s" = "workbench.openKeymap"

                [editor]
                "ctrl-s" = "editor.save"
                "#,
                Platform::Linux,
            )
            .unwrap();
        keymap
    }

    fn key(text: &str) -> Keystroke {
        Keystroke::parse(text, Platform::Linux).unwrap()
    }

    fn command(id: &str) -> MatchResult {
        MatchResult::Command(CommandId::new(id).unwrap())
    }

    #[test]
    fn single_keystroke_in_global_context() {
        let mut matcher = KeymapMatcher::new();
        assert_eq!(
            matcher.push(&keymap(), key("ctrl-q"), &[]),
            command("app.quit")
        );
        assert_eq!(
            matcher.push(&keymap(), key("ctrl-x"), &[]),
            MatchResult::NoMatch
        );
    }

    #[test]
    fn inner_context_wins() {
        let mut matcher = KeymapMatcher::new();
        assert_eq!(
            matcher.push(&keymap(), key("ctrl-s"), &["editor", "workbench"]),
            command("editor.save")
        );
        assert_eq!(
            matcher.push(&keymap(), key("ctrl-s"), &["terminal"]),
            command("workbench.saveAll")
        );
    }

    #[test]
    fn sequences_wait_for_the_next_keystroke() {
        let mut matcher = KeymapMatcher::new();
        assert_eq!(
            matcher.push(&keymap(), key("ctrl-k"), &[]),
            MatchResult::Pending
        );
        assert_eq!(matcher.pending(), &[key("ctrl-k")]);
        assert_eq!(
            matcher.push(&keymap(), key("ctrl-s"), &[]),
            command("workbench.openKeymap")
        );
        assert!(matcher.pending().is_empty());
    }

    #[test]
    fn ambiguous_prefix_reports_its_own_command() {
        let mut keymap = keymap();
        keymap
            .apply(
                "[global]\n\"ctrl-k\" = \"workbench.quickOpen\"",
                Platform::Linux,
            )
            .unwrap();
        let mut matcher = KeymapMatcher::new();
        assert_eq!(
            matcher.push(&keymap, key("ctrl-k"), &[]),
            MatchResult::Pending
        );
        assert_eq!(
            matcher.pending_command(&keymap, &[]),
            Some(CommandId::new("workbench.quickOpen").unwrap())
        );
        assert_eq!(
            matcher.push(&keymap, key("ctrl-s"), &[]),
            command("workbench.openKeymap")
        );
        assert_eq!(matcher.pending_command(&keymap, &[]), None);
    }

    #[test]
    fn broken_sequence_is_dropped() {
        let mut matcher = KeymapMatcher::new();
        matcher.push(&keymap(), key("ctrl-k"), &[]);
        assert_eq!(matcher.push(&keymap(), key("x"), &[]), MatchResult::NoMatch);
        assert!(matcher.pending().is_empty());

        matcher.push(&keymap(), key("ctrl-k"), &[]);
        matcher.reset();
        assert_eq!(
            matcher.push(&keymap(), key("ctrl-s"), &[]),
            command("workbench.saveAll")
        );
    }
}

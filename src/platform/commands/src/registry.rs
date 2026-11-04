//! Registry of known commands, populated at startup and read-only
//! afterwards.

use std::collections::BTreeMap;

use crate::CommandId;

/// What is known about a command independently of how it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDescriptor {
    /// Stable id.
    pub id: CommandId,
    /// Id of the localized message with the title shown in the palette and
    /// menus.
    pub title_message: String,
}

impl CommandDescriptor {
    /// Describes command `id` whose title is the localized message
    /// `title_message`.
    pub fn new(id: CommandId, title_message: impl Into<String>) -> Self {
        Self {
            id,
            title_message: title_message.into(),
        }
    }
}

/// Errors while building a [`CommandRegistry`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// The same id was registered twice.
    #[error("command `{0}` is registered more than once")]
    Duplicate(CommandId),
}

/// All known commands, by id.
#[derive(Debug, Default)]
pub struct CommandRegistry {
    commands: BTreeMap<CommandId, CommandDescriptor>,
}

impl CommandRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a command.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Duplicate`] if the id is already taken.
    pub fn register(&mut self, descriptor: CommandDescriptor) -> Result<(), RegistryError> {
        if self.commands.contains_key(&descriptor.id) {
            return Err(RegistryError::Duplicate(descriptor.id));
        }
        self.commands.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    /// The command with `id`, if registered.
    pub fn get(&self, id: &CommandId) -> Option<&CommandDescriptor> {
        self.commands.get(id)
    }

    /// Whether `id` is registered.
    pub fn contains(&self, id: &CommandId) -> bool {
        self.commands.contains_key(id)
    }

    /// All commands, ordered by id.
    pub fn iter(&self) -> impl Iterator<Item = &CommandDescriptor> {
        self.commands.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> CommandId {
        CommandId::new(text).unwrap()
    }

    #[test]
    fn registers_and_finds_commands() {
        let mut registry = CommandRegistry::new();
        registry
            .register(CommandDescriptor::new(
                id("editor.save"),
                "command-editor-save",
            ))
            .unwrap();

        assert_eq!(
            registry.get(&id("editor.save")).unwrap().title_message,
            "command-editor-save"
        );
        assert!(!registry.contains(&id("editor.close")));
        assert_eq!(registry.iter().count(), 1);
    }

    #[test]
    fn rejects_duplicates() {
        let mut registry = CommandRegistry::new();
        registry
            .register(CommandDescriptor::new(id("app.quit"), "a"))
            .unwrap();
        assert_eq!(
            registry.register(CommandDescriptor::new(id("app.quit"), "b")),
            Err(RegistryError::Duplicate(id("app.quit")))
        );
    }
}

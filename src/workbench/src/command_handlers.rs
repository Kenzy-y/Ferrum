//! What runs when a command is invoked. Each owner registers handlers for
//! its own command ids next to the code that implements them; the window
//! root looks a command up here instead of matching ids by hand.
//!
//! The command *descriptors* (id, title) live in `commands::CommandRegistry`,
//! which has no UI. Handlers need a window, so they live here, one table
//! per window, filled when the window opens and read-only afterwards.

use std::collections::HashMap;
use std::rc::Rc;

use commands::{CommandId, RegistryError};
use gpui::{App, Context, WeakEntity, Window};

/// Runs one command in a window.
pub type CommandHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The handlers of every command available in a window.
#[derive(Default)]
pub struct CommandHandlers {
    handlers: HashMap<CommandId, CommandHandler>,
}

impl CommandHandlers {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the handler of `id`.
    ///
    /// # Errors
    ///
    /// Fails if `id` already has a handler.
    pub fn register(
        &mut self,
        id: CommandId,
        handler: CommandHandler,
    ) -> Result<(), RegistryError> {
        if self.handlers.contains_key(&id) {
            return Err(RegistryError::Duplicate(id));
        }
        self.handlers.insert(id, handler);
        Ok(())
    }

    /// Registers a handler that runs `run` on `entity`; nothing happens once
    /// the entity is gone. Ids that are not valid command ids are skipped
    /// (they are constants, covered by the owners' tests).
    ///
    /// # Errors
    ///
    /// Fails if `id` already has a handler.
    pub fn register_entity<T: 'static>(
        &mut self,
        id: &str,
        entity: &WeakEntity<T>,
        run: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Result<(), RegistryError> {
        let Ok(id) = CommandId::new(id) else {
            tracing::error!(id, "not a valid command id");
            return Ok(());
        };
        let entity = entity.clone();
        self.register(
            id,
            Rc::new(move |window, cx| {
                let _ = entity.update(cx, |this, cx| run(this, window, cx));
            }),
        )
    }

    /// Runs the handler of `id`. Returns `false` if it has none.
    pub fn run(&self, id: &CommandId, window: &mut Window, cx: &mut App) -> bool {
        match self.handlers.get(id) {
            Some(handler) => {
                let handler = handler.clone();
                handler(window, cx);
                true
            }
            None => false,
        }
    }

    /// Whether `id` has a handler.
    pub fn contains(&self, id: &CommandId) -> bool {
        self.handlers.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_has_one_handler() {
        let mut handlers = CommandHandlers::new();
        let id = CommandId::new("test.run").unwrap();
        handlers.register(id.clone(), Rc::new(|_, _| {})).unwrap();
        assert!(handlers.contains(&id));
        assert!(matches!(
            handlers.register(id.clone(), Rc::new(|_, _| {})),
            Err(RegistryError::Duplicate(duplicate)) if duplicate == id
        ));
    }
}

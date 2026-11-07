//! Running language services: one per server and workspace folder, shared
//! by every window and editor of that folder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use language::{LanguageEntry, LanguageService, ServerContext};
use process::ProcessRunner;
use settings::SettingsStore;

/// The services, keyed by server id and workspace root.
pub(crate) struct LanguageServers {
    process: Arc<dyn ProcessRunner>,
    services: HashMap<(String, PathBuf), Arc<dyn LanguageService>>,
}

impl LanguageServers {
    pub(crate) fn new(process: Arc<dyn ProcessRunner>) -> Self {
        Self {
            process,
            services: HashMap::new(),
        }
    }

    /// The service for files of `language` in the folder `root`, started
    /// on first use, and whether this call started it. `None` if the
    /// language has no server.
    pub(crate) fn service(
        &mut self,
        language: &LanguageEntry,
        root: &Path,
        settings: &SettingsStore,
    ) -> Option<(Arc<dyn LanguageService>, bool)> {
        let factory = language.server()?;
        let key = (factory.id().to_owned(), root.to_path_buf());
        let process = self.process.clone();
        let started = !self.services.contains_key(&key);
        let service = self.services.entry(key).or_insert_with(|| {
            tracing::info!(server = factory.id(), root = %root.display(), "starting language server");
            factory.start(ServerContext {
                root: root.to_path_buf(),
                process,
                settings,
            })
        });
        Some((service.clone(), started))
    }

    /// Stops the services of `root` (all with `None`) and forgets them, so
    /// the next [`LanguageServers::service`] starts new ones. Returns
    /// futures that end when they have stopped.
    pub(crate) fn stop(&mut self, root: Option<&Path>) -> Vec<language::BoxFuture<()>> {
        let keys: Vec<_> = self
            .services
            .keys()
            .filter(|(_, folder)| root.is_none_or(|root| folder == root))
            .cloned()
            .collect();
        keys.into_iter()
            .filter_map(|key| self.services.remove(&key))
            .map(|service| service.shutdown())
            .collect()
    }

    /// Passes changed settings to every service.
    pub(crate) fn configure(&self, settings: &SettingsStore) {
        for service in self.services.values() {
            service.configure(settings);
        }
    }
}

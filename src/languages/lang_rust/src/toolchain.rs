//! The Rust toolchain for the status bar and the breadcrumbs (ADR 0011):
//! the `rustc` version of a folder (rustup picks it by
//! `rust-toolchain.toml`) and the edition of a file's crate.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs::Fs;
use language::{BoxFuture, FileDetail, Toolchain};
use process::{CommandSpec, ProcessRunner};

/// Message of the edition in the breadcrumbs; takes `$value`.
const EDITION_MESSAGE: &str = "lang-rust-edition";

/// Edition of a package that does not name one.
const DEFAULT_EDITION: &str = "2015";

/// Folders above a file searched for its `Cargo.toml`.
const MAX_MANIFEST_DEPTH: usize = 32;

/// `rustc` and Cargo manifests.
pub(crate) struct RustToolchain;

impl Toolchain for RustToolchain {
    fn version(
        &self,
        root: Option<PathBuf>,
        process: Arc<dyn ProcessRunner>,
    ) -> BoxFuture<Option<String>> {
        Box::pin(async move {
            let mut command = CommandSpec::new("rustc").arg("--version");
            // rustup picks the toolchain by the folder; without one, the
            // home folder gives the default toolchain rather than whatever
            // the IDE was started in.
            if let Some(folder) = root.or_else(std::env::home_dir) {
                command = command.current_dir(folder);
            }
            let output = process.run(&command).await.ok()?;
            if !output.success() {
                return None;
            }
            parse_version(&String::from_utf8_lossy(&output.stdout))
        })
    }

    fn file_detail(&self, file: &Path, fs: Arc<dyn Fs>) -> BoxFuture<Option<FileDetail>> {
        let file = file.to_path_buf();
        Box::pin(async move {
            let edition = edition(fs.as_ref(), &file).await?;
            Some(FileDetail {
                message_id: EDITION_MESSAGE,
                value: edition,
            })
        })
    }
}

/// `1.82.0` from `rustc 1.82.0 (f6e511eec 2024-10-15)`.
fn parse_version(output: &str) -> Option<String> {
    let mut words = output.split_whitespace();
    (words.next()? == "rustc").then_some(())?;
    words.next().map(str::to_owned)
}

/// What a manifest says about the edition.
#[derive(Debug, PartialEq, Eq)]
enum Manifest {
    /// A package with its edition.
    Package(String),
    /// A package that takes the workspace's edition.
    Inherited,
    /// A workspace root, with the edition its packages inherit.
    Workspace(Option<String>),
    /// Neither a package nor a workspace.
    Other,
}

fn parse_manifest(text: &str) -> Manifest {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Manifest::Other;
    };
    let string =
        |value: Option<&toml::Value>| value.and_then(toml::Value::as_str).map(str::to_owned);
    if let Some(package) = table.get("package").and_then(toml::Value::as_table) {
        return match package.get("edition") {
            Some(toml::Value::Table(edition)) if edition.get("workspace").is_some() => {
                Manifest::Inherited
            }
            edition => {
                Manifest::Package(string(edition).unwrap_or_else(|| DEFAULT_EDITION.to_owned()))
            }
        };
    }
    match table.get("workspace").and_then(toml::Value::as_table) {
        Some(workspace) => Manifest::Workspace(string(
            workspace
                .get("package")
                .and_then(toml::Value::as_table)
                .and_then(|package| package.get("edition")),
        )),
        None => Manifest::Other,
    }
}

/// Edition of the crate of `file`: its nearest package manifest, or the
/// workspace manifest above it when the package inherits the edition.
async fn edition(fs: &dyn Fs, file: &Path) -> Option<String> {
    let mut inherited = false;
    for folder in file.ancestors().skip(1).take(MAX_MANIFEST_DEPTH) {
        let manifest = folder.join("Cargo.toml");
        let Ok(text) = fs.read_to_string(&manifest).await else {
            continue;
        };
        match parse_manifest(&text) {
            Manifest::Package(edition) if !inherited => return Some(edition),
            Manifest::Inherited if !inherited => inherited = true,
            Manifest::Workspace(edition) if inherited => return edition,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use fs::FakeFs;

    use super::*;

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("FakeFs futures complete on first poll"),
        }
    }

    #[test]
    fn the_version_is_the_second_word() {
        assert_eq!(
            parse_version("rustc 1.82.0 (f6e511eec 2024-10-15)\n"),
            Some("1.82.0".to_owned())
        );
        assert_eq!(parse_version("cargo 1.82.0"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn the_edition_comes_from_the_nearest_package() {
        let fs = FakeFs::new();
        fs.insert_file(
            "/w/Cargo.toml",
            "[workspace]\n[workspace.package]\nedition = \"2024\"\n",
        );
        fs.insert_file(
            "/w/app/Cargo.toml",
            "[package]\nname = \"app\"\nedition = \"2021\"\n",
        );
        fs.insert_file("/w/app/src/main.rs", "fn main() {}\n");
        fs.insert_file(
            "/w/lib/Cargo.toml",
            "[package]\nname = \"lib\"\nedition.workspace = true\n",
        );
        fs.insert_file("/w/lib/src/lib.rs", "");
        fs.insert_file("/old/Cargo.toml", "[package]\nname = \"old\"\n");
        fs.insert_file("/old/src/main.rs", "");
        let edition_of = |path: &str| ready(edition(&fs, Path::new(path)));
        assert_eq!(edition_of("/w/app/src/main.rs"), Some("2021".to_owned()));
        assert_eq!(edition_of("/w/lib/src/lib.rs"), Some("2024".to_owned()));
        assert_eq!(edition_of("/old/src/main.rs"), Some("2015".to_owned()));
        assert_eq!(edition_of("/elsewhere/main.rs"), None);
    }
}

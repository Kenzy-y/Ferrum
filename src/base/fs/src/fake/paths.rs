//! Path resolution in the fake file system: symbolic links, `.` and `..`,
//! and metadata of the resolved node.

use std::path::{Component, Path, PathBuf};

use super::{Node, State};
use crate::{EntryKind, FsError, Metadata};

/// Maximum number of symbolic links followed while resolving a path.
const MAX_LINK_DEPTH: usize = 40;

/// Joins a link target to the link's directory and removes `.` / `..`.
fn resolve_target(link: &Path, target: &Path) -> PathBuf {
    let joined = if target.is_relative() {
        link.parent().unwrap_or(Path::new("")).join(target)
    } else {
        target.to_path_buf()
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

/// Resolves every symbolic link in `path`, component by component.
pub(super) fn canonical(state: &State, path: &Path) -> Result<PathBuf, FsError> {
    let mut resolved = PathBuf::new();
    let mut hops = 0;
    let mut pending: Vec<PathBuf> = path
        .components()
        .map(|component| PathBuf::from(component.as_os_str()))
        .collect();
    pending.reverse();
    while let Some(component) = pending.pop() {
        let candidate = resolved.join(&component);
        match state.nodes.get(&candidate) {
            Some(Node::Symlink(target)) => {
                hops += 1;
                if hops > MAX_LINK_DEPTH {
                    return Err(FsError::Io {
                        path: path.to_path_buf(),
                        source: std::io::Error::other("too many levels of symbolic links"),
                    });
                }
                let target = resolve_target(&candidate, target);
                let mut parts: Vec<PathBuf> = target
                    .components()
                    .map(|part| PathBuf::from(part.as_os_str()))
                    .collect();
                parts.reverse();
                resolved = PathBuf::new();
                pending.extend(parts);
            }
            // Missing components are kept: callers decide whether the
            // resolved path must exist.
            Some(_) | None => resolved = candidate,
        }
    }
    Ok(resolved)
}

pub(super) fn metadata_of(state: &State, path: &Path) -> Option<Metadata> {
    let node = state.nodes.get(path)?;
    let is_symlink = matches!(node, Node::Symlink(_));
    let target = if is_symlink {
        canonical(state, path)
            .ok()
            .and_then(|resolved| state.nodes.get(&resolved))
    } else {
        Some(node)
    };
    let metadata = match target {
        Some(Node::File {
            contents,
            read_only,
            modified,
        }) => Metadata {
            kind: EntryKind::File,
            is_symlink,
            len: u64::try_from(contents.len()).unwrap_or(u64::MAX),
            modified: Some(*modified),
            read_only: *read_only,
        },
        Some(Node::Dir) => Metadata {
            kind: EntryKind::Dir,
            is_symlink,
            len: 0,
            modified: None,
            read_only: false,
        },
        Some(Node::Symlink(_)) | None => Metadata {
            kind: EntryKind::Other,
            is_symlink,
            len: 0,
            modified: None,
            read_only: false,
        },
    };
    Some(metadata)
}

//! File paths as `file://` URIs and back.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use lsp_types::Uri;

/// Bytes kept as they are in a URI path; the rest is percent-encoded.
fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':')
}

/// The `file://` URI of an absolute path. `None` if the path is relative
/// or not valid Unicode.
pub(crate) fn path_to_uri(path: &Path) -> Option<Uri> {
    if !path.is_absolute() {
        return None;
    }
    let text = path.to_str()?;
    let slashed = if cfg!(windows) {
        // `C:\a` → `/C:/a`.
        format!("/{}", text.replace('\\', "/"))
    } else {
        text.to_owned()
    };
    let mut uri = String::with_capacity(slashed.len() + 8);
    uri.push_str("file://");
    for byte in slashed.bytes() {
        if is_unreserved(byte) {
            uri.push(char::from(byte));
        } else {
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    Uri::from_str(&uri).ok()
}

/// The path of a `file://` URI. `None` for other schemes.
pub(crate) fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let rest = uri.as_str().strip_prefix("file://")?;
    // Skip an authority (`localhost`), keep the path.
    let path = &rest[rest.find('/')?..];
    let decoded = percent_decode(path)?;
    if cfg!(windows) {
        // Servers write the drive letter in either case; paths in the IDE
        // have it upper case.
        let drive = match decoded.as_bytes() {
            [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => Some(*drive),
            _ => None,
        };
        let path = match drive {
            Some(drive) => format!(
                "{}{}",
                char::from(drive.to_ascii_uppercase()),
                &decoded[2..]
            ),
            None => decoded,
        };
        Some(PathBuf::from(path.replace('/', "\\")))
    } else {
        Some(PathBuf::from(decoded))
    }
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute(path: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:{}", path.replace('/', "\\")))
        } else {
            PathBuf::from(path)
        }
    }

    #[test]
    fn round_trips_paths_with_spaces_and_unicode() {
        let path = absolute("/projects/my app/src/главный #1.rs");
        let uri = path_to_uri(&path).unwrap();
        assert!(uri.as_str().starts_with("file:///"), "{}", uri.as_str());
        assert!(uri.as_str().contains("my%20app"), "{}", uri.as_str());
        assert!(uri.as_str().contains("%231.rs"), "{}", uri.as_str());
        assert_eq!(uri_to_path(&uri).unwrap(), path);
    }

    #[test]
    fn reads_uris_written_by_servers() {
        let uri = Uri::from_str(if cfg!(windows) {
            "file:///c%3A/projects/a.rs"
        } else {
            "file://localhost/projects/a.rs"
        })
        .unwrap();
        let expected = if cfg!(windows) {
            PathBuf::from("C:\\projects\\a.rs")
        } else {
            PathBuf::from("/projects/a.rs")
        };
        assert_eq!(uri_to_path(&uri).unwrap(), expected);
        assert!(uri_to_path(&Uri::from_str("untitled:1").unwrap()).is_none());
        assert!(path_to_uri(Path::new("relative.rs")).is_none());
    }
}

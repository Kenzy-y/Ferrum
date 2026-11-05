//! The base protocol: `Content-Length` headers, a blank line, a JSON body.

use std::io::{BufRead, Write};

/// A body larger than this is a broken stream, not a message.
const MAX_MESSAGE_BYTES: usize = 512 * 1024 * 1024;

fn invalid(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.to_owned())
}

/// Reads one message body. `None` at the end of the stream.
///
/// # Errors
///
/// Fails on a read error or a malformed header.
pub(crate) fn read_frame(reader: &mut impl BufRead) -> std::io::Result<Option<Vec<u8>>> {
    let mut length = None;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            if length.is_some() {
                break;
            }
            // Stray blank lines between messages.
            continue;
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(invalid("malformed header"));
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            let value: usize = value
                .trim()
                .parse()
                .map_err(|_| invalid("bad Content-Length"))?;
            if value > MAX_MESSAGE_BYTES {
                return Err(invalid("message too large"));
            }
            length = Some(value);
        }
    }
    let mut body = vec![0; length.unwrap_or_default()];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// Writes one message body with its header and flushes.
///
/// # Errors
///
/// Fails if the stream is closed.
pub(crate) fn write_frame(writer: &mut impl Write, body: &[u8]) -> std::io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Read};

    use super::*;

    /// Hands out its bytes a few at a time, like a pipe under load.
    struct Trickle {
        bytes: Vec<u8>,
        position: usize,
        step: usize,
    }

    impl Read for Trickle {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let end = (self.position + self.step)
                .min(self.bytes.len())
                .min(self.position + buffer.len());
            let count = end - self.position;
            buffer[..count].copy_from_slice(&self.bytes[self.position..end]);
            self.position = end;
            Ok(count)
        }
    }

    #[test]
    fn reads_back_to_back_and_split_messages() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, br#"{"a":1}"#).unwrap();
        bytes.extend_from_slice(b"Content-Type: application/vscode-jsonrpc; charset=utf-8\r\n");
        write_frame(&mut bytes, "{\"ü\":2}".as_bytes()).unwrap();
        for step in [1, 3, 7, 1000] {
            let mut reader = BufReader::with_capacity(
                4,
                Trickle {
                    bytes: bytes.clone(),
                    position: 0,
                    step,
                },
            );
            assert_eq!(read_frame(&mut reader).unwrap().unwrap(), br#"{"a":1}"#);
            assert_eq!(
                read_frame(&mut reader).unwrap().unwrap(),
                "{\"ü\":2}".as_bytes()
            );
            assert!(read_frame(&mut reader).unwrap().is_none());
        }
    }

    #[test]
    fn rejects_garbage() {
        let mut reader = BufReader::new(&b"Content-Length: many\r\n\r\n"[..]);
        assert!(read_frame(&mut reader).is_err());
        let mut reader = BufReader::new(&b"no colon\r\n\r\n"[..]);
        assert!(read_frame(&mut reader).is_err());
    }
}

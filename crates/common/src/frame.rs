//! Length-prefixed JSON framing over a byte stream (TCP).
//!
//! Each message on the wire is:
//!
//! ```text
//! +-----------------------+------------------------+
//! | u32 length (4 bytes)  | JSON payload (N bytes) |
//! | big-endian            |                        |
//! +-----------------------+------------------------+
//! ```
//!
//! The length prefix lets the reader know exactly how many bytes to pull
//! off the stream before attempting to deserialize, since TCP gives us a
//! stream with no inherent message boundaries.

use std::io::{self, Read, Write};

use serde::{de::DeserializeOwned, Serialize};

/// Reject frames larger than this to avoid a malicious peer asking us to
/// allocate gigabytes. 16 MiB is generous for control messages.
const MAX_FRAME_LEN: u32 = 16 * 1024 * 1024;

/// Errors that can occur while reading or writing a frame.
#[derive(Debug)]
pub enum FrameError {
    /// The underlying stream errored (includes a clean EOF).
    Io(io::Error),
    /// The frame's declared length exceeded [`MAX_FRAME_LEN`].
    FrameTooLarge(u32),
    /// The payload was not valid JSON for the expected type.
    Decode(serde_json::Error),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::Io(e) => write!(f, "i/o error: {e}"),
            FrameError::FrameTooLarge(n) => {
                write!(f, "frame of {n} bytes exceeds limit of {MAX_FRAME_LEN}")
            }
            FrameError::Decode(e) => write!(f, "decode error: {e}"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        FrameError::Io(e)
    }
}

impl From<serde_json::Error> for FrameError {
    fn from(e: serde_json::Error) -> Self {
        FrameError::Decode(e)
    }
}

/// Serialize `msg` to JSON and write it as a single length-prefixed frame.
pub fn write_message<W: Write, T: Serialize>(writer: &mut W, msg: &T) -> Result<(), FrameError> {
    let payload = serde_json::to_vec(msg)?;
    let len = u32::try_from(payload.len()).map_err(|_| FrameError::FrameTooLarge(u32::MAX))?;
    if len > MAX_FRAME_LEN {
        return Err(FrameError::FrameTooLarge(len));
    }
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Read exactly one length-prefixed frame and deserialize it into `T`.
///
/// Returns `Err(FrameError::Io(..))` with [`io::ErrorKind::UnexpectedEof`]
/// when the peer closes the connection cleanly between messages.
pub fn read_message<R: Read, T: DeserializeOwned>(reader: &mut R) -> Result<T, FrameError> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf);
    if len > MAX_FRAME_LEN {
        return Err(FrameError::FrameTooLarge(len));
    }
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;
    let msg = serde_json::from_slice(&payload)?;
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{PowerAction, Request, Response};

    #[test]
    fn round_trips_a_request() {
        let original = Request::Power(PowerAction::Reset);
        let mut buf = Vec::new();
        write_message(&mut buf, &original).unwrap();

        let mut cursor = io::Cursor::new(buf);
        let decoded: Request = read_message(&mut cursor).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn round_trips_multiple_messages_on_one_stream() {
        let mut buf = Vec::new();
        write_message(&mut buf, &Request::Ping).unwrap();
        write_message(&mut buf, &Response::Pong).unwrap();

        let mut cursor = io::Cursor::new(buf);
        assert_eq!(
            Request::Ping,
            read_message::<_, Request>(&mut cursor).unwrap()
        );
        assert_eq!(
            Response::Pong,
            read_message::<_, Response>(&mut cursor).unwrap()
        );
    }

    #[test]
    fn clean_eof_is_reported() {
        let mut cursor = io::Cursor::new(Vec::new());
        let err = read_message::<_, Request>(&mut cursor).unwrap_err();
        match err {
            FrameError::Io(e) => assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof),
            other => panic!("expected Io eof, got {other:?}"),
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later

//! Length-prefixed message codec.
//!
//! Frames use a little-endian `u32` byte length followed by a compact binary
//! payload. The public shape mirrors the intended postcard transport while this
//! bootstrap implementation keeps the wire format local and dependency-light in
//! offline CI environments.

use std::borrow::Cow;
use std::io::{self, Read, Write};

use crate::messages::{Entry, EntryKind, EntryRef, Request, Response};

/// Maximum accepted frame length: 1 MiB.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Codec errors.
#[derive(Debug)]
pub enum CodecError {
    /// Underlying I/O error.
    Io(io::Error),
    /// Frame length exceeded [`MAX_FRAME_BYTES`].
    FrameTooLarge {
        /// Actual frame length.
        len: usize,
    },
    /// Invalid or unsupported wire value.
    InvalidData(&'static str),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::FrameTooLarge { len } => write!(f, "frame too large: {len} bytes"),
            Self::InvalidData(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for CodecError {}

impl From<io::Error> for CodecError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Convenient result type for codec operations.
pub type Result<T> = std::result::Result<T, CodecError>;

/// Trait for SayRat wire encoders.
pub trait WireEncode {
    /// Append this value to `out`.
    fn encode(&self, out: &mut Vec<u8>) -> Result<()>;
}

/// Trait for SayRat wire decoders.
pub trait WireDecode<'a>: Sized {
    /// Decode this value from `input`.
    fn decode(input: &mut Cursor<'a>) -> Result<Self>;
}

/// Read one framed message.
pub fn read_message<R, T>(reader: &mut R) -> Result<T>
where
    R: Read,
    for<'a> T: WireDecode<'a>,
{
    let mut len_buf = [0_u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(CodecError::FrameTooLarge { len });
    }
    let mut buf = vec![0_u8; len];
    reader.read_exact(&mut buf)?;
    let mut cursor = Cursor::new(&buf);
    T::decode(&mut cursor)
}

/// Write one framed message.
pub fn write_message<W, T>(writer: &mut W, message: &T) -> Result<()>
where
    W: Write,
    T: WireEncode,
{
    let mut buf = Vec::with_capacity(256);
    message.encode(&mut buf)?;
    if buf.len() > MAX_FRAME_BYTES {
        return Err(CodecError::FrameTooLarge { len: buf.len() });
    }
    writer.write_all(&(buf.len() as u32).to_le_bytes())?;
    writer.write_all(&buf)?;
    writer.flush()?;
    Ok(())
}

/// Async-shaped equivalent of [`read_message`].
pub async fn read_message_async<R, T>(reader: &mut R) -> Result<T>
where
    R: Read + Unpin,
    for<'a> T: WireDecode<'a>,
{
    read_message(reader)
}

/// Async-shaped equivalent of [`write_message`].
pub async fn write_message_async<W, T>(writer: &mut W, message: &T) -> Result<()>
where
    W: Write + Unpin,
    T: WireEncode,
{
    write_message(writer, message)
}

/// Borrowing decode cursor.
pub struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(len).ok_or(CodecError::InvalidData("cursor overflow"))?;
        let bytes =
            self.bytes.get(self.pos..end).ok_or(CodecError::InvalidData("truncated frame"))?;
        self.pos = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(*self.take(1)?.first().ok_or(CodecError::InvalidData("truncated byte"))?)
    }

    fn u16(&mut self) -> Result<u16> {
        let mut buf = [0; 2];
        buf.copy_from_slice(self.take(2)?);
        Ok(u16::from_le_bytes(buf))
    }

    fn u32(&mut self) -> Result<u32> {
        let mut buf = [0; 4];
        buf.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(buf))
    }

    fn u64(&mut self) -> Result<u64> {
        let mut buf = [0; 8];
        buf.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(buf))
    }

    fn str(&mut self) -> Result<&'a str> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes).map_err(|_| CodecError::InvalidData("invalid utf-8"))
    }
}

fn put_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, value: &str) -> Result<()> {
    let len =
        u32::try_from(value.len()).map_err(|_| CodecError::FrameTooLarge { len: value.len() })?;
    put_u32(out, len);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn put_opt_str(out: &mut Vec<u8>, value: Option<&str>) -> Result<()> {
    match value {
        Some(value) => {
            put_u8(out, 1);
            put_str(out, value)
        }
        None => {
            put_u8(out, 0);
            Ok(())
        }
    }
}

fn decode_opt_str<'a>(input: &mut Cursor<'a>) -> Result<Option<Cow<'a, str>>> {
    match input.u8()? {
        0 => Ok(None),
        1 => Ok(Some(Cow::Borrowed(input.str()?))),
        _ => Err(CodecError::InvalidData("invalid option tag")),
    }
}

impl WireEncode for EntryKind {
    fn encode(&self, out: &mut Vec<u8>) -> Result<()> {
        put_u8(
            out,
            match self {
                Self::Application => 0,
                Self::File => 1,
                Self::PluginCommand => 2,
            },
        );
        Ok(())
    }
}

impl<'a> WireDecode<'a> for EntryKind {
    fn decode(input: &mut Cursor<'a>) -> Result<Self> {
        match input.u8()? {
            0 => Ok(Self::Application),
            1 => Ok(Self::File),
            2 => Ok(Self::PluginCommand),
            _ => Err(CodecError::InvalidData("invalid entry kind")),
        }
    }
}

impl WireEncode for Entry {
    fn encode(&self, out: &mut Vec<u8>) -> Result<()> {
        put_u64(out, self.id);
        self.kind.encode(out)?;
        put_str(out, &self.name)?;
        put_opt_str(out, self.subtitle.as_deref())?;
        put_opt_str(out, self.exec.as_deref())?;
        put_opt_str(out, self.icon.as_deref())
    }
}

impl<'a> WireDecode<'a> for Entry {
    fn decode(input: &mut Cursor<'a>) -> Result<Self> {
        Ok(Self {
            id: input.u64()?,
            kind: EntryKind::decode(input)?,
            name: input.str()?.to_owned(),
            subtitle: decode_opt_str(input)?.map(Cow::into_owned),
            exec: decode_opt_str(input)?.map(Cow::into_owned),
            icon: decode_opt_str(input)?.map(Cow::into_owned),
        })
    }
}

impl WireEncode for EntryRef<'_> {
    fn encode(&self, out: &mut Vec<u8>) -> Result<()> {
        put_u64(out, self.id);
        self.kind.encode(out)?;
        put_str(out, &self.name)?;
        put_opt_str(out, self.subtitle.as_deref())?;
        put_opt_str(out, self.exec.as_deref())?;
        put_opt_str(out, self.icon.as_deref())
    }
}

impl<'de, 'a> WireDecode<'de> for EntryRef<'a> {
    fn decode(input: &mut Cursor<'de>) -> Result<Self> {
        Ok(Self {
            id: input.u64()?,
            kind: EntryKind::decode(input)?,
            name: Cow::Owned(input.str()?.to_owned()),
            subtitle: decode_opt_str(input)?.map(|value| Cow::Owned(value.into_owned())),
            exec: decode_opt_str(input)?.map(|value| Cow::Owned(value.into_owned())),
            icon: decode_opt_str(input)?.map(|value| Cow::Owned(value.into_owned())),
        })
    }
}

impl WireEncode for Request {
    fn encode(&self, out: &mut Vec<u8>) -> Result<()> {
        match self {
            Self::Hello { client_version } => {
                put_u8(out, 0);
                put_str(out, client_version)
            }
            Self::Ping => {
                put_u8(out, 1);
                Ok(())
            }
            Self::Shutdown => {
                put_u8(out, 2);
                Ok(())
            }
            Self::ListEntries { limit } => {
                put_u8(out, 3);
                put_u16(out, *limit);
                Ok(())
            }
        }
    }
}

impl<'a> WireDecode<'a> for Request {
    fn decode(input: &mut Cursor<'a>) -> Result<Self> {
        match input.u8()? {
            0 => Ok(Self::Hello { client_version: input.str()?.to_owned() }),
            1 => Ok(Self::Ping),
            2 => Ok(Self::Shutdown),
            3 => Ok(Self::ListEntries { limit: input.u16()? }),
            _ => Err(CodecError::InvalidData("invalid request tag")),
        }
    }
}

impl WireEncode for Response<'_> {
    fn encode(&self, out: &mut Vec<u8>) -> Result<()> {
        match self {
            Self::Hello { daemon_version, protocol_version } => {
                put_u8(out, 0);
                put_str(out, daemon_version)?;
                put_u16(out, *protocol_version);
                Ok(())
            }
            Self::Pong => {
                put_u8(out, 1);
                Ok(())
            }
            Self::Ack => {
                put_u8(out, 2);
                Ok(())
            }
            Self::Entries { items, more } => {
                put_u8(out, 3);
                put_u8(out, u8::from(*more));
                let len = u32::try_from(items.len())
                    .map_err(|_| CodecError::FrameTooLarge { len: items.len() })?;
                put_u32(out, len);
                for item in items {
                    item.encode(out)?;
                }
                Ok(())
            }
            Self::Error { message } => {
                put_u8(out, 4);
                put_str(out, message)
            }
        }
    }
}

impl<'de, 'a> WireDecode<'de> for Response<'a> {
    fn decode(input: &mut Cursor<'de>) -> Result<Self> {
        match input.u8()? {
            0 => Ok(Self::Hello {
                daemon_version: input.str()?.to_owned(),
                protocol_version: input.u16()?,
            }),
            1 => Ok(Self::Pong),
            2 => Ok(Self::Ack),
            3 => {
                let more = match input.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(CodecError::InvalidData("invalid bool")),
                };
                let len = input.u32()? as usize;
                let mut items = Vec::with_capacity(len.min(1024));
                for _ in 0..len {
                    items.push(EntryRef::decode(input)?);
                }
                Ok(Self::Entries { items, more })
            }
            4 => Ok(Self::Error { message: input.str()?.to_owned() }),
            _ => Err(CodecError::InvalidData("invalid response tag")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::messages::{EntryKind, EntryRef, Request, Response};

    fn round_trip<T>(value: &T) -> T
    where
        T: WireEncode + for<'a> WireDecode<'a> + PartialEq + std::fmt::Debug,
    {
        let mut bytes = Vec::new();
        write_message(&mut bytes, value).unwrap_or_else(|err| panic!("write failed: {err}"));
        let mut cursor = &bytes[..];
        let decoded = read_message(&mut cursor).unwrap_or_else(|err| panic!("read failed: {err}"));
        assert_eq!(value, &decoded);
        decoded
    }

    fn round_trip_response(value: &Response<'_>) {
        let mut bytes = Vec::new();
        write_message(&mut bytes, value).unwrap_or_else(|err| panic!("write failed: {err}"));
        let mut cursor = &bytes[..];
        let decoded: Response<'_> =
            read_message(&mut cursor).unwrap_or_else(|err| panic!("read failed: {err}"));
        assert_eq!(value, &decoded);
    }

    #[test]
    fn request_round_trips_cover_all_variants() {
        round_trip(&Request::Hello { client_version: String::from("ui/0") });
        round_trip(&Request::Ping);
        round_trip(&Request::Shutdown);
        round_trip(&Request::ListEntries { limit: 25 });
    }

    #[test]
    fn response_round_trips_cover_all_variants() {
        round_trip_response(&Response::Hello {
            daemon_version: String::from("d/0"),
            protocol_version: 1,
        });
        round_trip_response(&Response::Pong);
        round_trip_response(&Response::Ack);
        round_trip_response(&Response::Entries {
            items: vec![EntryRef {
                id: 9,
                kind: EntryKind::Application,
                name: Cow::Borrowed("Terminal"),
                subtitle: Some(Cow::Borrowed("System")),
                exec: Some(Cow::Borrowed("xterm")),
                icon: Some(Cow::Borrowed("utilities-terminal")),
            }],
            more: false,
        });
        round_trip_response(&Response::Error { message: String::from("nope") });
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let mut bytes = ((MAX_FRAME_BYTES as u32) + 1).to_le_bytes().to_vec();
        let err = read_message::<_, Request>(&mut &bytes[..])
            .err()
            .unwrap_or_else(|| panic!("wanted error"));
        assert!(matches!(err, CodecError::FrameTooLarge { .. }));
        bytes.clear();
    }
}

//! Panic-free little-endian reading and writing.
//!
//! Every field read goes through [`Reader`], which returns a typed
//! [`WireError::Truncated`] rather than indexing out of bounds, so the decoders
//! can never panic on hostile input. [`Writer`] is a thin append-only wrapper
//! over `Vec<u8>`.

use crate::error::WireError;

/// A bounds-checked cursor over a byte slice.
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Wrap a slice.
    pub(crate) const fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// The number of unread bytes.
    pub(crate) const fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// The number of bytes read so far.
    pub(crate) const fn consumed(&self) -> usize {
        self.pos
    }

    /// Take the next `n` bytes, or fail with [`WireError::Truncated`].
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.pos.checked_add(n).ok_or(WireError::Truncated {
            needed: n,
            actual: self.remaining(),
        })?;
        if end > self.buf.len() {
            return Err(WireError::Truncated {
                needed: n,
                actual: self.remaining(),
            });
        }
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// Read a fixed-size byte array.
    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    /// Read a `u8`.
    pub(crate) fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.array::<1>()?[0])
    }

    /// Read an `i8`.
    pub(crate) fn i8(&mut self) -> Result<i8, WireError> {
        Ok(self.u8()? as i8)
    }

    /// Read a little-endian `u16`.
    pub(crate) fn u16(&mut self) -> Result<u16, WireError> {
        Ok(u16::from_le_bytes(self.array::<2>()?))
    }

    /// Read a little-endian `i16`.
    pub(crate) fn i16(&mut self) -> Result<i16, WireError> {
        Ok(i16::from_le_bytes(self.array::<2>()?))
    }

    /// Read a little-endian `u32`.
    pub(crate) fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.array::<4>()?))
    }

    /// Read a little-endian `u64`.
    pub(crate) fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_le_bytes(self.array::<8>()?))
    }

    /// Read a one-byte-length-prefixed UTF-8 string.
    pub(crate) fn string_u8(&mut self) -> Result<String, WireError> {
        let len = self.u8()? as usize;
        let bytes = self.take(len)?;
        core::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| WireError::InvalidUtf8)
    }
}

/// An append-only little-endian writer over a `Vec<u8>`.
#[derive(Default)]
pub(crate) struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// A fresh, empty writer.
    pub(crate) fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Append a `u8`.
    pub(crate) fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    /// Append an `i8`.
    pub(crate) fn i8(&mut self, value: i8) {
        self.buf.push(value as u8);
    }

    /// Append a little-endian `u16`.
    pub(crate) fn u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Append a little-endian `i16`.
    pub(crate) fn i16(&mut self, value: i16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Append a little-endian `u32`.
    pub(crate) fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Append a little-endian `u64`.
    pub(crate) fn u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Append raw bytes.
    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.buf.extend_from_slice(value);
    }

    /// Append a one-byte-length-prefixed UTF-8 string.
    pub(crate) fn string_u8(&mut self, value: &str) -> Result<(), WireError> {
        let len = value.len();
        if len > crate::constants::MAX_CONTROL_STRING {
            return Err(WireError::StringTooLong {
                len,
                max: crate::constants::MAX_CONTROL_STRING,
            });
        }
        self.u8(len as u8);
        self.bytes(value.as_bytes());
        Ok(())
    }

    /// Consume the writer, yielding its bytes.
    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.buf
    }
}

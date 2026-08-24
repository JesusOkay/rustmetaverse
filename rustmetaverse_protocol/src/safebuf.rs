//! Bounds-checked buffer parsing primitives for untrusted network data.
//!
//! The `bytes::Buf` trait methods (`get_u8`, `get_u32_le`, `copy_to_slice`,
//! etc.) **panic** when the buffer does not contain enough bytes. For parsing
//! UDP packets from a Second Life simulator — which can be truncated, malformed,
//! or simply use a protocol variant we don't fully support — a panic is
//! unacceptable: it kills the receive task and drops the connection.
//!
//! `SafeBuf` wraps a `Bytes` and checks `remaining() >= N` before every read.
//! On underflow it returns `io::Error(UnexpectedEof)` instead of panicking.
//!
//! ## Design
//!
//! `SafeBuf` is a thin newtype around `Bytes`. It derefs to `Bytes` for
//! read-only access (len, remaining, slicing) but intercepts all consuming
//! reads. It is zero-cost in release builds — the bounds check that `bytes`
//! would do internally as a debug_assert / panic becomes a proper `Result`.

use bytes::{Buf, Bytes};
use std::io;

/// A bounds-checked reader over `Bytes`.
///
/// Every consuming read validates `remaining() >= required` before advancing
/// the internal cursor. On underflow, `UnexpectedEof` is returned.
#[derive(Debug, Clone)]
pub struct SafeBuf {
    inner: Bytes,
}

impl SafeBuf {
    /// Wrap an existing `Bytes` for safe reading.
    #[inline]
    pub fn new(buf: Bytes) -> Self {
        SafeBuf { inner: buf }
    }

    /// Number of bytes remaining before the cursor.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.inner.remaining()
    }

    /// Total length of the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Borrow the underlying bytes as a byte slice (does not advance).
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
    }

    /// Consume and return the inner `Bytes`.
    #[inline]
    pub fn into_inner(self) -> Bytes {
        self.inner
    }

    /// Ensure at least `n` bytes remain, or return `UnexpectedEof`.
    #[inline]
    fn require(&self, n: usize) -> io::Result<()> {
        if self.inner.remaining() >= n {
            Ok(())
        } else {
            Err(eof(n, self.inner.remaining()))
        }
    }

    // ---- Fixed-size reads --------------------------------------------------

    /// Read one unsigned byte.
    #[inline]
    pub fn read_u8(&mut self) -> io::Result<u8> {
        self.require(1)?;
        Ok(self.inner.get_u8())
    }

    /// Read one signed byte.
    #[inline]
    pub fn read_i8(&mut self) -> io::Result<i8> {
        self.require(1)?;
        Ok(self.inner.get_i8())
    }

    /// Read a big-endian u16.
    #[inline]
    pub fn read_u16(&mut self) -> io::Result<u16> {
        self.require(2)?;
        Ok(self.inner.get_u16())
    }

    /// Read a little-endian u16.
    #[inline]
    pub fn read_u16_le(&mut self) -> io::Result<u16> {
        self.require(2)?;
        Ok(self.inner.get_u16_le())
    }

    /// Read a big-endian i16.
    #[inline]
    pub fn read_i16(&mut self) -> io::Result<i16> {
        self.require(2)?;
        Ok(self.inner.get_i16())
    }

    /// Read a little-endian i16.
    #[inline]
    pub fn read_i16_le(&mut self) -> io::Result<i16> {
        self.require(2)?;
        Ok(self.inner.get_i16_le())
    }

    /// Read a big-endian u32.
    #[inline]
    pub fn read_u32(&mut self) -> io::Result<u32> {
        self.require(4)?;
        Ok(self.inner.get_u32())
    }

    /// Read a little-endian u32.
    #[inline]
    pub fn read_u32_le(&mut self) -> io::Result<u32> {
        self.require(4)?;
        Ok(self.inner.get_u32_le())
    }

    /// Read a big-endian i32.
    #[inline]
    pub fn read_i32(&mut self) -> io::Result<i32> {
        self.require(4)?;
        Ok(self.inner.get_i32())
    }

    /// Read a little-endian i32.
    #[inline]
    pub fn read_i32_le(&mut self) -> io::Result<i32> {
        self.require(4)?;
        Ok(self.inner.get_i32_le())
    }

    /// Read a big-endian u64.
    #[inline]
    pub fn read_u64(&mut self) -> io::Result<u64> {
        self.require(8)?;
        Ok(self.inner.get_u64())
    }

    /// Read a little-endian u64.
    #[inline]
    pub fn read_u64_le(&mut self) -> io::Result<u64> {
        self.require(8)?;
        Ok(self.inner.get_u64_le())
    }

    /// Read a big-endian f32.
    #[inline]
    pub fn read_f32(&mut self) -> io::Result<f32> {
        self.require(4)?;
        Ok(self.inner.get_f32())
    }

    /// Read a little-endian f32.
    #[inline]
    pub fn read_f32_le(&mut self) -> io::Result<f32> {
        self.require(4)?;
        Ok(self.inner.get_f32_le())
    }

    /// Read a big-endian f64.
    #[inline]
    pub fn read_f64(&mut self) -> io::Result<f64> {
        self.require(8)?;
        Ok(self.inner.get_f64())
    }

    /// Read a little-endian f64.
    #[inline]
    pub fn read_f64_le(&mut self) -> io::Result<f64> {
        self.require(8)?;
        Ok(self.inner.get_f64_le())
    }

    // ---- Variable-size reads -----------------------------------------------

    /// Copy exactly `n` bytes into `dst`, advancing the cursor.
    ///
    /// Panics only if `dst.len() != n` (programmer error), never on underflow.
    #[inline]
    pub fn read_bytes_into(&mut self, dst: &mut [u8]) -> io::Result<()> {
        self.require(dst.len())?;
        self.inner.copy_to_slice(dst);
        Ok(())
    }

    /// Read `n` bytes as an owned `Vec<u8>`.
    #[inline]
    pub fn read_vec(&mut self, n: usize) -> io::Result<Vec<u8>> {
        self.require(n)?;
        let mut v = vec![0u8; n];
        self.inner.copy_to_slice(&mut v);
        Ok(v)
    }

    /// Read a fixed-size array.
    #[inline]
    pub fn read_array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        self.require(N)?;
        let mut arr = [0u8; N];
        self.inner.copy_to_slice(&mut arr);
        Ok(arr)
    }

    /// Read a length-prefixed string where the 1-byte prefix gives the length.
    #[inline]
    pub fn read_string_u8(&mut self) -> io::Result<Vec<u8>> {
        let len = self.read_u8()? as usize;
        self.read_vec(len)
    }

    /// Read a length-prefixed string where the 2-byte LE prefix gives length.
    #[inline]
    pub fn read_string_u16_le(&mut self) -> io::Result<Vec<u8>> {
        let len = self.read_u16_le()? as usize;
        self.read_vec(len)
    }

    /// Read a fixed-size NUL-padded byte buffer (common in LLUDP).
    #[inline]
    pub fn read_fixed_bytes<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        self.read_array()
    }

    /// Skip `n` bytes without reading them.
    #[inline]
    pub fn skip(&mut self, n: usize) -> io::Result<()> {
        self.require(n)?;
        self.inner.advance(n);
        Ok(())
    }

    // ---- Indexing ----------------------------------------------------------

    /// Peek at byte at absolute offset from the current cursor, without advancing.
    #[inline]
    pub fn peek_at(&self, offset: usize) -> io::Result<u8> {
        if offset < self.inner.remaining() {
            Ok(self.inner[offset])
        } else {
            Err(eof(offset + 1, self.inner.remaining()))
        }
    }

    /// Take the first `n` bytes as a `Bytes` (zero-copy split), advancing.
    #[inline]
    pub fn take(&mut self, n: usize) -> io::Result<Bytes> {
        self.require(n)?;
        Ok(self.inner.split_to(n))
    }
}

impl From<Bytes> for SafeBuf {
    #[inline]
    fn from(buf: Bytes) -> Self {
        SafeBuf::new(buf)
    }
}

impl From<SafeBuf> for Bytes {
    #[inline]
    fn from(buf: SafeBuf) -> Self {
        buf.inner
    }
}

impl AsRef<[u8]> for SafeBuf {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

/// Construct an `UnexpectedEof` error with diagnostics.
#[inline]
fn eof(needed: usize, remaining: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::UnexpectedEof,
        format!(
            "buffer underflow: needed {} bytes, {} remaining",
            needed, remaining
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_u8_returns_eof_on_empty() {
        let mut buf = SafeBuf::new(Bytes::new());
        assert!(matches!(buf.read_u8(), Err(e) if e.kind() == io::ErrorKind::UnexpectedEof));
    }

    #[test]
    fn read_u32_le_returns_eof_on_short_buffer() {
        // This is exactly the crash scenario: len=10 but we try to read a field
        // that needs more. Here 2 bytes remaining, asking for 4.
        let mut buf = SafeBuf::new(Bytes::from_static(&[0x01, 0x02]));
        let err = buf.read_u32_le().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
        assert!(err.to_string().contains("needed 4"));
        assert!(err.to_string().contains("2 remaining"));
    }

    #[test]
    fn copy_to_slice_returns_eof_instead_of_panic() {
        // len=2, advance=10 — the exact crash class
        let mut buf = SafeBuf::new(Bytes::from_static(&[0xAB, 0xCD]));
        let mut dst = [0u8; 10];
        let err = buf.read_bytes_into(&mut dst).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn read_vec_returns_eof_on_short_buffer() {
        let mut buf = SafeBuf::new(Bytes::from_static(&[0x01, 0x02, 0x03]));
        let err = buf.read_vec(20).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn successful_reads_advance_cursor() {
        let mut buf = SafeBuf::new(Bytes::from_static(&[0x01, 0x02, 0x03, 0x04]));
        assert_eq!(buf.read_u8().unwrap(), 0x01);
        assert_eq!(buf.read_u16_le().unwrap(), 0x0302);
        assert_eq!(buf.remaining(), 1);
        assert_eq!(buf.read_u8().unwrap(), 0x04);
        assert_eq!(buf.remaining(), 0);
    }

    #[test]
    fn read_array_compiles_and_works() {
        let mut buf = SafeBuf::new(Bytes::from_static(&[1, 2, 3, 4]));
        let arr: [u8; 4] = buf.read_array().unwrap();
        assert_eq!(arr, [1, 2, 3, 4]);
    }

    #[test]
    fn read_array_returns_eof_on_short() {
        let mut buf = SafeBuf::new(Bytes::from_static(&[1, 2]));
        let result: io::Result<[u8; 4]> = buf.read_array();
        assert!(result.is_err());
    }
}

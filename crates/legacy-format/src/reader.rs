//! Bounded little-endian byte reader shared by native song and import parsers.

/// Why a bounded read at the current offset failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    Overflow,
    Truncated,
}

/// Cursored slice reader for little-endian binary records.
#[derive(Debug, Clone, Copy)]
pub struct ByteReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteReader<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub const fn at(bytes: &'a [u8], offset: usize) -> Self {
        Self { bytes, offset }
    }

    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    pub fn take(&mut self, length: usize) -> Result<&'a [u8], ReadError> {
        let end = self.offset.checked_add(length).ok_or(ReadError::Overflow)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(ReadError::Truncated)?;
        self.offset = end;
        Ok(slice)
    }

    pub fn u8(&mut self) -> Result<u8, ReadError> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, ReadError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub fn i16(&mut self) -> Result<i16, ReadError> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32, ReadError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn i32(&mut self) -> Result<i32, ReadError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_primitives_and_rejects_truncation_and_overflow() {
        let data = [
            0x7f, // u8: 127
            0x34, 0x12, // u16: 0x1234
            0xfe, 0xff, // i16: -2
            0x78, 0x56, 0x34, 0x12, // u32: 0x12345678
            0xf0, 0xff, 0xff, 0xff, // i32: -16
        ];
        let mut reader = ByteReader::new(&data);
        assert_eq!(reader.u8(), Ok(127));
        assert_eq!(reader.u16(), Ok(0x1234));
        assert_eq!(reader.i16(), Ok(-2));
        assert_eq!(reader.u32(), Ok(0x1234_5678));
        assert_eq!(reader.i32(), Ok(-16));
        assert_eq!(reader.offset(), data.len());
        assert_eq!(reader.u8(), Err(ReadError::Truncated));

        let mut overflow = ByteReader::at(&data, usize::MAX - 1);
        assert_eq!(overflow.take(2), Err(ReadError::Overflow));
        assert_eq!(overflow.take(4), Err(ReadError::Overflow));
    }
}

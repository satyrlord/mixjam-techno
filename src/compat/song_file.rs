//! Shared file-size policy for original MIX songs, including embedded audio.
use std::{fs::File, io::Read, path::Path};

pub(crate) const MIX_BYTE_LIMIT: usize = 500_000_000;
const SIZE_ERROR: &str = "The MIX file exceeds the 500 MB limit (500,000,000 bytes).";

pub(crate) fn read_song_file(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("Cannot open the MIX file: {error}"))?;
    let size = file
        .metadata()
        .map_err(|error| format!("Cannot read the MIX file size: {error}"))?
        .len();
    read_bounded(file, size, MIX_BYTE_LIMIT)
}

fn read_bounded(reader: impl Read, size: u64, limit: usize) -> Result<Vec<u8>, String> {
    if size > limit as u64 {
        return Err(SIZE_ERROR.into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size as usize)
        .map_err(|error| format!("Cannot allocate memory for the MIX file: {error}"))?;
    // Read at most one byte past the limit, even if the file grows after metadata.
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read the MIX file: {error}"))?;
    if bytes.len() > limit {
        return Err(SIZE_ERROR.into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn song_size_limit_is_500_decimal_megabytes() {
        assert_eq!(MIX_BYTE_LIMIT, 500_000_000);
    }

    #[test]
    fn reader_accepts_the_inclusive_limit_and_rejects_one_byte_more() {
        let limit = 32;
        for length in [limit - 1, limit, limit + 1] {
            let bytes = vec![7; length];
            let result = read_bounded(bytes.as_slice(), length as u64, limit);
            if length <= limit {
                assert_eq!(result.unwrap(), bytes);
            } else {
                assert_eq!(result.unwrap_err(), SIZE_ERROR);
            }
        }
    }

    #[test]
    fn oversized_metadata_rejects_before_reading() {
        let mut reader = Cursor::new([7; 8]);
        assert_eq!(
            read_bounded(&mut reader, MIX_BYTE_LIMIT as u64 + 1, MIX_BYTE_LIMIT).unwrap_err(),
            SIZE_ERROR
        );
        assert_eq!(reader.position(), 0);
    }

    #[test]
    fn growing_file_is_rejected_and_read_stops_after_one_extra_byte() {
        let limit = 32;
        let mut reader = Cursor::new([7; 100]);
        assert_eq!(
            read_bounded(&mut reader, limit as u64 - 1, limit).unwrap_err(),
            SIZE_ERROR
        );
        assert_eq!(reader.position(), limit as u64 + 1);
    }
}

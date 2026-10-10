//! Installed MIX files: two row-major 351 by 8 little-endian u16 grids.
//! Grid 2 holds sixteenth-bar offsets. Trailer bytes stay opaque and lossless.
use std::{
    fmt,
    path::{Component, Path, PathBuf},
};

pub const TRACKS: usize = 8;
pub const ROWS: usize = 351;
pub const CELLS: usize = TRACKS * ROWS;
pub const GRID_BYTES: usize = CELLS * 2;
pub const TRAILER_OFFSET: usize = 2 + GRID_BYTES * 2;
pub const T1_SIGNATURE: u16 = 0x0a07;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixError(pub String);
impl fmt::Display for MixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for MixError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mix {
    pub signature: u16,
    placements: Vec<u16>,
    auxiliary: Vec<u16>,
    pub trailer: Vec<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub row: usize,
    pub track: usize,
    pub sample_id: u16,
    pub auxiliary: u16,
}

impl Default for Mix {
    fn default() -> Self {
        Self {
            signature: T1_SIGNATURE,
            placements: vec![0; CELLS],
            auxiliary: vec![0; CELLS],
            trailer: Vec::new(),
        }
    }
}
impl Mix {
    pub fn parse(bytes: &[u8]) -> Result<Self, MixError> {
        if bytes.len() < TRAILER_OFFSET {
            return Err(MixError(format!(
                "MIX needs at least {TRAILER_OFFSET} bytes; got {}",
                bytes.len()
            )));
        }
        let signature = u16::from_le_bytes([bytes[0], bytes[1]]);
        // Only the signature directly observed in this installation is admitted.
        if signature != T1_SIGNATURE {
            return Err(MixError(format!(
                "Unsupported MIX signature {signature:#06x}"
            )));
        }
        let read = |start| {
            bytes[start..start + GRID_BYTES]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect()
        };
        Ok(Self {
            signature,
            placements: read(2),
            auxiliary: read(2 + GRID_BYTES),
            trailer: bytes[TRAILER_OFFSET..].to_vec(),
        })
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(TRAILER_OFFSET + self.trailer.len());
        out.extend_from_slice(&self.signature.to_le_bytes());
        for v in self.placements.iter().chain(&self.auxiliary) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&self.trailer);
        out
    }
    fn index(row: usize, track: usize) -> Result<usize, MixError> {
        if row >= ROWS || track >= TRACKS {
            return Err(MixError(format!(
                "Cell outside {ROWS} rows and {TRACKS} tracks"
            )));
        }
        Ok(row * TRACKS + track)
    }
    pub fn cell(&self, row: usize, track: usize) -> Result<(u16, u16), MixError> {
        let i = Self::index(row, track)?;
        Ok((self.placements[i], self.auxiliary[i]))
    }
    pub fn set_cell(
        &mut self,
        row: usize,
        track: usize,
        sample: u16,
        auxiliary: u16,
    ) -> Result<(), MixError> {
        let i = Self::index(row, track)?;
        self.placements[i] = sample;
        self.auxiliary[i] = auxiliary;
        Ok(())
    }
    pub fn events(&self) -> impl Iterator<Item = Placement> + '_ {
        self.placements
            .iter()
            .enumerate()
            .filter(|(_, id)| **id != 0)
            .map(|(i, id)| Placement {
                row: i / TRACKS,
                track: i % TRACKS,
                sample_id: *id,
                auxiliary: self.auxiliary[i],
            })
    }
}

/// Raw ordered MAX path inventory. Its slots are not native timeline IDs.
/// Use library::Library for original factory and expansion-kit sample IDs.
#[derive(Debug, Clone)]
pub struct Catalog {
    entries: Vec<String>,
}
impl Catalog {
    pub fn parse(bytes: &[u8]) -> Result<Self, MixError> {
        let text: String = bytes.iter().map(|b| char::from(*b)).collect();
        let mut entries = Vec::new();
        for line in text.lines() {
            let name = line.trim().trim_matches('"').replace('\\', "/");
            if !name.is_empty() {
                let p = Path::new(&name);
                if name.contains(':')
                    || name.starts_with('/')
                    || p.components().any(|c| !matches!(c, Component::Normal(_)))
                {
                    return Err(MixError(
                        "Catalog path must stay inside the sample root".into(),
                    ));
                }
            }
            entries.push(name);
        }
        Ok(Self { entries })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get(&self, id: u16) -> Option<&str> {
        self.entries
            .get(usize::from(id))
            .filter(|s| !s.is_empty())
            .map(String::as_str)
    }
    pub fn resolve(&self, root: &Path, id: u16) -> Option<PathBuf> {
        self.get(id).map(|p| root.join(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn separates_grids_and_retains_unknown_tail() {
        let mut b = vec![0; TRAILER_OFFSET];
        b[..2].copy_from_slice(&T1_SIGNATURE.to_le_bytes());
        b[2..4].copy_from_slice(&512u16.to_le_bytes());
        b[2 + GRID_BYTES..4 + GRID_BYTES].copy_from_slice(&15u16.to_le_bytes());
        b.extend_from_slice(b"opaque\0trailer");
        let m = Mix::parse(&b).unwrap();
        assert_eq!(
            m.events().collect::<Vec<_>>(),
            vec![Placement {
                row: 0,
                track: 0,
                sample_id: 512,
                auxiliary: 15
            }]
        );
        assert_eq!(m.to_bytes(), b);
    }
    #[test]
    fn edit_changes_only_two_named_fields() {
        let mut m = Mix::default();
        let before = m.to_bytes();
        m.set_cell(350, 7, 3145, 14).unwrap();
        assert_eq!(m.cell(350, 7).unwrap(), (3145, 14));
        assert!(m.set_cell(351, 0, 1, 0).is_err());
        let after = m.to_bytes();
        assert_eq!(before.iter().zip(&after).filter(|(a, b)| a != b).count(), 3);
    }
    #[test]
    fn rejects_truncated_and_other_product_files() {
        assert!(Mix::parse(&vec![0; TRAILER_OFFSET - 1]).is_err());
        assert!(Mix::parse(&vec![0; TRAILER_OFFSET]).is_err());
    }
    #[test]
    fn catalog_preserves_empty_slots_and_blocks_escape() {
        let c = Catalog::parse(b"ba\\first.\x70xd\r\n\r\nba\\third.\x70xd\r\n").unwrap();
        assert_eq!(c.len(), 3);
        assert_eq!(c.get(2), Some("ba/third.\x70xd"));
        assert_eq!(c.get(1), None);
        for bad in [b"../secret".as_slice(), b"C:\\secret", b"/secret"] {
            assert!(Catalog::parse(bad).is_err());
        }
    }
}

//! Counted timeline profiles from the installed Gen 2 and Gen 3 products.
//! Unsupported generated records cause an error before a project is returned.
mod assets;
mod layout;
mod project;
#[cfg(test)]
mod tests;
mod waves;
pub(crate) use assets::restore_missing;
pub(super) use layout::native_fader_gain;
pub(crate) use layout::{Event, Header, Reference, Timeline, WaveReference};
pub(crate) use project::build_project;
pub(super) use project::native_source;
pub(crate) use waves::embedded_wave_ranges;
#[cfg(test)]
#[path = "chunks/import_tests.rs"]
mod import_tests;

use super::song_file::read_song_file;
use crate::{engine::Project, library::Library, products::Product};
use layout::{read_header, read_timeline};
use std::path::Path;

pub fn load(path: &Path, product: Product, library: &Library) -> Result<Project, String> {
    let bytes = read_song_file(path)?;
    let header = read_header(&bytes, product)?;
    let raw = read_timeline(&bytes, header.after_mixer, product)?;
    build_project(path, &bytes, product, library, header, raw)
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or("The native song record is truncated.")?;
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn block(&mut self) -> Result<&'a [u8], String> {
        let length = self.u16()? as usize;
        self.take(length)
    }
}

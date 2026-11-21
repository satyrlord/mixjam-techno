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
pub(crate) use layout::{
    Event, Header, Reference, Timeline, WaveReference, checked_clip_gain, clip_gain,
    native_position, native_tick_count,
};
pub(crate) use project::build_project;
pub(super) use project::native_source;
pub(crate) use waves::embedded_wave_records;
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

struct Reader<'a>(legacy_format::ByteReader<'a>);

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self(legacy_format::ByteReader::new(bytes))
    }
    const fn at(bytes: &'a [u8], offset: usize) -> Self {
        Self(legacy_format::ByteReader::at(bytes, offset))
    }
    const fn bytes(&self) -> &'a [u8] {
        self.0.bytes()
    }
    const fn offset(&self) -> usize {
        self.0.offset()
    }
    fn set_offset(&mut self, offset: usize) {
        self.0.set_offset(offset);
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        self.0
            .take(length)
            .map_err(|_| "The native song record is truncated.".into())
    }
    fn u8(&mut self) -> Result<u8, String> {
        self.0
            .u8()
            .map_err(|_| "The native song record is truncated.".into())
    }
    fn u16(&mut self) -> Result<u16, String> {
        self.0
            .u16()
            .map_err(|_| "The native song record is truncated.".into())
    }
    fn u32(&mut self) -> Result<u32, String> {
        self.0
            .u32()
            .map_err(|_| "The native song record is truncated.".into())
    }
    fn i16(&mut self) -> Result<i16, String> {
        self.0
            .i16()
            .map_err(|_| "The native song record is truncated.".into())
    }
    fn i32(&mut self) -> Result<i32, String> {
        self.0
            .i32()
            .map_err(|_| "The native song record is truncated.".into())
    }
    fn block(&mut self) -> Result<&'a [u8], String> {
        let length = self.u16()? as usize;
        self.take(length)
    }
}

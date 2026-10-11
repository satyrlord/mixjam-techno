//! Native screenshot writer. It has no access to projects, sources, or devices.
use eframe::egui;
use std::{io::Write, path::Path};

/// Write a screenshot for an agent. Agents read PNG, not BMP.
pub(super) fn write_png(path: &Path, image: &egui::ColorImage) -> Result<(), String> {
    image::save_buffer(
        path,
        image.as_raw(),
        image.width() as u32,
        image.height() as u32,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|error| format!("Cannot save the screenshot: {error}"))
}

pub(super) fn write_bmp(path: &Path, image: &egui::ColorImage) -> std::io::Result<()> {
    let width = image.width() as u32;
    let height = image.height() as u32;
    let bytes = width * height * 4;
    let mut file = std::fs::File::create(path)?;
    file.write_all(b"BM")?;
    file.write_all(&(54 + bytes).to_le_bytes())?;
    file.write_all(&[0; 4])?;
    file.write_all(&54_u32.to_le_bytes())?;
    file.write_all(&40_u32.to_le_bytes())?;
    file.write_all(&width.to_le_bytes())?;
    file.write_all(&(-(height as i32)).to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&32_u16.to_le_bytes())?;
    file.write_all(&0_u32.to_le_bytes())?;
    file.write_all(&bytes.to_le_bytes())?;
    file.write_all(&[0; 16])?;
    for color in &image.pixels {
        file.write_all(&[color.b(), color.g(), color.r(), 255])?;
    }
    Ok(())
}

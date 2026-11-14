mod asset_publication;
mod event_records;
mod mixer_headers;
mod wave_records;

use super::*;
use super::{
    assets::{
        MAX_PENDING_ASSET_BYTES, NEXT_ASSET_TEMP_ID, preserve_wave, publish_asset_directly,
        publish_staged_asset, queue_asset, stage_asset,
    },
    layout::{
        Event, Reference, SENTINEL, Timeline, WaveReference, decode_pan, native_fader_gain,
        native_position, native_tick_count, profile, read_chunk, read_header, read_timeline,
    },
    project::build_project,
    waves::{
        Wave, embedded_wave_records, embedded_waves, normalize_saved_path, read_external_wave,
        read_wave_extension, source_wave, wave_info,
    },
};
use crate::products::IMPORTED_ID_START;
use crate::{
    compat::{drums, settings},
    engine::{Clip, ImportedSample, MAX_TICKS, TICKS_PER_BAR, TRACK_COUNT, Track},
};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fs,
    io::{Cursor, Read},
    path::PathBuf,
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};

fn text_block(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
    bytes.extend_from_slice(value);
}

fn header_fixture(product: Product, bpm: u16, effective: u32) -> Vec<u8> {
    titled_header_fixture(product, bpm, effective, b"Fixture title\0Mixer text")
}

fn titled_header_fixture(product: Product, bpm: u16, effective: u32, title: &[u8]) -> Vec<u8> {
    let (app_id, count, width) = profile(product).unwrap();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&app_id.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&bpm.to_le_bytes());
    bytes.extend_from_slice(&effective.to_le_bytes());
    text_block(&mut bytes, b"Composer\0#SKKENNUNG#:0000000\0\x01");
    text_block(&mut bytes, title);
    let mixer_count =
        count + usize::from(matches!(product, Product::D3 | Product::D4 | Product::T3));
    for _ in 0..mixer_count {
        bytes.extend_from_slice(&(if width == 4 { 0u16 } else { 128u16 }).to_le_bytes());
        bytes.extend_from_slice(&16384u16.to_le_bytes());
        if width == 8 {
            bytes.extend_from_slice(&0u32.to_le_bytes());
        }
    }
    bytes
}

fn lane(bytes: &mut Vec<u8>, number: u8, events: &[(u8, i32, u16)]) {
    bytes.push(number);
    bytes.extend_from_slice(&(events.len() as u32).to_le_bytes());
    for (class, position, sample) in events {
        bytes.push(*class);
        bytes.extend_from_slice(&position.to_le_bytes());
        bytes.extend_from_slice(&sample.to_le_bytes());
        if *position >= 0 {
            bytes.extend_from_slice(&[0; 10]);
        }
    }
}

fn wave_fixture() -> Vec<u8> {
    wave_fixture_frames(4)
}

fn wave_fixture_frames(frames: usize) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for value in [0i16, 1000, -1000, 0].into_iter().cycle().take(frames) {
            writer.write_sample(value).unwrap();
        }
        writer.finalize().unwrap();
    }
    cursor.into_inner()
}

fn asset_test_directory(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "mixjam-techno-{label}-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_ASSET_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).unwrap();
    directory
}

//! Names that a sample folder states: pack tempos, the tempo a folder's files
//! agree on, the folder that holds a native pack, and the compact names that
//! identities match.

use crate::products::Product;
use std::path::{Path, PathBuf};

/// The tempo in a loop pack's BPM.INF: the `BPM` key of its `[Tempo]` section.
/// Some commercial loop collections put one such file in each pack folder.
pub(super) fn stated_pack_tempo(text: &str) -> Option<f64> {
    let mut section = "";
    text.lines().map(str::trim).find_map(|line| {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim();
            return None;
        }
        let (key, value) = line.split_once('=')?;
        let tempo: f64 = (section.eq_ignore_ascii_case("tempo")
            && key.trim().eq_ignore_ascii_case("bpm"))
        .then(|| value.trim().parse().ok())??;
        (40.0..=300.0).contains(&tempo).then_some(tempo)
    })
}

/// Whether two tempos are the same up to whole octaves, such as 80 and 160.
pub(super) fn octave_apart(a: f64, b: f64) -> bool {
    let octaves = (a / b).log2();
    (octaves - octaves.round()).abs() < 1e-6
}

/// The tempo that a loop pack states in a file name, such as 180 in
/// `HARDCORE_BASS001_180_C_SL3.WAV`: a two- or three-digit BPM token followed by
/// a key token, which is a note name with an optional sharp, flat, or minor
/// mark, or X for a sound without a key.
pub(super) fn named_tempo(path: &Path) -> Option<f64> {
    let stem = path.file_stem()?.to_str()?;
    let tokens: Vec<&str> = stem.split('_').collect();
    tokens.windows(2).find_map(|pair| {
        let (bpm, key) = (pair[0], pair[1].to_ascii_lowercase());
        let note = key
            .strip_prefix(|c: char| ('a'..='g').contains(&c))
            .is_some_and(|rest| ["", "#", "b", "m", "#m", "bm"].contains(&rest));
        let tempo: f64 =
            (bpm.len() >= 2 && bpm.len() <= 3 && bpm.bytes().all(|b| b.is_ascii_digit()))
                .then(|| bpm.parse().ok())??;
        ((note || key == "x") && (40.0..=300.0).contains(&tempo)).then_some(tempo)
    })
}

/// The number of whole bars, up to 32, a duration spans at a tempo, within
/// four frames at 44.1 kHz plus 0.05 percent (at most 5 ms) for trimmed
/// loops. Longer files are recordings, not loops, and fit no tempo.
pub(super) fn whole_bars(seconds: f64, tempo: f64) -> Option<u32> {
    let bars = seconds * tempo / 240.0;
    let nearest = bars.round();
    let slack = 4.0 / f64::from(legacy_format::NATIVE_SAMPLE_RATE) + (seconds / 2000.0).min(0.005);
    ((1.0..=32.0).contains(&nearest) && (bars - nearest).abs() * 240.0 / tempo <= slack)
        .then_some(nearest as u32)
}

/// Whether at least three files and three quarters of a folder are whole bars
/// at this tempo.
pub(super) fn agrees_on_tempo(durations: &[(u64, u32)], tempo: f64) -> bool {
    let whole = durations
        .iter()
        .filter(|(frames, rate)| {
            *rate > 0 && whole_bars(*frames as f64 / f64::from(*rate), tempo).is_some()
        })
        .count();
    whole >= 3 && whole * 4 >= durations.len() * 3
}

/// The tempo at which a folder's WAV durations are whole bars, when at least
/// three quarters of them (and three files) agree on one. Candidates are whole
/// BPM from 60 to 200 and the fallback.
///
/// Durations alone fit several tempos: two-bar loops at 140 are one bar at 70
/// and three at 210. Tempos within five percent of the best fit stay in the
/// running, so that one odd file does not decide. Among those, tempos that give
/// 1, 2, 4, 8, or 16 bars are preferred; then the fallback itself; then the
/// octave from 88 up to 176 BPM, which holds exactly one of a tempo, its half,
/// and its double; then the tempo nearest the fallback by ratio.
pub(crate) fn infer_folder_tempo(durations: &[(u64, u32)], fallback: f64) -> Option<f64> {
    if !fallback.is_finite() || fallback <= 0.0 {
        return None;
    }
    let seconds: Vec<f64> = durations
        .iter()
        .filter(|(_, rate)| *rate > 0)
        .map(|(frames, rate)| *frames as f64 / f64::from(*rate))
        .collect();
    if seconds.len() < 3 {
        return None;
    }
    // Whole-bar files, and those among them with a power-of-two bar count.
    let fits: Vec<(f64, usize, usize)> = (60..=200)
        .map(f64::from)
        .chain(std::iter::once(fallback))
        .map(|tempo| {
            let bars = seconds.iter().filter_map(|s| whole_bars(*s, tempo));
            let (mut whole, mut even) = (0, 0);
            for count in bars {
                whole += 1;
                even += usize::from(count <= 16 && count.is_power_of_two());
            }
            (tempo, whole, even)
        })
        .collect();
    let near = |count: usize, best: usize| count * 20 >= best * 19;
    let most_whole = fits.iter().map(|fit| fit.1).max()?;
    if most_whole < 3 || most_whole * 4 < seconds.len() * 3 {
        return None;
    }
    let agreeing = fits.iter().filter(|fit| near(fit.1, most_whole));
    let most_even = agreeing.clone().map(|fit| fit.2).max()?;
    let rank = |tempo: f64| {
        (
            tempo != fallback,
            !(88.0..176.0).contains(&tempo),
            (tempo / fallback).ln().abs(),
        )
    };
    agreeing
        .filter(|fit| near(fit.2, most_even))
        .map(|fit| fit.0)
        .min_by(|a, b| {
            rank(*a)
                .partial_cmp(&rank(*b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

pub(super) fn canonical_group(group: &str) -> String {
    match group.to_ascii_lowercase().as_str() {
        "sequence" => "seq".into(),
        "sphere" => "layer".into(),
        "voice" => "rap".into(),
        "extra" => "xtra".into(),
        "effects" => "effect".into(),
        "guitars" => "guitar".into(),
        other => other.into(),
    }
}

/// The folder of an original installation that holds its packed sample
/// archives and their indexes.
pub(crate) fn packed_folder(root: &Path) -> PathBuf {
    root.join("\x65Jay").join("\x50XD")
}

/// Whether the nearest product-named folder, from the file up to the
/// selected folder, names another product.
pub(super) fn in_foreign_product_folder(path: &Path, root: &Path, product: Product) -> bool {
    path.ancestors()
        .skip(1)
        .take_while(|folder| folder.starts_with(root))
        .filter_map(Path::file_name)
        .find_map(|name| product_folder_hint(&name.to_string_lossy()))
        .is_some_and(|hint| hint != product)
}

pub(super) fn product_folder_hint(name: &str) -> Option<Product> {
    match compact_name(name).as_str() {
        "\x64ance1" | "\x64ance\x65jay1" => Some(Product::D1),
        "\x64ance2" | "\x64ance\x65jay2" => Some(Product::D2),
        "\x64ance3" | "\x64ance\x65jay3" => Some(Product::D3),
        "\x64ance4" | "\x64ance\x65jay4" => Some(Product::D4),
        "\x68ouse\x65jay" => Some(Product::H1),
        "\x72ave\x65jay" | "\x74echno1" | "\x74echno\x65jay1" => Some(Product::T1),
        "\x74echno2" | "\x74echno\x65jay" | "\x74echno\x65jay2" => Some(Product::T2),
        "\x74echno3" | "\x74echno\x65jay3" => Some(Product::T3),
        _ => None,
    }
}

pub(super) fn slug(name: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('-');
            }
            result.push(c.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    result
}

pub(crate) fn compact_name(name: &str) -> String {
    let mut result = String::new();
    let mut digits = String::new();
    fn finish(result: &mut String, digits: &mut String) {
        if !digits.is_empty() {
            let trimmed = digits.trim_start_matches('0');
            result.push_str(if trimmed.is_empty() { "0" } else { trimmed });
            digits.clear();
        }
    }
    for c in name.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            finish(&mut result, &mut digits);
            if c.is_ascii_alphabetic() {
                result.push(c.to_ascii_lowercase());
            }
        }
    }
    finish(&mut result, &mut digits);
    result
}

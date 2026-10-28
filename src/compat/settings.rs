//! Settings list that D3, D4, T3, and H1 save after the song
//! title. Each entry is `KEY#°_#VALUE%°_%`, where `°` is byte 0xB0. The reader
//! keeps every entry as text. Import applies the Booster entries to the master
//! pedalboard; the others are kept only.
use crate::{
    library::decode_1252,
    master::{EQ_BAND_MAX, MasterChain},
    products::Product,
};
use std::collections::BTreeMap;

const VALUE_MARK: &[u8] = b"#\xb0_#";
const ENTRY_END: &[u8] = b"%\xb0_%";

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Saved {
    pub(crate) entries: BTreeMap<String, String>,
    /// Entries without a key or value mark, repeated keys, and text after the
    /// last entry.
    pub(crate) unread: usize,
}

fn find(bytes: &[u8], mark: &[u8]) -> Option<usize> {
    bytes.windows(mark.len()).position(|window| window == mark)
}

/// Read the list from a title block: the title, a NUL, the list, a NUL, 0x01.
pub(super) fn parse(title_block: &[u8]) -> Saved {
    let mut saved = Saved::default();
    let mut rest = title_block
        .split(|byte| *byte == 0)
        .nth(1)
        .unwrap_or_default();
    while let Some(end) = find(rest, ENTRY_END) {
        let entry = &rest[..end];
        rest = &rest[end + ENTRY_END.len()..];
        let Some(mark) = find(entry, VALUE_MARK).filter(|mark| *mark > 0) else {
            saved.unread += 1;
            continue;
        };
        let key = decode_1252(&entry[..mark]);
        if saved.entries.contains_key(&key) {
            saved.unread += 1;
            continue;
        }
        saved
            .entries
            .insert(key, decode_1252(&entry[mark + VALUE_MARK.len()..]));
    }
    // Some songs hold plain text here and no list. That text is not an entry.
    if (!saved.entries.is_empty() || saved.unread > 0) && !rest.is_empty() {
        saved.unread += 1;
    }
    saved
}

/// The original's saved master-effect settings as the Booster pedals. A switch saved as
/// `1` turns its pedal on; a pedal keeps its default for a missing value.
/// The editor scales a saved knob from 0 to 32767 to the library's 0 to 1000
/// and divides an equaliser band by 100. A switched-on pedal with a value
/// that cannot be read stays off, and the second value names it.
pub(super) fn booster(product: Product, saved: &Saved) -> (MasterChain, Vec<&'static str>) {
    let mut chain = MasterChain::default();
    let mut unusable = Vec::new();
    if !matches!(product, Product::D3 | Product::D4 | Product::T3) {
        return (chain, unusable);
    }
    let on = |key: &str| saved.entries.get(key).is_some_and(|value| value == "1");
    // `None` for a value that is present but cannot be used.
    let number = |key: &str, max: u32, default: u32| match saved.entries.get(key) {
        None => Some(default),
        Some(text) => text
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|value| *value <= max),
    };
    // The editor's `r(K * 1000 / 32767)`. No saved value falls halfway, and
    // a missing value keeps the pedal's default.
    let knob = |key: &str, default: u16| match saved.entries.get(key) {
        None => Some(default),
        Some(_) => number(key, 32_767, 0)
            .map(|value| (f64::from(value) * 1000.0 / 32_767.0).round() as u16),
    };
    let bands: Option<Vec<u8>> = (0..10)
        .map(|band| {
            number(
                &format!("BoostEQ_{band}"),
                u32::from(EQ_BAND_MAX),
                u32::from(EQ_BAND_MAX / 2),
            )
            .map(|value| value as u8)
        })
        .collect();
    match bands {
        Some(bands) => {
            chain.booster_eq.bands.copy_from_slice(&bands);
            chain.booster_eq.on = on("BoostEQ");
        }
        None if on("BoostEQ") => unusable.push("EQ"),
        None => {}
    }
    let compressor = &mut chain.booster_compressor;
    match (
        knob("BoostCompressorDrive", compressor.drive),
        knob("BoostCompressorSpeed", compressor.speed),
        knob("BoostCompressorGain", compressor.gain),
    ) {
        (Some(drive), Some(speed), Some(gain)) => {
            (compressor.drive, compressor.speed, compressor.gain) = (drive, speed, gain);
            compressor.on = on("BoostCompressorLED");
        }
        _ if on("BoostCompressorLED") => unusable.push("compressor"),
        _ => {}
    }
    match knob("BoostStereoWide", chain.booster_wide.spread) {
        Some(spread) => {
            chain.booster_wide.spread = spread;
            chain.booster_wide.on = on("BoostStereoLED");
        }
        None if on("BoostStereoLED") => unusable.push("stereo wide"),
        None => {}
    }
    (chain, unusable)
}

/// Notices for saved settings that change the original's sound and that the
/// renderer does not apply.
pub(super) fn notices(product: Product, saved: &Saved, has_drum_patterns: bool) -> Vec<String> {
    let on = |key: &str| saved.entries.get(key).is_some_and(|value| value == "1");
    let named = |switches: &[(&str, &'static str)]| -> Vec<&'static str> {
        switches
            .iter()
            .filter(|(key, _)| on(key))
            .map(|(_, name)| *name)
            .collect()
    };
    let mut notices = Vec::new();
    if saved.unread > 0 {
        notices.push(format!(
            "{} saved settings entries could not be read and were not kept.",
            saved.unread
        ));
    }
    match product {
        Product::D3 | Product::D4 | Product::T3 => {
            let (_, unusable) = booster(product, saved);
            if !unusable.is_empty() {
                notices.push(format!(
                    "Saved Booster effects are not applied because their values cannot be read: \
                     {}. The mix can sound different from the original. The values remain in \
                     Saved settings.",
                    unusable.join(", ")
                ));
            }
            let drum = named(&[
                ("DrumEQ", "EQ"),
                ("DrumOver", "overdrive"),
                ("DrumEcho", "echo"),
                ("DrumChorus", "chorus"),
                ("DrumMidsweep", "midsweep"),
                ("DrumHall", "reverb"),
            ]);
            let pads: Vec<String> = (0..16)
                .filter(|pad| on(&format!("FX_{pad}")))
                .map(|pad| (pad + 1).to_string())
                .collect();
            if has_drum_patterns && !drum.is_empty() && !pads.is_empty() {
                notices.push(format!(
                    "Saved drum pad effects are not applied: {} on pad {}. Drum patterns play \
                     without them. The values remain in Saved settings.",
                    drum.join(", "),
                    pads.join(", ")
                ));
            }
        }
        Product::H1 if !saved.entries.is_empty() => notices.push(format!(
            "House saved {} settings with this song. They remain in Saved settings; the \
             application does not read them.",
            saved.entries.len()
        )),
        _ => {}
    }
    notices
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(title: &str, entries: &[(&str, &str)]) -> Vec<u8> {
        let mut bytes = title.as_bytes().to_vec();
        bytes.push(0);
        for (key, value) in entries {
            bytes.extend_from_slice(key.as_bytes());
            bytes.extend_from_slice(VALUE_MARK);
            bytes.extend_from_slice(value.as_bytes());
            bytes.extend_from_slice(ENTRY_END);
        }
        bytes.extend_from_slice(&[0, 1]);
        bytes
    }

    #[test]
    fn list_keeps_every_entry_and_counts_what_it_cannot_read() {
        let saved = parse(&block(
            "Title",
            &[
                ("BoostEQ", "1"),
                ("BoostEQ_0", "50"),
                ("Empty", ""),
                ("MixPan3", "729,166666666667"),
            ],
        ));
        assert_eq!(saved.unread, 0);
        assert_eq!(saved.entries.len(), 4);
        assert_eq!(saved.entries["BoostEQ_0"], "50");
        assert_eq!(saved.entries["Empty"], "");
        assert_eq!(saved.entries["MixPan3"], "729,166666666667");

        // Plain text after the title, and a title alone, are not lists.
        for bytes in [
            &b"-\0All the way up\0\x01"[..],
            b"Title",
            b"Title\0\x01",
            b"",
        ] {
            assert_eq!(parse(bytes), Saved::default());
        }

        let mut damaged = b"Title\0A#\xb0_#1%\xb0_%".to_vec();
        damaged.extend_from_slice(b"no mark%\xb0_%#\xb0_#no key%\xb0_%A#\xb0_#2%\xb0_%tail\0\x01");
        let saved = parse(&damaged);
        assert_eq!(saved.entries, BTreeMap::from([("A".into(), "1".into())]));
        assert_eq!(saved.unread, 4);
        assert!(notices(Product::T3, &saved, false)[0].starts_with("4 saved settings entries"));
    }

    #[test]
    fn booster_settings_become_master_pedals() {
        // A D4 factory song saves these values.
        let mut entries = vec![
            ("BoostCompressorLED", "1"),
            ("BoostCompressorSpeed", "32767"),
            ("BoostCompressorDrive", "32767"),
            ("BoostCompressorGain", "9101"),
            ("BoostStereoLED", "1"),
            ("BoostEQ", "1"),
            ("BoostStereoWide", "14563"),
            ("BOOU1_0", "78"),
        ];
        let bands: Vec<_> = (0..10)
            .map(|band| (format!("BoostEQ_{band}"), (50 + band * 5).to_string()))
            .collect();
        entries.extend(
            bands
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        let saved = parse(&block("Title", &entries));
        let (chain, unusable) = booster(Product::D4, &saved);
        assert!(unusable.is_empty());
        assert!(chain.booster_eq.on && chain.booster_compressor.on && chain.booster_wide.on);
        assert_eq!(
            chain.booster_eq.bands,
            [50, 55, 60, 65, 70, 75, 80, 85, 90, 95]
        );
        let compressor = &chain.booster_compressor;
        assert_eq!(
            (compressor.drive, compressor.speed, compressor.gain),
            (1000, 1000, 278)
        );
        assert_eq!(chain.booster_wide.spread, 444);
        assert!(notices(Product::D4, &saved, false).is_empty());
        assert_eq!(booster(Product::D2, &saved).0, MasterChain::default());
        assert_eq!(booster(Product::H1, &saved).0, MasterChain::default());

        // Switches that are off, or absent, keep the values with the pedal off.
        // The factory value 16384 is the middle of the knob.
        let quiet = parse(&block(
            "Title",
            &[
                ("BoostCompressorLED", "0"),
                ("BoostCompressorDrive", "16384"),
                ("BoostStereoWide", "21348"),
            ],
        ));
        let (chain, _) = booster(Product::T3, &quiet);
        assert!(!chain.is_active());
        assert_eq!(chain.booster_compressor.drive, 500);
        assert_eq!(chain.booster_wide.spread, 652);
        assert_eq!(
            booster(Product::T3, &Saved::default()).0,
            MasterChain::default()
        );
    }

    #[test]
    fn notices_name_switched_on_effects_that_are_not_applied() {
        let quiet = parse(&block(
            "Title",
            &[("BoostEQ", "0"), ("BoostCompressorLED", "0"), ("FX_3", "1")],
        ));
        assert!(notices(Product::D4, &quiet, true).is_empty());

        let saved = parse(&block(
            "Title",
            &[
                ("BoostCompressorLED", "1"),
                ("BoostCompressorGain", "32768"),
                ("BoostStereoLED", "1"),
                ("BoostStereoWide", "wide"),
                ("BoostEQ", "0"),
                ("BoostEQ_4", "101"),
                ("DrumEcho", "1"),
                ("DrumHall", "1"),
                ("FX_0", "1"),
                ("FX_12", "1"),
            ],
        ));
        let (chain, unusable) = booster(Product::D4, &saved);
        assert!(!chain.is_active());
        assert_eq!(unusable, ["compressor", "stereo wide"]);
        let without_drums = notices(Product::D4, &saved, false);
        assert_eq!(without_drums.len(), 1);
        assert!(without_drums[0].contains("cannot be read: compressor, stereo wide."));
        let with_drums = notices(Product::D3, &saved, true);
        assert_eq!(with_drums.len(), 2);
        assert!(with_drums[1].contains("not applied: echo, reverb on pad 1, 13."));

        let house = notices(Product::H1, &saved, true);
        assert_eq!(house.len(), 1);
        assert!(house[0].starts_with("House saved 10 settings"));
        assert!(notices(Product::H1, &Saved::default(), true).is_empty());
        assert!(notices(Product::D2, &saved, true).is_empty());
    }
}

//! Settings list that Dance 3, Dance 4, Techno 3, and House save after the song
//! title. Each entry is `KEY#°_#VALUE%°_%`, where `°` is byte 0xB0. The reader
//! keeps every entry as text; it does not apply any of them.
use crate::{library::decode_1252, products::Product};
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
            let booster = named(&[
                ("BoostEQ", "EQ"),
                ("BoostCompressorLED", "compressor"),
                ("BoostStereoLED", "stereo wide"),
            ]);
            if !booster.is_empty() {
                notices.push(format!(
                    "Saved Booster effects are not applied: {}. The mix can sound different from the original. The values remain in Saved settings.",
                    booster.join(", ")
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
                    "Saved drum pad effects are not applied: {} on pad {}. Drum patterns play without them. The values remain in Saved settings.",
                    drum.join(", "),
                    pads.join(", ")
                ));
            }
        }
        Product::H1 if !saved.entries.is_empty() => notices.push(format!(
            "House saved {} settings with this song. They remain in Saved settings; the application does not read them.",
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
                ("BoostStereoLED", "1"),
                ("DrumEcho", "1"),
                ("DrumHall", "1"),
                ("FX_0", "1"),
                ("FX_12", "1"),
            ],
        ));
        let without_drums = notices(Product::D4, &saved, false);
        assert_eq!(without_drums.len(), 1);
        assert!(without_drums[0].contains("not applied: compressor, stereo wide."));
        let with_drums = notices(Product::D3, &saved, true);
        assert_eq!(with_drums.len(), 2);
        assert!(with_drums[1].contains("not applied: echo, reverb on pad 1, 13."));

        let house = notices(Product::H1, &saved, true);
        assert_eq!(house.len(), 1);
        assert!(house[0].starts_with("House saved 6 settings"));
        assert!(notices(Product::H1, &Saved::default(), true).is_empty());
        assert!(notices(Product::D2, &saved, true).is_empty());
    }
}

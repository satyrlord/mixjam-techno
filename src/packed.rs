//! The packed sample decoder recovered from packed32r4.dll.
//!
//! Static authority: decoder RVA 0x75ac and table builder RVA 0x3095, which
//! builds `ORIGINAL_DPCM_STEPS`.
//! Original predictors wrap at 16 bits. Dictionary token zero initially emits
//! five unchanged predictor values. Other dictionary tokens must be defined.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSample {
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}

const MAX_FRAMES: usize = 64_000_000;

/// Decode one complete packed archive record to 44.1 kHz mono PCM.
pub fn decode_record(record: &[u8]) -> Result<DecodedSample, String> {
    if !record.starts_with(b"t\x50xD") || record.len() < 12 {
        return Err("The sample does not contain a complete packed header.".into());
    }
    let data_offset = usize::from(record[4]) + 12;
    if data_offset > record.len() {
        return Err("The packed metadata exceeds the sample record.".into());
    }
    let frames =
        u32::from_le_bytes(record[data_offset - 6..data_offset - 2].try_into().unwrap()) as usize;
    if frames > MAX_FRAMES {
        return Err("The packed sample exceeds the supported frame limit.".into());
    }
    // FUN_1000af92 skips the final header word. The channel state, rather
    // than that word, supplies the original decoder's starting predictor.
    let mut predictor = 0_i16;
    let table = &legacy_format::packed::ORIGINAL_DPCM_STEPS;
    let mut dictionary: [Option<Vec<i16>>; 256] = std::array::from_fn(|_| None);
    dictionary[0] = Some(vec![0; 5]);
    let mut samples = Vec::with_capacity(frames);
    let mut cursor = data_offset;
    while samples.len() < frames && cursor < record.len() {
        let token = record[cursor];
        cursor += 1;
        if token == 0xff {
            let code = *record
                .get(cursor)
                .ok_or("The packed delta code is truncated.")?;
            cursor += 1;
            predictor = predictor.wrapping_add(table[usize::from(code)]);
            samples.push(predictor);
            continue;
        }
        let slot = if token >= 0xf4 {
            let count = usize::from(token - 0xf3);
            if count > 7 {
                return Err("The packed dictionary sequence exceeds one dictionary slot.".into());
            }
            let slot = usize::from(
                *record
                    .get(cursor)
                    .ok_or("The packed dictionary index is truncated.")?,
            );
            cursor += 1;
            if slot >= 0xf4 {
                return Err("The packed dictionary index is outside the token range.".into());
            }
            let codes = record
                .get(cursor..cursor + count)
                .ok_or("The packed dictionary definition is truncated.")?;
            cursor += count;
            dictionary[slot] = Some(codes.iter().map(|&code| table[usize::from(code)]).collect());
            slot
        } else {
            usize::from(token)
        };
        let deltas = dictionary[slot]
            .as_ref()
            .ok_or("The packed stream uses an undefined dictionary entry.")?;
        // The original emits the complete final dictionary sequence into its
        // padded mix buffer. The record duration bounds the audible PCM.
        for &delta in deltas.iter().take(frames - samples.len()) {
            predictor = predictor.wrapping_add(delta);
            samples.push(predictor);
        }
    }
    // RWavToTemp clears its destination before decoding. Many StepSynth records
    // encode one fewer frame than their duration; the final frame is silence.
    samples.resize(frames, 0);
    Ok(DecodedSample {
        sample_rate: 44_100,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(frames: u32, initial: i16, stream: &[u8]) -> Vec<u8> {
        let mut bytes = b"t\x50xD\0\0".to_vec();
        bytes.extend(frames.to_le_bytes());
        bytes.extend(initial.to_le_bytes());
        bytes.extend(stream);
        bytes
    }

    #[test]
    fn dictionary_definition_reference_and_literal_delta() {
        let decoded =
            decode_record(&record(6, 0, &[0xff, 90, 0xf5, 1, 204, 52, 1, 0xff, 128])).unwrap();
        assert_eq!(decoded.samples, [-430, 2882, -430, 2882, -430, -430]);
    }

    #[test]
    fn initial_dictionary_ignores_unused_header_word() {
        assert_eq!(
            decode_record(&record(5, 123, &[0])).unwrap().samples,
            [0; 5]
        );
        assert_eq!(decode_record(&record(1, 0, &[])).unwrap().samples, [0]);
    }

    #[test]
    fn predictor_wraps_without_saturation() {
        assert_eq!(
            decode_record(&record(2, 0, &[0xff, 255, 0xff, 255]))
                .unwrap()
                .samples,
            [25266, -15004]
        );
    }

    #[test]
    fn malformed_records_fail_without_partial_audio() {
        for bytes in [
            vec![],
            b"t\x50xD".to_vec(),
            record(1, 0, &[0xff]),
            record(1, 0, &[1]),
            record(1, 0, &[0xf5, 1, 128]),
        ] {
            assert!(decode_record(&bytes).is_err());
        }
    }
}

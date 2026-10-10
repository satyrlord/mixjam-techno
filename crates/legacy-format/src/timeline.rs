//! Recovered beginning-of-song APlay scheduling in byte units.
//! Original main: PlayStart 0x439bb0; timing constants at 0x492477/0x49249c.
//! A bar is 235200 stereo PCM bytes. Grid 2 is a sixteenth-bar start offset.
use crate::editing::EDITABLE_ROWS;
use crate::mix::{Mix, MixError, TRACKS};

pub const BYTES_PER_FRAME: u32 = 4;
pub const BYTES_PER_BAR: u32 = 235_200;
pub const FRAMES_PER_BAR: u32 = BYTES_PER_BAR / BYTES_PER_FRAME;
pub const FRAMES_PER_SIXTEENTH: u32 = FRAMES_PER_BAR / 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledSample {
    pub row: usize,
    pub track: usize,
    pub sample_id: u16,
    pub sixteenth_offset: u16,
    pub start_frame: u64,
    /// Original APlay duration limit. The source can finish before this limit.
    pub limit_frames: u64,
}

/// `bars` supplies the original catalog's integer sample length. This function
/// does not infer lengths from an unknown sample, seek, or loop state.
pub fn schedule(
    mix: &Mix,
    mut bars: impl FnMut(u16) -> Option<u32>,
) -> Result<Vec<ScheduledSample>, MixError> {
    let mut out = Vec::new();
    for track in 0..TRACKS {
        // The native WORD comparison is signed: IDs with bit 15 set are
        // skipped. The file parser still preserves those raw values.
        let events: Vec<_> = mix
            .events()
            .filter(|e| e.track == track && e.row < EDITABLE_ROWS && e.sample_id <= i16::MAX as u16)
            .collect();
        for (index, e) in events.iter().enumerate() {
            if e.auxiliary >= 16 {
                return Err(MixError(format!(
                    "Sixteenth offset {} is outside 0..15 at row {} track {}",
                    e.auxiliary, e.row, track
                )));
            }
            let nominal = bars(e.sample_id).ok_or_else(|| {
                MixError(format!("Missing catalog length for sample {}", e.sample_id))
            })?;
            if nominal == 0 || nominal > 32767 || (nominal == 32767 && e.auxiliary > 0) {
                return Err(MixError(
                    "Catalog length is outside the supported signed 16-bit scheduling range".into(),
                ));
            }
            // The original adds a row for a delayed sample before it searches
            // for the next occupied row and caps the row gap to this extent.
            let extent = (u64::from(nominal) + u64::from(e.auxiliary > 0))
                .min((EDITABLE_ROWS - e.row) as u64);
            let next = events.get(index + 1);
            if next.is_some_and(|n| n.auxiliary >= 16) {
                return Err(MixError(
                    "Next sample has an invalid sixteenth offset".into(),
                ));
            }
            let gap = next.map_or(extent, |n| (n.row - e.row) as u64).min(extent);
            let end_offset = next
                .filter(|n| (n.row - e.row) as u64 == gap)
                .map_or(0, |n| u64::from(n.auxiliary));
            let start_offset = u64::from(e.auxiliary) * u64::from(FRAMES_PER_SIXTEENTH);
            let limit_frames = gap * u64::from(FRAMES_PER_BAR)
                + end_offset * u64::from(FRAMES_PER_SIXTEENTH)
                - start_offset;
            out.push(ScheduledSample {
                row: e.row,
                track,
                sample_id: e.sample_id,
                sixteenth_offset: e.auxiliary,
                start_frame: e.row as u64 * u64::from(FRAMES_PER_BAR) + start_offset,
                limit_frames,
            });
        }
    }
    out.sort_by_key(|e| (e.start_frame, e.track));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_next_placement_limits_previous_voice_at_exact_frame() {
        let mut m = Mix::default();
        m.set_cell(0, 2, 1, 4).unwrap();
        m.set_cell(1, 2, 2, 12).unwrap();
        let p = schedule(&m, |_| Some(2)).unwrap();
        assert_eq!(p[0].start_frame, 14700);
        assert_eq!(p[1].start_frame, 102900);
        assert_eq!(p[0].limit_frames, 88200);
        assert_eq!(p[1].limit_frames, 132300);
    }
    #[test]
    fn offset_extends_original_duration_envelope_without_changing_sample() {
        let mut m = Mix::default();
        m.set_cell(3, 0, 77, 15).unwrap();
        let p = schedule(&m, |_| Some(1)).unwrap();
        assert_eq!(p[0].start_frame, 231525);
        assert_eq!(p[0].limit_frames, 62475);
    }
    #[test]
    fn rejects_missing_lengths_and_invalid_offsets() {
        let mut m = Mix::default();
        m.set_cell(0, 0, 1, 0).unwrap();
        assert!(schedule(&m, |_| None).is_err());
        m.set_cell(0, 0, 1, 16).unwrap();
        assert!(schedule(&m, |_| Some(1)).is_err());
    }
    #[test]
    fn native_signed_sample_ids_are_preserved_but_not_scheduled() {
        let mut m = Mix::default();
        m.set_cell(0, 0, 65535, 0).unwrap();
        assert_eq!(m.events().count(), 1);
        assert!(schedule(&m, |_| Some(1)).unwrap().is_empty());
        m.set_cell(0, 0, 1, 0).unwrap();
        assert!(schedule(&m, |_| Some(32768)).is_err());
    }
    #[test]
    fn native_guard_row_is_not_played_and_region_end_clips_last_voice() {
        let mut m = Mix::default();
        m.set_cell(350, 0, 731, 0).unwrap();
        assert!(schedule(&m, |_| Some(20)).unwrap().is_empty());
        m.set_cell(349, 0, 732, 15).unwrap();
        let events = schedule(&m, |_| Some(20)).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].limit_frames, 3675);
        assert_eq!(
            events[0].start_frame + events[0].limit_frames,
            350 * u64::from(FRAMES_PER_BAR)
        );
    }
}

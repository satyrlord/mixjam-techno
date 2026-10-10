//! Original selected-cell copy and paste transformations.
//! Copy 0x435670, Paste 0x434f20, VCopy 0x4352e0, VPaste 0x4349b0.
//! The original editing bound at form+0x466 is 350 (VA 0x47079f).
//! The serialized 351st row is preserved as a guard row and is not a paste target.
use crate::mix::{Mix, MixError, ROWS, TRACKS};

pub const EDITABLE_ROWS: usize = 350;
#[derive(Debug, Clone, PartialEq, Eq)]
struct CopiedCell {
    row: usize,
    track: usize,
    sample: u16,
    step: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Clipboard {
    cells: Vec<CopiedCell>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PasteResult {
    pub written: usize,
    pub clipped: usize,
}

impl Clipboard {
    /// The first selected row is the origin. Track indices remain absolute,
    /// as in the original column-preserving copy buffers.
    pub fn copy(mix: &Mix, selection: &[(usize, usize)]) -> Result<Self, MixError> {
        let mut origin = ROWS;
        for &(row, track) in selection {
            mix.cell(row, track)?;
            origin = origin.min(row);
        }
        let mut cells = Vec::new();
        for &(row, track) in selection {
            let (sample, step) = mix.cell(row, track)?;
            if step >= 16 {
                return Err(MixError(
                    "Selected cell has an invalid sixteenth offset".into(),
                ));
            }
            if sample > 0
                && sample <= i16::MAX as u16
                && !cells
                    .iter()
                    .any(|c: &CopiedCell| c.row == row - origin && c.track == track)
            {
                cells.push(CopiedCell {
                    row: row - origin,
                    track,
                    sample,
                    step: step as u8,
                });
            }
        }
        cells.sort_by_key(|c| (c.row, c.track));
        Ok(Self { cells })
    }
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    pub fn len(&self) -> usize {
        self.cells.len()
    }
    /// Ordinary Paste preserves copied sixteenth offsets and clips cells
    /// outside tracks 0..7 or editable rows 0..349. Occupied targets overwrite.
    pub fn paste(
        &self,
        mix: &mut Mix,
        row: i32,
        track_delta: i32,
    ) -> Result<PasteResult, MixError> {
        self.paste_shifted(mix, row, track_delta, 0)
    }
    /// VPaste shifts the selected sixteenth position. One carry/borrow matches
    /// the original; valid UI endpoints supply a delta within -15..15.
    pub fn paste_shifted(
        &self,
        mix: &mut Mix,
        row: i32,
        track_delta: i32,
        step_delta: i16,
    ) -> Result<PasteResult, MixError> {
        if !(-15..=15).contains(&step_delta) {
            return Err(MixError("Paste step delta must be within -15..15".into()));
        }
        let mut result = PasteResult::default();
        for c in &self.cells {
            let target_row = i64::from(row) + c.row as i64;
            let target_track = i64::from(track_delta) + c.track as i64;
            // VPaste applies this guard before the substep carry/borrow.
            if target_row >= EDITABLE_ROWS as i64 || !(0..TRACKS as i64).contains(&target_track) {
                result.clipped += 1;
                continue;
            }
            let step = i16::from(c.step) + step_delta;
            let (target_row, step) = if step < 0 {
                (target_row - 1, step + 16)
            } else if step >= 16 {
                (target_row + 1, step - 16)
            } else {
                (target_row, step)
            };
            // The original can carry into the preserved guard row. Negative
            // targets are clipped here instead of emulating an invalid access.
            if !(0..ROWS as i64).contains(&target_row) {
                result.clipped += 1;
                continue;
            }
            mix.set_cell(
                target_row as usize,
                target_track as usize,
                c.sample,
                step as u16,
            )?;
            result.written += 1;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copy_keeps_track_columns_and_paste_overwrites_only_selected_positive_cells() {
        let mut m = Mix::default();
        m.set_cell(5, 2, 731, 15).unwrap();
        m.set_cell(7, 6, 1075, 2).unwrap();
        let c = Clipboard::copy(&m, &[(5, 2), (7, 6), (5, 2)]).unwrap();
        assert_eq!(c.len(), 2);
        let mut target = Mix::default();
        target.set_cell(10, 3, 99, 1).unwrap();
        let r = c.paste(&mut target, 10, 1).unwrap();
        assert_eq!(
            r,
            PasteResult {
                written: 2,
                clipped: 0
            }
        );
        assert_eq!(target.cell(10, 3).unwrap(), (731, 15));
        assert_eq!(target.cell(12, 7).unwrap(), (1075, 2));
    }
    #[test]
    fn sixteenth_shift_carries_borrows_and_retains_native_guard_row() {
        let mut m = Mix::default();
        m.set_cell(0, 0, 731, 15).unwrap();
        let c = Clipboard::copy(&m, &[(0, 0)]).unwrap();
        c.paste_shifted(&mut m, 349, 0, 1).unwrap();
        assert_eq!(m.cell(350, 0).unwrap(), (731, 0));
        assert_eq!(c.paste(&mut m, 350, 0).unwrap().clipped, 1);
        m.set_cell(0, 0, 732, 0).unwrap();
        let c = Clipboard::copy(&m, &[(0, 0)]).unwrap();
        c.paste_shifted(&mut m, 5, 0, -1).unwrap();
        assert_eq!(m.cell(4, 0).unwrap(), (732, 15));
        assert_eq!(c.paste_shifted(&mut m, 0, 0, -1).unwrap().clipped, 1);
    }
    #[test]
    fn invalid_selection_fails_without_changing_source() {
        let m = Mix::default();
        assert!(Clipboard::copy(&m, &[(ROWS, 0)]).is_err());
        assert!(Clipboard::copy(&m, &[]).unwrap().is_empty());
    }
}

//! The block schedule that export, stem export, and playback share: the
//! song's frames, then the FX buses' tail, then the processing delay.
use super::Mix;
use crate::fx::TAIL_LIMIT;

/// What a block of the schedule holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    /// Frames of the song.
    Song,
    /// Silence through the buses, which still ring. `after` counts the tail
    /// frames before the block: the song's frames go on through the tail.
    Tail { after: u64 },
    /// Silence that drains the processing delay.
    Flush,
}

/// The blocks of a render after its song frames. The buses' tail plays in
/// steps of `Mix::TAIL_BLOCK` frames while the buses ring, up to
/// `TAIL_LIMIT` frames. Whether they ring is asked once a step, so a render
/// in any block size ends at the same frame. Then the processing delay
/// plays out. No step allocates: playback runs it on the audio thread.
#[derive(Clone, Copy, Debug)]
pub(super) struct Schedule {
    /// The most frames in a block.
    block: u64,
    /// The tail frames played, and of them those left of the current step.
    rung: u64,
    step: u64,
    /// Whether the buses have stopped ringing or reached `TAIL_LIMIT`.
    rung_out: bool,
    /// The frames of processing delay still to play out.
    flush: u64,
}

impl Schedule {
    /// A schedule of blocks of up to `block` frames, which plays out
    /// `latency` frames of delay at the end.
    pub(super) fn new(block: usize, latency: usize) -> Self {
        Self {
            block: block as u64,
            rung: 0,
            step: 0,
            rung_out: false,
            flush: latency as u64,
        }
    }

    /// The next block and its frames, with `song` frames of the song left
    /// before it, or `None` at the end. `ringing` says whether the buses
    /// still ring. It is asked only where a tail step can start, and not
    /// again once it says no.
    pub(super) fn next(
        &mut self,
        song: u64,
        ringing: impl FnOnce() -> bool,
    ) -> Option<(Phase, usize)> {
        if song > 0 {
            return Some((Phase::Song, song.min(self.block) as usize));
        }
        if self.step == 0 && !self.rung_out {
            if self.rung < TAIL_LIMIT && ringing() {
                self.step = Mix::TAIL_BLOCK.min(TAIL_LIMIT - self.rung);
            } else {
                self.rung_out = true;
            }
        }
        if self.step > 0 {
            let count = self.step.min(self.block);
            let after = self.rung;
            self.rung += count;
            self.step -= count;
            return Some((Phase::Tail { after }, count as usize));
        }
        if self.flush > 0 {
            let count = self.flush.min(self.block);
            self.flush -= count;
            return Some((Phase::Flush, count as usize));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every block of a render of `song` frames in blocks of `block`
    /// frames, whose buses ring for the first `rings` questions, and the
    /// count of questions.
    fn blocks(
        song: u64,
        block: usize,
        latency: usize,
        rings: usize,
    ) -> (Vec<(Phase, usize)>, usize) {
        let mut schedule = Schedule::new(block, latency);
        let (mut played, mut asked, mut blocks) = (0, 0, Vec::new());
        while let Some((phase, count)) = schedule.next(song - played, || {
            asked += 1;
            asked <= rings
        }) {
            if phase == Phase::Song {
                played += count as u64;
            }
            blocks.push((phase, count));
        }
        (blocks, asked)
    }

    /// The frames of each run of one phase in `blocks`, in order.
    fn runs(blocks: &[(Phase, usize)]) -> Vec<(Phase, usize)> {
        let mut runs: Vec<(Phase, usize)> = Vec::new();
        for &(phase, count) in blocks {
            match (runs.last_mut(), phase) {
                (Some((Phase::Song, frames)), Phase::Song)
                | (Some((Phase::Tail { .. }, frames)), Phase::Tail { .. })
                | (Some((Phase::Flush, frames)), Phase::Flush) => *frames += count,
                _ => runs.push((phase, count)),
            }
        }
        runs
    }

    #[test]
    fn the_song_plays_then_the_tail_in_steps_then_the_delay() {
        let step = Mix::TAIL_BLOCK as usize;
        assert_eq!(
            blocks(3000, 65_536, 300, 2),
            (
                vec![
                    (Phase::Song, 3000),
                    (Phase::Tail { after: 0 }, step),
                    (Phase::Tail { after: step as u64 }, step),
                    (Phase::Flush, 300),
                ],
                3
            )
        );
        // Without a tail or a delay, the render ends with the song.
        assert_eq!(
            blocks(3000, 1024, 0, 0).0,
            [(Phase::Song, 1024), (Phase::Song, 1024), (Phase::Song, 952)]
        );
        assert_eq!(blocks(0, 256, 0, 0), (Vec::new(), 1));
    }

    #[test]
    fn short_blocks_ask_once_a_step_and_end_at_the_same_frame() {
        let (whole, asked) = blocks(5000, 65_536, 700, 3);
        assert_eq!(asked, 4);
        for block in [256, 100, 1] {
            let (short, asked) = blocks(5000, block, 700, 3);
            assert_eq!(asked, 4, "{block}-frame blocks");
            assert!(short.iter().all(|&(_, count)| count <= block));
            assert_eq!(runs(&short), runs(&whole), "{block}-frame blocks");
            // A tail block's start counts the tail before it.
            let mut rung = 0;
            for (phase, count) in short {
                if let Phase::Tail { after } = phase {
                    assert_eq!(after, rung);
                    rung += count as u64;
                }
            }
            assert_eq!(rung, 3 * Mix::TAIL_BLOCK);
        }
    }

    #[test]
    fn the_tail_stops_at_its_limit() {
        let mut schedule = Schedule::new(65_536, 0);
        let mut rung = 0;
        while let Some((phase, count)) = schedule.next(0, || true) {
            assert!(matches!(phase, Phase::Tail { .. }));
            rung += count as u64;
        }
        assert_eq!(rung, TAIL_LIMIT);
    }
}

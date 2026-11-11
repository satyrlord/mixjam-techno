//! Waveform peaks of library sounds: the lowest and highest sample of each
//! bucket of frames, at several resolutions. Views draw waveforms from these
//! tables and never decode audio. A worker thread decodes each sound once,
//! when a view first asks for it; the tables are kept until the catalog
//! changes.
use crate::{audio::Pcm, library::Library};
use std::{
    collections::HashMap,
    sync::{Arc, mpsc},
};

/// Frames per bucket of the finest table. Each coarser table has four times
/// as many frames per bucket.
pub const FINEST_BUCKET: u32 = 64;
/// How much coarser each table is than the one before it.
const LEVEL_FACTOR: u32 = 4;

/// One resolution: the lowest and highest sample of each bucket, scaled to
/// -127..=127 of full scale.
#[derive(Clone, Debug, PartialEq)]
pub struct PeakLevel {
    pub frames_per_bucket: u32,
    pub buckets: Vec<[i8; 2]>,
}

/// The peaks of one sound, all channels together.
#[derive(Clone, Debug, PartialEq)]
pub struct Peaks {
    pub frames: u64,
    /// From the finest to the coarsest. The coarsest holds at most a few
    /// buckets; a sound shorter than one bucket has one table of one bucket.
    pub levels: Vec<PeakLevel>,
}

fn scale(value: f32) -> i8 {
    (value.clamp(-1.0, 1.0) * 127.0).round() as i8
}

impl Peaks {
    pub fn from_pcm(pcm: &Pcm) -> Self {
        let channels = usize::from(pcm.channels.max(1));
        let frames = pcm.samples.len() / channels;
        let bucket = FINEST_BUCKET as usize;
        let finest = PeakLevel {
            frames_per_bucket: FINEST_BUCKET,
            buckets: pcm.samples[..frames * channels]
                .chunks(bucket * channels)
                .map(|chunk| {
                    let (low, high) = chunk
                        .iter()
                        .fold((f32::MAX, f32::MIN), |(low, high), &sample| {
                            (low.min(sample), high.max(sample))
                        });
                    [scale(low), scale(high)]
                })
                .collect(),
        };
        let mut levels = vec![finest];
        while let Some(last) = levels.last()
            && last.buckets.len() > LEVEL_FACTOR as usize
        {
            let coarser = PeakLevel {
                frames_per_bucket: last.frames_per_bucket * LEVEL_FACTOR,
                buckets: last
                    .buckets
                    .chunks(LEVEL_FACTOR as usize)
                    .map(|chunk| {
                        chunk.iter().fold([i8::MAX, i8::MIN], |[low, high], pair| {
                            [low.min(pair[0]), high.max(pair[1])]
                        })
                    })
                    .collect(),
            };
            levels.push(coarser);
        }
        Self {
            frames: frames as u64,
            levels,
        }
    }

    /// The coarsest table whose buckets are no longer than `frames_per_point`.
    fn level_for(&self, frames_per_point: f64) -> &PeakLevel {
        self.levels
            .iter()
            .rev()
            .find(|level| f64::from(level.frames_per_bucket) <= frames_per_point)
            .unwrap_or(&self.levels[0])
    }

    /// The lowest and highest sample, from -1 to 1, between two frames,
    /// read from the table that suits `frames_per_point`. A range outside
    /// the sound reads as silence.
    pub fn range(&self, start: f64, end: f64, frames_per_point: f64) -> (f32, f32) {
        if end <= start || end <= 0.0 || start >= self.frames as f64 {
            return (0.0, 0.0);
        }
        let level = self.level_for(frames_per_point);
        let size = f64::from(level.frames_per_bucket);
        let count = level.buckets.len();
        let first = (start.max(0.0) / size).floor() as usize;
        let last = ((end.max(start) / size).ceil() as usize).max(first + 1);
        let [low, high] = level.buckets[first.min(count)..last.min(count)]
            .iter()
            .fold([i8::MAX, i8::MIN], |[low, high], pair| {
                [low.min(pair[0]), high.max(pair[1])]
            });
        if low > high {
            return (0.0, 0.0);
        }
        (f32::from(low) / 127.0, f32::from(high) / 127.0)
    }

    /// The sound's highest absolute sample, from 0 to 1.
    pub fn loudest(&self) -> f32 {
        self.levels.last().map_or(0.0, |level| {
            level
                .buckets
                .iter()
                .map(|[low, high]| low.unsigned_abs().max(high.unsigned_abs()))
                .max()
                .map_or(0.0, |peak| f32::from(peak) / 127.0)
        })
    }

    /// The whole sound in `count` columns of equal length, as a thumbnail.
    pub fn columns(&self, count: usize) -> Vec<(f32, f32)> {
        let step = self.frames as f64 / count.max(1) as f64;
        (0..count)
            .map(|column| {
                let start = column as f64 * step;
                self.range(start, start + step, step)
            })
            .collect()
    }
}

/// What a view gets when it asks for a sound's peaks.
#[derive(Clone, Debug)]
pub enum PeakState {
    /// The worker has not finished this sound yet.
    Pending,
    Ready(Arc<Peaks>),
    /// The sound cannot be read, for example because its file is gone.
    Unavailable,
}

struct Request {
    revision: u64,
    sample: u32,
    library: Arc<Library>,
}

struct Reply {
    revision: u64,
    sample: u32,
    peaks: Option<Arc<Peaks>>,
}

struct Worker {
    sender: mpsc::Sender<Request>,
    receiver: mpsc::Receiver<Reply>,
}

impl Worker {
    fn spawn() -> Option<Self> {
        let (sender, requests) = mpsc::channel::<Request>();
        let (replies, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("mixjam-techno-peaks".into())
            .spawn(move || {
                let mut queue: Vec<Request> = Vec::new();
                loop {
                    if queue.is_empty() {
                        match requests.recv() {
                            Ok(request) => queue.push(request),
                            Err(_) => return,
                        }
                    }
                    queue.extend(requests.try_iter());
                    // The newest catalog only, and the newest request first:
                    // the rows on screen now come before the rows scrolled past.
                    let newest = queue.iter().map(|request| request.revision).max();
                    queue.retain(|request| Some(request.revision) == newest);
                    let Some(request) = queue.pop() else {
                        continue;
                    };
                    let peaks = request
                        .library
                        .read_pcm(request.sample)
                        .ok()
                        .map(|pcm| Arc::new(Peaks::from_pcm(&pcm)));
                    let reply = Reply {
                        revision: request.revision,
                        sample: request.sample,
                        peaks,
                    };
                    if replies.send(reply).is_err() {
                        return;
                    }
                }
            })
            .ok()?;
        Some(Self { sender, receiver })
    }
}

/// The peak tables of one catalog, filled by a worker thread on demand.
#[derive(Default)]
pub(crate) struct PeakStore {
    revision: u64,
    entries: HashMap<u32, PeakState>,
    worker: Option<Worker>,
}

impl PeakStore {
    /// The peaks of `sample` in the catalog of `revision`. The first request
    /// queues the sound for the worker and reads as pending.
    pub(crate) fn get(&mut self, revision: u64, sample: u32, library: &Arc<Library>) -> PeakState {
        if revision != self.revision {
            self.revision = revision;
            self.entries.clear();
        }
        if let Some(state) = self.entries.get(&sample) {
            return state.clone();
        }
        if self.worker.is_none() {
            self.worker = Worker::spawn();
        }
        let sent = self.worker.as_ref().is_some_and(|worker| {
            worker
                .sender
                .send(Request {
                    revision,
                    sample,
                    library: Arc::clone(library),
                })
                .is_ok()
        });
        let state = if sent {
            PeakState::Pending
        } else {
            // A worker that cannot start or has stopped is replaced on the
            // next request; meanwhile the sound has no waveform.
            self.worker = None;
            PeakState::Unavailable
        };
        self.entries.insert(sample, state.clone());
        state
    }

    #[cfg(all(test, feature = "gui"))]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Give `sample` finished tables without the worker, for view tests.
    #[cfg(all(test, feature = "gui"))]
    pub(crate) fn insert(&mut self, revision: u64, sample: u32, peaks: Arc<Peaks>) {
        if revision != self.revision {
            self.revision = revision;
            self.entries.clear();
        }
        self.entries.insert(sample, PeakState::Ready(peaks));
    }

    /// Take the worker's finished tables. Returns whether any sound is still
    /// pending, so the view keeps refreshing until all have arrived.
    pub(crate) fn poll(&mut self) -> bool {
        if let Some(worker) = &self.worker {
            for reply in worker.receiver.try_iter() {
                // A finished table stays; only a pending sound takes a reply.
                if reply.revision == self.revision
                    && matches!(self.entries.get(&reply.sample), Some(PeakState::Pending))
                {
                    self.entries.insert(
                        reply.sample,
                        reply.peaks.map_or(PeakState::Unavailable, PeakState::Ready),
                    );
                }
            }
        }
        self.entries
            .values()
            .any(|state| matches!(state, PeakState::Pending))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm(channels: u16, samples: Vec<f32>) -> Pcm {
        Pcm {
            sample_rate: 44_100,
            channels,
            samples,
        }
    }

    #[test]
    fn each_bucket_holds_the_lowest_and_highest_sample_of_every_channel() {
        // Two channels; the right channel holds the extremes of the second bucket.
        let mut samples = vec![0.0; 2 * 2 * FINEST_BUCKET as usize];
        samples[0] = 0.5;
        samples[3] = -0.25;
        let second = 2 * FINEST_BUCKET as usize;
        samples[second + 1] = 1.0;
        samples[second + 5] = -1.0;
        let peaks = Peaks::from_pcm(&pcm(2, samples));
        assert_eq!(peaks.frames, 2 * u64::from(FINEST_BUCKET));
        assert_eq!(peaks.levels.len(), 1);
        assert_eq!(peaks.levels[0].buckets, vec![[-32, 64], [-127, 127]]);
    }

    #[test]
    fn coarser_tables_merge_four_buckets_until_a_few_remain() {
        let frames = 1000 * FINEST_BUCKET as usize;
        let samples = (0..frames)
            .map(|frame| if frame == frames - 1 { -0.5 } else { 0.25 })
            .collect();
        let peaks = Peaks::from_pcm(&pcm(1, samples));
        let sizes: Vec<(u32, usize)> = peaks
            .levels
            .iter()
            .map(|level| (level.frames_per_bucket, level.buckets.len()))
            .collect();
        assert_eq!(
            sizes,
            vec![(64, 1000), (256, 250), (1024, 63), (4096, 16), (16_384, 4)]
        );
        // The last frame's dip reaches every table's last bucket.
        for level in &peaks.levels {
            assert_eq!(
                level.buckets.last(),
                Some(&[-64, 32]),
                "{}",
                level.frames_per_bucket
            );
        }
    }

    #[test]
    fn a_range_reads_the_table_that_suits_its_zoom_and_silence_outside_the_sound() {
        let frames = 64 * FINEST_BUCKET as usize;
        let samples = (0..frames)
            .map(|frame| {
                if frame == 10 * FINEST_BUCKET as usize {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        let peaks = Peaks::from_pcm(&pcm(1, samples));
        let at = f64::from(10 * FINEST_BUCKET);
        assert_eq!(peaks.range(at, at + 1.0, 1.0), (0.0, 1.0));
        assert_eq!(peaks.range(0.0, at, 64.0), (0.0, 0.0));
        // A coarse view still sees the peak inside its wider bucket.
        assert_eq!(peaks.range(0.0, 1.0, 1024.0), (0.0, 1.0));
        assert_eq!(
            peaks.range(frames as f64, frames as f64 + 64.0, 64.0),
            (0.0, 0.0)
        );
        let columns = peaks.columns(8);
        assert_eq!(columns.len(), 8);
        assert_eq!(columns[1], (0.0, 1.0));
        assert!(
            columns
                .iter()
                .enumerate()
                .all(|(i, c)| i == 1 || *c == (0.0, 0.0))
        );
    }

    #[test]
    fn an_empty_or_tiny_sound_has_one_small_table() {
        let empty = Peaks::from_pcm(&pcm(1, Vec::new()));
        assert_eq!(empty.frames, 0);
        assert_eq!(empty.columns(4), vec![(0.0, 0.0); 4]);
        let tiny = Peaks::from_pcm(&pcm(1, vec![0.1, -0.2, 0.3]));
        assert_eq!(tiny.levels.len(), 1);
        assert_eq!(tiny.levels[0].buckets, vec![[-25, 38]]);
    }

    #[test]
    fn disjoint_empty_and_reversed_ranges_read_as_silence() {
        // The final bucket is partial; frames after the sound must not read it.
        let peaks = Peaks::from_pcm(&pcm(1, vec![0.25; 3]));
        for (start, end) in [
            (-64.0, -1.0),
            (-1.0, 0.0),
            (3.0, 4.0),
            (4.0, 64.0),
            (1.0, 1.0),
            (2.0, 1.0),
        ] {
            assert_eq!(peaks.range(start, end, 64.0), (0.0, 0.0), "{start}..{end}");
        }
        let value = 32.0 / 127.0;
        // A range that intersects the sound still reads its peaks.
        assert_eq!(peaks.range(-1.0, 1.0, 64.0), (value, value));
        assert_eq!(peaks.range(2.0, 4.0, 64.0), (value, value));
    }

    #[test]
    fn the_store_reads_pending_until_the_worker_answers_and_forgets_an_old_catalog() {
        let library = Arc::new(Library::fixture(Vec::new()));
        let mut store = PeakStore::default();
        assert!(matches!(store.get(1, 7, &library), PeakState::Pending));
        // The fixture has no audio for sound 7, so the worker reports it unavailable.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while store.poll() {
            assert!(
                std::time::Instant::now() < deadline,
                "The worker must answer."
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(matches!(store.get(1, 7, &library), PeakState::Unavailable));
        // A new catalog asks again.
        assert!(matches!(store.get(2, 7, &library), PeakState::Pending));
    }
}

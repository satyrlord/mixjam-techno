//! Waveform peaks of library sounds: the lowest and highest sample of each
//! bucket of frames, at several resolutions. Views draw waveforms from these
//! tables and never decode audio. A worker thread decodes the sounds a view
//! shows as pending, in the order the view asked for them. A new catalog
//! keeps each table whose sound reads the same audio from an unchanged file.
use crate::{
    audio::Pcm,
    library::{AudioKey, Library},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
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

/// The sounds a view showed as pending in its last frame, in the order it
/// asked for them. Each list replaces the worker's queue.
struct Wanted {
    revision: u64,
    samples: Vec<u32>,
    library: Arc<Library>,
}

struct Reply {
    revision: u64,
    sample: u32,
    /// The audio the table was read from, taken before the read.
    key: Option<AudioKey>,
    peaks: Option<Arc<Peaks>>,
}

struct Worker {
    sender: mpsc::Sender<Wanted>,
    receiver: mpsc::Receiver<Reply>,
}

impl Worker {
    fn spawn() -> Option<Self> {
        let (sender, requests) = mpsc::channel::<Wanted>();
        let (replies, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("mixjam-techno-peaks".into())
            .spawn(move || {
                let mut queue: VecDeque<u32> = VecDeque::new();
                let mut current: Option<(u64, Arc<Library>)> = None;
                loop {
                    let mut next = None;
                    if queue.is_empty() {
                        // An idle worker keeps no catalog alive.
                        current = None;
                        match requests.recv() {
                            Ok(wanted) => next = Some(wanted),
                            Err(_) => return,
                        }
                    }
                    // Only the newest list counts: the rows on screen now,
                    // never the rows scrolled past.
                    if let Some(wanted) = requests.try_iter().last() {
                        next = Some(wanted);
                    }
                    if let Some(wanted) = next {
                        queue = wanted.samples.into();
                        current = Some((wanted.revision, wanted.library));
                    }
                    let (Some(sample), Some((revision, library))) = (queue.pop_front(), &current)
                    else {
                        continue;
                    };
                    let key = library.audio_key(sample);
                    // A decoder that panics marks one sound unreadable and
                    // keeps the worker for the others.
                    let peaks = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        library
                            .read_pcm(sample)
                            .ok()
                            .map(|pcm| Arc::new(Peaks::from_pcm(&pcm)))
                    }))
                    .unwrap_or(None);
                    let reply = Reply {
                        revision: *revision,
                        sample,
                        key,
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
    /// The audio each finished table of this catalog was read from.
    keys: HashMap<u32, AudioKey>,
    /// Finished tables of earlier catalogs by their audio, until a sound
    /// of this catalog with the same audio takes them back.
    earlier: HashMap<AudioKey, PeakState>,
    worker: Option<Worker>,
    /// The pending sounds asked for since the last `poll`, in order.
    asked: Vec<u32>,
    asked_set: HashSet<u32>,
    /// The list the worker holds now.
    sent: Vec<u32>,
    library: Option<Arc<Library>>,
}

impl PeakStore {
    /// The peaks of `sample` in the catalog of `revision`. A sound that is
    /// not ready reads as pending; the next `poll` queues it for the worker
    /// ahead of the sounds asked for after it. In a new catalog, a sound
    /// whose audio and file match a finished table takes it at once.
    pub(crate) fn get(&mut self, revision: u64, sample: u32, library: &Arc<Library>) -> PeakState {
        if revision != self.revision {
            self.new_catalog(revision);
        }
        if !self
            .library
            .as_ref()
            .is_some_and(|known| Arc::ptr_eq(known, library))
        {
            self.library = Some(Arc::clone(library));
        }
        if !self.entries.contains_key(&sample)
            && !self.earlier.is_empty()
            && let Some(key) = library.audio_key(sample)
            && let Some(state) = self.earlier.remove(&key)
        {
            self.keys.insert(sample, key);
            self.entries.insert(sample, state);
        }
        let state = self
            .entries
            .entry(sample)
            .or_insert(PeakState::Pending)
            .clone();
        if matches!(state, PeakState::Pending) && self.asked_set.insert(sample) {
            self.asked.push(sample);
        }
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
            self.new_catalog(revision);
        }
        self.entries.insert(sample, PeakState::Ready(peaks));
    }

    /// Move to the catalog of `revision`. Finished tables wait by their audio
    /// for the sounds of the new catalog; pending sounds are asked for again.
    /// Only the previous catalog's tables wait: tables that no sound took
    /// back by then are dropped, so a session of many catalogs stays bounded.
    fn new_catalog(&mut self, revision: u64) {
        self.revision = revision;
        let mut earlier = HashMap::with_capacity(self.keys.len());
        for (sample, state) in self.entries.drain() {
            if let Some(key) = self.keys.remove(&sample)
                && !matches!(state, PeakState::Pending)
            {
                earlier.insert(key, state);
            }
        }
        self.earlier = earlier;
        self.keys.clear();
        self.asked.clear();
        self.asked_set.clear();
        if !self.sent.is_empty() {
            // Forget the worker's old queue too, even when every visible
            // sound in the new catalog takes back a finished table.
            self.send(Vec::new());
        }
        self.sent.clear();
    }

    /// Take the worker's finished tables and hand it the sounds still
    /// pending on screen. Returns whether any sound is still pending, so the
    /// view keeps refreshing until all have arrived.
    pub(crate) fn poll(&mut self) -> bool {
        let mut stopped = false;
        if let Some(worker) = &self.worker {
            loop {
                match worker.receiver.try_recv() {
                    Ok(reply) => {
                        // A finished table stays; only a pending sound takes a reply.
                        if reply.revision == self.revision
                            && matches!(self.entries.get(&reply.sample), Some(PeakState::Pending))
                        {
                            self.entries.insert(
                                reply.sample,
                                reply.peaks.map_or(PeakState::Unavailable, PeakState::Ready),
                            );
                            if let Some(key) = reply.key {
                                self.keys.insert(reply.sample, key);
                            }
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        stopped = true;
                        break;
                    }
                }
            }
        }
        if stopped {
            // A worker that stopped is replaced, and its list goes to the
            // next one.
            self.worker = None;
            let lost = std::mem::take(&mut self.sent);
            for sample in lost {
                if matches!(self.entries.get(&sample), Some(PeakState::Pending))
                    && self.asked_set.insert(sample)
                {
                    self.asked.push(sample);
                }
            }
        }
        let asked: Vec<u32> = std::mem::take(&mut self.asked)
            .into_iter()
            .filter(|sample| matches!(self.entries.get(sample), Some(PeakState::Pending)))
            .collect();
        self.asked_set.clear();
        if asked != self.sent {
            // Sounds that left the screen before the worker reached them are
            // forgotten, so they are asked for again when they return.
            let keep: HashSet<u32> = asked.iter().copied().collect();
            self.entries.retain(|sample, state| {
                !matches!(state, PeakState::Pending) || keep.contains(sample)
            });
            self.send(asked);
        }
        self.entries
            .values()
            .any(|state| matches!(state, PeakState::Pending))
    }

    fn send(&mut self, samples: Vec<u32>) {
        let Some(library) = self.library.clone() else {
            return;
        };
        if self.worker.is_none() {
            self.worker = Worker::spawn();
        }
        let wanted = Wanted {
            revision: self.revision,
            samples: samples.clone(),
            library,
        };
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.sender.send(wanted).is_ok())
        {
            self.sent = samples;
        } else {
            // No worker can start: these sounds have no waveform.
            self.worker = None;
            self.sent.clear();
            for sample in samples {
                self.entries.insert(sample, PeakState::Unavailable);
            }
        }
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
        settle(&mut store);
        assert!(matches!(store.get(1, 7, &library), PeakState::Unavailable));
        // A new catalog asks again.
        assert!(matches!(store.get(2, 7, &library), PeakState::Pending));
    }

    #[test]
    fn closing_all_peak_views_cancels_the_worker_queue() {
        let library = Arc::new(Library::fixture(Vec::new()));
        let mut store = PeakStore::default();
        let (sender, requests) = mpsc::channel();
        let (_replies, receiver) = mpsc::channel();
        store.worker = Some(Worker { sender, receiver });
        store.get(1, 7, &library);
        store.get(1, 8, &library);
        assert!(store.poll());
        assert_eq!(requests.try_recv().unwrap().samples, vec![7, 8]);

        // No view asks for peaks after both sounds leave the screen. Cancel
        // the worker's remaining queue and stop refreshing for those sounds.
        assert!(!store.poll());
        assert!(requests.try_recv().unwrap().samples.is_empty());
        assert!(!store.entries.contains_key(&7));
        assert!(!store.entries.contains_key(&8));
        assert!(matches!(store.get(1, 7, &library), PeakState::Pending));
    }

    #[test]
    fn a_new_catalog_with_only_cached_peaks_cancels_the_old_worker_queue() {
        let folder =
            std::env::temp_dir().join(format!("peaks-catalog-cancellation-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("loop.wav");
        let audio = pcm(1, vec![0.5; FINEST_BUCKET as usize]);
        audio.write_wav(&path).unwrap();
        let library = Arc::new(Library::fixture(vec![crate::library::Sample {
            id: 1,
            code: "cached".into(),
            name: "Cached sound".into(),
            bars: 1,
            group_name: "Loop".into(),
            offset: 0,
            length: 0,
            frame_count: FINEST_BUCKET,
            sample_rate: 44_100,
            source_bpm: 140.0,
            channels: 1,
            wav_path: Some(path),
            digest: None,
        }]));
        let mut store = PeakStore::default();
        let (sender, requests) = mpsc::channel();
        let (_replies, receiver) = mpsc::channel();
        store.worker = Some(Worker { sender, receiver });
        store.get(1, 1, &library);
        store
            .entries
            .insert(1, PeakState::Ready(Arc::new(Peaks::from_pcm(&audio))));
        store.keys.insert(1, library.audio_key(1).unwrap());
        store.get(1, 7, &library);
        store.get(1, 8, &library);
        assert!(store.poll());
        assert_eq!(requests.try_recv().unwrap().samples, vec![7, 8]);

        // The new catalog's only visible sound recovers its cached table.
        // The old pending sounds still need an empty replacement queue.
        let next = Arc::new((*library).clone());
        assert!(matches!(store.get(2, 1, &next), PeakState::Ready(_)));
        assert!(!store.poll());
        let cancelled = requests.try_recv().unwrap();
        assert_eq!(cancelled.revision, 2);
        assert!(cancelled.samples.is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_new_catalog_replaces_a_worker_queue_with_the_same_sample_ids() {
        let library = Arc::new(Library::fixture(Vec::new()));
        let mut store = PeakStore::default();
        let (sender, requests) = mpsc::channel();
        let (_replies, receiver) = mpsc::channel();
        store.worker = Some(Worker { sender, receiver });
        for revision in [1, 2] {
            store.get(revision, 7, &library);
            store.get(revision, 8, &library);
            assert!(store.poll());
            let wanted = requests.try_iter().last().unwrap();
            assert_eq!(wanted.revision, revision);
            assert_eq!(wanted.samples, vec![7, 8]);
        }
    }

    #[test]
    fn sounds_that_leave_the_screen_never_keep_the_store_pending() {
        let library = Arc::new(Library::fixture(Vec::new()));
        let mut store = PeakStore::default();
        // A long list scrolls past: each frame asks for other sounds.
        for frame in 0..20u32 {
            for sample in frame * 10..frame * 10 + 10 {
                store.get(1, sample, &library);
            }
            store.poll();
        }
        // Only the last screen stays asked for; it must finish, and nothing
        // scrolled past may stay pending.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            for sample in 190..200 {
                store.get(1, sample, &library);
            }
            if !store.poll() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "The sounds on screen must finish."
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        for sample in 190..200 {
            assert!(matches!(
                store.get(1, sample, &library),
                PeakState::Unavailable
            ));
        }
        assert!(
            !store
                .entries
                .values()
                .any(|state| matches!(state, PeakState::Pending))
        );
    }

    #[test]
    fn a_stopped_worker_is_replaced_with_its_list() {
        let library = Arc::new(Library::fixture(Vec::new()));
        let mut store = PeakStore::default();
        store.get(1, 3, &library);
        // A worker whose thread has ended: its reply channel is closed.
        let (sender, _requests) = mpsc::channel();
        let (_, receiver) = mpsc::channel();
        store.worker = Some(Worker { sender, receiver });
        store.sent = vec![3];
        store.asked.clear();
        store.asked_set.clear();
        // Recovery sends the lost list even before the view asks again.
        assert!(store.poll());
        settle(&mut store);
        assert!(matches!(store.get(1, 3, &library), PeakState::Unavailable));
    }

    /// Keep the pending sounds visible until the worker has answered them.
    fn settle(store: &mut PeakStore) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let library = store.library.clone().unwrap();
            let pending: Vec<u32> = store
                .entries
                .iter()
                .filter_map(|(&sample, state)| {
                    matches!(state, PeakState::Pending).then_some(sample)
                })
                .collect();
            for sample in pending {
                store.get(store.revision, sample, &library);
            }
            if !store.poll() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "The worker must answer."
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn a_new_catalog_keeps_the_tables_of_unchanged_files_and_rereads_changed_ones() {
        let folder = std::env::temp_dir().join(format!("peaks-catalog-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("loop.wav");
        pcm(1, vec![0.5; 4 * FINEST_BUCKET as usize])
            .write_wav(&path)
            .unwrap();
        let sound = |id: u32| crate::library::Sample {
            id,
            code: format!("sound {id}"),
            name: format!("Sound {id}"),
            bars: 1,
            group_name: "Loop".into(),
            offset: 0,
            length: 0,
            frame_count: 4 * FINEST_BUCKET,
            sample_rate: 44_100,
            source_bpm: 140.0,
            channels: 1,
            wav_path: Some(path.clone()),
            digest: None,
        };
        let mut store = PeakStore::default();
        let first = Arc::new(Library::fixture(vec![sound(1)]));
        store.get(1, 1, &first);
        settle(&mut store);
        let PeakState::Ready(read) = store.get(1, 1, &first) else {
            panic!("The sound must have peaks.");
        };
        // A folder joins the catalog: the same audio, now under another id,
        // keeps its table without a read.
        let second = Arc::new(Library::fixture(vec![sound(2), sound(5)]));
        let PeakState::Ready(kept) = store.get(2, 5, &second) else {
            panic!("An unchanged file keeps its table.");
        };
        assert!(Arc::ptr_eq(&read, &kept));
        // The file changes: the next catalog reads it again.
        std::thread::sleep(std::time::Duration::from_millis(20));
        pcm(1, vec![-0.25; 8 * FINEST_BUCKET as usize])
            .write_wav(&path)
            .unwrap();
        let third = Arc::new(Library::fixture(vec![sound(5)]));
        assert!(matches!(store.get(3, 5, &third), PeakState::Pending));
        settle(&mut store);
        let PeakState::Ready(reread) = store.get(3, 5, &third) else {
            panic!("The changed file must be read again.");
        };
        assert_eq!(reread.frames, 8 * u64::from(FINEST_BUCKET));
        let _ = std::fs::remove_dir_all(&folder);
    }
}

use super::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Counts the allocations of a thread while it is counting, so a test can
/// show that the callback's side does not allocate. Other threads, and this
/// one when it is not counting, allocate as usual.
struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn note_allocation() {
    let _ = COUNTING.try_with(|counting| {
        if counting.get() {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
    });
}

// SAFETY: every call goes to the system allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        // SAFETY: the caller's contract is passed on.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        // SAFETY: the caller's contract is passed on.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note_allocation();
        // SAFETY: the caller's contract is passed on.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract is passed on.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The allocations that `run` makes on this thread.
fn allocations(run: impl FnOnce()) -> usize {
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    run();
    COUNTING.with(|counting| counting.set(false));
    ALLOCATIONS.with(Cell::get)
}

/// A small fixed-seed generator of uniform noise in -1 to 1.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
}

/// Write `frames` frames of `channels`, each sample its own index, in
/// callbacks of `callback` frames.
fn write_counting(writer: &mut Writer, channels: usize, frames: usize, callback: usize) {
    let mut index = 0_usize;
    while index < frames * channels {
        let end = (index + callback * channels).min(frames * channels);
        for sample in index..end {
            writer.sample(Some(-(sample as f32)), sample as f32);
        }
        writer.finish(0);
        index = end;
    }
}

#[test]
fn the_ring_keeps_the_newest_whole_frames_oldest_first() {
    // Ten frames of capture and five of margin at 10 Hz, in stereo.
    let flight = Flight::with_seconds(2, 10, 1, 1);
    let flight = Arc::new(Flight {
        ring: Ring::new(10, 5, 2),
        ..Arc::into_inner(flight).unwrap()
    });
    let mut writer = flight.writer();
    let (mut device, mut source) = (Vec::new(), Vec::new());
    // Before the ring is full, everything written.
    write_counting(&mut writer, 2, 4, 3);
    flight
        .ring
        .copy(usize::MAX, 2, &mut device, Some(&mut source));
    assert_eq!(device, (0..8).map(|x| x as f32).collect::<Vec<_>>());
    assert_eq!(source, (0..8).map(|x| -(x as f32)).collect::<Vec<_>>());
    // After the ring has wrapped several times, in odd callbacks, the
    // newest ten frames in order: the five frames of margin are not read.
    write_counting(&mut writer, 2, 53, 7);
    flight
        .ring
        .copy(usize::MAX, 2, &mut device, Some(&mut source));
    // The second write counted from 0 again: its last frames are 43 to 52.
    let expected: Vec<f32> = (86..106).map(|x| x as f32).collect();
    assert_eq!(device, expected);
    assert_eq!(source, expected.iter().map(|x| -x).collect::<Vec<_>>());
    // A shorter copy is the newest whole frames.
    flight.ring.copy(5, 2, &mut device, None);
    assert_eq!(device, [102.0, 103.0, 104.0, 105.0]);
}

#[test]
fn the_callback_side_does_not_allocate() {
    let flight = Flight::with_seconds(2, 48_000, 1, 1);
    let mut writer = flight.writer();
    let mut mixer = [0.25_f32, f32::NAN, -0.5, f32::INFINITY]
        .into_iter()
        .cycle();
    let mut output_f32 = vec![0.0_f32; 2 * 480];
    let mut output_i16 = vec![0_i16; 2 * 441];
    // A warm-up, so that lazily made state is not counted.
    super::super::output::fill_into(&mut output_f32, &mut mixer, &mut writer);
    let counted = allocations(|| {
        for _ in 0..200 {
            super::super::output::fill_into(&mut output_f32, &mut mixer, &mut writer);
            writer.record_fill(480, Duration::from_micros(30));
            super::super::output::fill_into(&mut output_i16, &mut mixer, &mut writer);
            writer.record_fill(441, Duration::from_micros(30));
            flight.record_stream_error();
        }
    });
    assert_eq!(counted, 0);
    // The non-finite half of the mixer's samples were counted and written as
    // silence; the ring holds the mixer's samples as they were.
    assert_eq!(
        flight.non_finite.load(Ordering::Relaxed),
        (201 * 480 + 200 * 441) as u64
    );
    let (mut device, mut source) = (Vec::new(), Vec::new());
    flight.ring.copy(8, 2, &mut device, Some(&mut source));
    assert_eq!(device[..4], [0.25, 0.0, -0.5, 0.0]);
    assert!(source[1].is_nan() && source[3].is_infinite());
    // The counting itself works.
    assert!(allocations(|| drop(Vec::<u8>::with_capacity(16))) > 0);
}

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("mixjam-flight-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

#[test]
fn a_capture_writes_both_rings_and_the_state() {
    let flight = Flight::with_seconds(2, 1000, 2, 1);
    let mut writer = flight.writer();
    // Three seconds: the capture holds the last two.
    for frame in 0..3000 {
        let value = frame as f32 / 4000.0;
        writer.sample(Some(f32::NAN), value);
        writer.sample(Some(-value), -value);
        if frame % 100 == 99 {
            writer.finish(if frame == 99 { 1 } else { 0 });
        }
    }
    let root = temp_root("save");
    let session = json!({"song": {"title": "Test"}});
    let saved = save(
        &flight,
        json!({"name": "test device"}),
        "Saved by a test",
        &root,
        Some((Duration::from_millis(250), session.clone())),
    )
    .expect("saves");
    assert_eq!(
        saved.folder.parent(),
        Some(root.join("diagnostics").as_path())
    );
    let mut reader = hound::WavReader::open(saved.folder.join("device.wav")).unwrap();
    assert_eq!(reader.spec().channels, 2);
    assert_eq!(reader.spec().sample_rate, 1000);
    assert_eq!(reader.spec().sample_format, hound::SampleFormat::Float);
    let device: Vec<f32> = reader.samples::<f32>().map(Result::unwrap).collect();
    assert_eq!(device.len(), 4000);
    assert_eq!(device[0], 1000.0 / 4000.0);
    assert_eq!(device[device.len() - 1], -2999.0 / 4000.0);
    let source: Vec<f32> = hound::WavReader::open(saved.folder.join("source.wav"))
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect();
    assert!(source[0].is_nan());
    assert_eq!(source[1], -0.25);
    let state: Value =
        serde_json::from_slice(&std::fs::read(saved.folder.join("state.json")).unwrap()).unwrap();
    assert_eq!(state, saved.state);
    assert_eq!(state["reason"], "Saved by a test");
    assert_eq!(state["session"], session);
    assert_eq!(state["session_age_ms"], 250);
    assert_eq!(state["device"]["name"], "test device");
    assert_eq!(state["callback"]["non_finite_samples_replaced"], 1);
    assert_eq!(state["files"]["frames"], 2000);
    assert_eq!(state["last_half_second"].as_array().unwrap().len(), 2);
    assert!(
        state["not_recorded"]
            .as_array()
            .is_some_and(|left| !left.is_empty())
    );
    // A second capture in the same second gets a folder of its own.
    let again = save(&flight, Value::Null, "Again", &root, None).expect("saves");
    assert_ne!(again.folder, saved.folder);
    assert_eq!(again.state["session"], Value::Null);
    let _ = std::fs::remove_dir_all(&root);
}

const RATE: u32 = 48_000;

fn stereo(mono: &[f32]) -> Vec<f32> {
    mono.iter().flat_map(|x| [*x, *x]).collect()
}

/// Scale `signal` to an RMS level of `dbfs`.
fn at_level(mut signal: Vec<f32>, dbfs: f32) -> Vec<f32> {
    let rms = (signal.iter().map(|x| x * x).sum::<f32>() / signal.len() as f32).sqrt();
    let gain = 10_f32.powf(dbfs / 20.0) / rms;
    signal.iter_mut().for_each(|x| *x *= gain);
    signal
}

/// The readings of each half-second window, a quarter second apart, as the
/// watcher reads them.
fn windows(signal: &[f32]) -> Vec<Reading> {
    let mut detector = Detector::new(RATE, 2);
    let window = detector.window_samples();
    let mut readings = Vec::new();
    let mut at = 0;
    while at + window <= signal.len() {
        readings.extend(detector.read(&signal[at..at + window]));
        at += window / 2;
    }
    readings
}

fn flattest(readings: &[Reading]) -> f32 {
    readings
        .iter()
        .map(|reading| reading.flatness)
        .fold(0.0, f32::max)
}

fn white(seconds: f32, seed: u64) -> Vec<f32> {
    let mut noise = Noise(seed);
    (0..(seconds * RATE as f32) as usize)
        .map(|_| noise.next())
        .collect()
}

/// A loud four-to-the-floor loop at 130 BPM: a pitched kick on each beat,
/// an off-beat saw bass, closed hats on the sixteenths, a clap on two and
/// four, and a white noise riser that opens fully over its last bars.
fn techno(seconds: f32) -> Vec<f32> {
    let frames = (seconds * RATE as f32) as usize;
    let beat = 60.0 / 130.0;
    let mut noise = Noise(7);
    let mut riser = 0.0_f32;
    let mut out = Vec::with_capacity(frames);
    for frame in 0..frames {
        let t = frame as f32 / RATE as f32;
        let in_beat = t % beat;
        let kick_phase = std::f32::consts::TAU
            * (45.0 * in_beat + 75.0 * 0.03 * (1.0 - (-in_beat / 0.03).exp()));
        let kick = kick_phase.sin() * (-in_beat / 0.25).exp();
        let off = (t + beat / 2.0) % beat;
        let bass = (2.0 * ((55.0 * t) % 1.0) - 1.0) * (-off / 0.15).exp() * 0.5;
        let white = noise.next();
        let sixteenth = t % (beat / 4.0);
        let hat = (white - riser) * (-sixteenth / 0.02).exp() * 0.3;
        let clap = if (t / beat) as usize % 2 == 1 {
            white * (-in_beat / 0.08).exp() * 0.4
        } else {
            0.0
        };
        // The riser's one-pole low-pass opens from 200 Hz to fully open.
        let open = (t / seconds).powi(2);
        let cutoff = 200.0 + open * 20_000.0;
        let coefficient = 1.0 - (-std::f32::consts::TAU * cutoff / RATE as f32).exp();
        riser += coefficient * (white - riser);
        out.push(kick + bass + hat + clap + riser * 0.5 * open);
    }
    let peak = out.iter().fold(0.0_f32, |peak, x| peak.max(x.abs()));
    out.iter_mut().for_each(|x| *x *= 0.95 / peak);
    out
}

/// Closed hats on the sixteenths and an open hat on the off-beats, from
/// steeply high-passed noise, as loud as the white noise below.
fn hats(seconds: f32) -> Vec<f32> {
    let mut noise = Noise(11);
    let beat = 60.0 / 130.0;
    let (mut last, mut before) = (0.0_f32, 0.0_f32);
    let signal = (0..(seconds * RATE as f32) as usize)
        .map(|frame| {
            let t = frame as f32 / RATE as f32;
            let white = noise.next();
            // A third difference: a high-pass of 18 dB an octave.
            let high = white - 3.0 * last + 3.0 * before;
            before = last;
            last = white;
            let closed = (-(t % (beat / 4.0)) / 0.025).exp();
            let open = (-((t + beat / 2.0) % beat) / 0.15).exp();
            high * (closed + open)
        })
        .collect();
    at_level(signal, -10.0)
}

/// White noise through a resonant band-pass that sweeps from 200 Hz to
/// 12 kHz and back, as loud as the white noise below.
fn sweep(seconds: f32) -> Vec<f32> {
    let mut noise = Noise(13);
    let frames = (seconds * RATE as f32) as usize;
    let (mut low, mut band) = (0.0_f32, 0.0_f32);
    let signal = (0..frames)
        .map(|frame| {
            let position = frame as f32 / frames as f32;
            let up = 1.0 - (2.0 * position - 1.0).abs();
            let cutoff = 200.0 * 60_f32.powf(up);
            let f = 2.0 * (std::f32::consts::PI * cutoff / (2.0 * RATE as f32)).sin();
            let q = 0.25;
            let high = noise.next() - low - q * band;
            band += f * high;
            low += f * band;
            band
        })
        .collect();
    at_level(signal, -10.0)
}

#[test]
fn white_noise_is_noisy_and_music_hats_and_sweeps_are_not() {
    let noise = windows(&stereo(&at_level(white(3.0, 3), -10.0)));
    assert!(noise.iter().all(Reading::noisy), "{noise:?}");
    assert!(flattest(&noise) > 0.85);
    // Quieter noise is below the level threshold.
    let quiet = windows(&stereo(&at_level(white(2.0, 5), -20.0)));
    assert!(!quiet.iter().any(Reading::noisy), "{quiet:?}");
    let music = windows(&stereo(&techno(16.0)));
    assert!(
        music
            .iter()
            .any(|reading| reading.rms_dbfs > NOISE_RMS_DBFS)
    );
    assert!(!music.iter().any(Reading::noisy), "{}", flattest(&music));
    let hats = windows(&stereo(&hats(8.0)));
    assert!(!hats.iter().any(Reading::noisy), "{}", flattest(&hats));
    let sweep = windows(&stereo(&sweep(8.0)));
    assert!(!sweep.iter().any(Reading::noisy), "{}", flattest(&sweep));
    println!(
        "flatness: white {:.2}, music {:.2}, hats {:.2}, sweep {:.2}",
        flattest(&noise),
        flattest(&music),
        flattest(&hats),
        flattest(&sweep)
    );
}

/// The watcher's look `index` after `start`.
fn look(start: Instant, index: u32) -> Instant {
    start + WATCH_PERIOD * index
}

#[test]
fn a_non_finite_sample_before_watcher_start_still_schedules_a_capture() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let flight = Flight::with_seconds(2, 8000, 2, 1);
    flight.writer().finish(1);
    let mut watcher = Watcher::new(&flight, &LAST);
    let start = Instant::now();
    assert_eq!(watcher.check(&flight, start, false), None);
    let (due, reason) = watcher
        .pending
        .expect("the startup fault schedules a capture");
    assert_eq!(due, start + AUTO_DELAY);
    assert!(reason.contains("1 non-finite samples"), "{reason}");
}

#[test]
fn a_stream_error_before_watcher_start_still_schedules_a_capture() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let flight = Flight::with_seconds(2, 8000, 2, 1);
    flight.record_stream_error();
    let mut watcher = Watcher::new(&flight, &LAST);
    let start = Instant::now();
    assert_eq!(watcher.check(&flight, start, false), None);
    let (due, reason) = watcher
        .pending
        .expect("the startup fault schedules a capture");
    assert_eq!(due, start + AUTO_DELAY);
    assert_eq!(reason, "Automatic: The audio stream reported an error");
}

#[test]
fn a_stopped_noise_tail_is_not_sustained_noise() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let flight = Flight::with_seconds(2, RATE, 2, 1);
    let mut writer = flight.writer();
    for sample in stereo(&at_level(white(0.5, 31), -6.0)) {
        writer.sample(Some(sample), sample);
    }
    writer.finish(0);
    let mut watcher = Watcher::new(&flight, &LAST);
    let start = Instant::now();
    for index in 0..24 {
        assert_eq!(watcher.check(&flight, look(start, index), false), None);
        assert!(watcher.pending.is_none(), "stalled look {index}");
    }
}

#[test]
fn noise_must_advance_for_more_than_a_second_before_a_capture() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let flight = Flight::with_seconds(2, RATE, 2, 1);
    let mut writer = flight.writer();
    let noise = stereo(&at_level(white(2.0, 37), -6.0));
    let window = RATE as usize;
    for &sample in &noise[..window] {
        writer.sample(Some(sample), sample);
    }
    writer.finish(0);
    let mut watcher = Watcher::new(&flight, &LAST);
    let start = Instant::now();
    assert_eq!(watcher.check(&flight, start, false), None);
    // Four new frames cannot turn half a second of noise into a sustained fault.
    for frame in 0..4 {
        for &sample in &noise[window + frame * 2..][..2] {
            writer.sample(Some(sample), sample);
        }
        writer.finish(0);
        assert_eq!(
            watcher.check(&flight, look(start, frame as u32 + 1), false),
            None
        );
        assert!(watcher.pending.is_none(), "only {} new frames", frame + 1);
    }
}

#[test]
fn stopping_a_watcher_saves_its_pending_capture_once() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let flight = Flight::with_seconds(2, 8000, 2, 1);
    let mut watcher = Watcher::new(&flight, &LAST);
    flight.record_stream_error();
    let start = Instant::now();
    assert_eq!(watcher.check(&flight, start, false), None);
    let (_, reason) = watcher.pending.clone().expect("a pending capture");
    assert_eq!(
        watcher.check(&flight, start + WATCH_PERIOD, true),
        Some(reason)
    );
    assert!(watcher.pending.is_none());
    assert_eq!(watcher.check(&flight, start + AUTO_DELAY, true), None);
}

#[test]
fn sustained_noise_schedules_one_capture_a_few_seconds_later() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let flight = Flight::with_seconds(2, RATE, 2, 1);
    let mut writer = flight.writer();
    let mut watcher = Watcher::new(&flight, &LAST);
    let start = Instant::now();
    let music = stereo(&techno(4.0));
    let noise = stereo(&at_level(white(4.0, 17), -6.0));
    let quarter = RATE as usize / 2;
    let mut index = 0;
    let mut feed = |watcher: &mut Watcher, writer: &mut Writer, samples: &[f32]| {
        let mut due = Vec::new();
        for chunk in samples.chunks(quarter) {
            for sample in chunk {
                writer.sample(Some(*sample), *sample);
            }
            writer.finish(0);
            index += 1;
            due.push(watcher.check(&flight, look(start, index), false));
        }
        due
    };
    // The music fills the ring first; then noise starts.
    assert!(
        feed(&mut watcher, &mut writer, &music)
            .iter()
            .all(Option::is_none)
    );
    assert!(watcher.pending.is_none());
    let due = feed(&mut watcher, &mut writer, &noise);
    // The fault is the fourth noisy window in a row: the fourth look into
    // the noise, or the fifth when the first window, half music, passes.
    let (at, reason) = watcher.pending.clone().expect("a pending capture");
    assert!(due.iter().all(Option::is_none));
    assert!(
        reason.starts_with("Automatic: Sustained broadband noise"),
        "{reason}"
    );
    assert!(
        [look(start, 16 + 4), look(start, 16 + 5)].contains(&(at - AUTO_DELAY)),
        "{:?}",
        at - start
    );
    // The capture is due once, after the delay.
    assert_eq!(watcher.check(&flight, at, false), Some(reason));
    assert_eq!(watcher.check(&flight, at + WATCH_PERIOD, false), None);
    // Noise that goes on is not a second fault within five minutes.
    let more = stereo(&at_level(white(3.0, 19), -6.0));
    assert!(
        feed(&mut watcher, &mut writer, &more)
            .iter()
            .all(Option::is_none)
    );
    assert!(watcher.pending.is_none());
}

#[test]
fn a_non_finite_sample_or_a_stream_error_schedules_a_capture_at_most_every_five_minutes() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let flight = Flight::with_seconds(2, 8000, 2, 1);
    let mut writer = flight.writer();
    let mut watcher = Watcher::new(&flight, &LAST);
    let start = Instant::now();
    assert_eq!(watcher.check(&flight, start, false), None);
    writer.finish(3);
    assert_eq!(watcher.check(&flight, start, false), None);
    let (at, reason) = watcher.pending.clone().expect("a pending capture");
    assert_eq!(at, start + AUTO_DELAY);
    assert!(reason.contains("3 non-finite samples"), "{reason}");
    assert_eq!(watcher.check(&flight, at, false), Some(reason));
    // A stream error a minute later is logged but not captured.
    flight.record_stream_error();
    assert_eq!(
        watcher.check(&flight, start + Duration::from_secs(60), false),
        None
    );
    assert!(watcher.pending.is_none());
    // Five minutes after the first, it is.
    flight.record_stream_error();
    let later = start + AUTO_INTERVAL;
    assert_eq!(watcher.check(&flight, later, false), None);
    let (at, reason) = watcher.pending.clone().expect("a pending capture");
    assert_eq!(reason, "Automatic: The audio stream reported an error");
    assert_eq!(watcher.check(&flight, at, false), Some(reason));
}

#[test]
fn the_rate_limit_allows_one_capture_in_five_minutes() {
    let mut last = None;
    let start = Instant::now();
    assert!(allow_automatic(&mut last, start));
    assert!(!allow_automatic(&mut last, start + Duration::from_secs(1)));
    assert!(!allow_automatic(
        &mut last,
        start + AUTO_INTERVAL - Duration::from_millis(1)
    ));
    assert!(allow_automatic(&mut last, start + AUTO_INTERVAL));
}

/// Reads every WAV file under `MIXJAM_FLIGHT_WAVS` as the watcher would and
/// reports, per file, the flattest window louder than the level threshold,
/// the flattest window of any audible level (a quiet solo lane's spectrum
/// is the same when it is played loud), and the longest run of noisy
/// windows.
///
/// ```text
/// MIXJAM_FLIGHT_WAVS=renders cargo test --release --lib flight_threshold_scan -- --ignored --nocapture
/// ```
#[test]
#[ignore = "reads local renders"]
fn flight_threshold_scan() {
    let root = PathBuf::from(std::env::var("MIXJAM_FLIGHT_WAVS").expect("MIXJAM_FLIGHT_WAVS"));
    let mut files = Vec::new();
    let mut folders = vec![root];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                folders.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let (mut overall, mut audible_overall, mut triggered) = (0.0_f32, 0.0_f32, 0);
    for path in files {
        let Ok(mut reader) = hound::WavReader::open(&path) else {
            continue;
        };
        let spec = reader.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => {
                reader.samples::<f32>().map(|x| x.unwrap_or(0.0)).collect()
            }
            hound::SampleFormat::Int => {
                let scale = (1_i64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|x| x.unwrap_or(0) as f32 / scale)
                    .collect()
            }
        };
        let channels = usize::from(spec.channels);
        let mut detector = Detector::new(spec.sample_rate, spec.channels);
        let window = detector.window_samples();
        let step = window / 2 / channels * channels;
        let (mut flattest, mut audible, mut loudest, mut run, mut longest) =
            (0.0_f32, 0.0_f32, f32::NEG_INFINITY, 0, 0);
        let mut at = 0;
        while at + window <= samples.len() {
            let readings = detector.read(&samples[at..at + window]);
            for reading in &readings {
                loudest = loudest.max(reading.rms_dbfs);
                if reading.rms_dbfs > NOISE_RMS_DBFS {
                    flattest = flattest.max(reading.flatness);
                }
                if reading.rms_dbfs > -40.0 {
                    audible = audible.max(reading.flatness);
                }
            }
            run = if readings.iter().any(Reading::noisy) {
                run + 1
            } else {
                0
            };
            longest = longest.max(run);
            at += step;
        }
        overall = overall.max(flattest);
        audible_overall = audible_overall.max(audible);
        triggered += usize::from(longest >= NOISE_WINDOWS);
        println!(
            "{}: loudest window {loudest:.1} dBFS, flattest loud window {flattest:.2}, \
             flattest window above -40 dBFS {audible:.2}, longest noisy run {longest}",
            path.display()
        );
    }
    println!(
        "flattest loud window of all {overall:.2}, above -40 dBFS {audible_overall:.2}; \
         files that would trigger {triggered}"
    );
}

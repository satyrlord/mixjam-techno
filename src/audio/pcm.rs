//! Decoded audio: PCM buffers, WAV reading and writing, and the hiding of
//! stray ends.
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Pcm {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}
impl Pcm {
    pub fn frames(&self) -> u64 {
        self.samples.len() as u64 / u64::from(self.channels.max(1))
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.sample_rate == 0
            || self.sample_rate > 384_000
            || !(1..=2).contains(&self.channels)
            || !self.samples.len().is_multiple_of(self.channels as usize)
            || self.samples.iter().any(|x| !x.is_finite() || x.abs() > 1.0)
        {
            return Err(
                "The audio must contain normalized finite mono or stereo PCM samples.".into(),
            );
        }
        Ok(())
    }
    pub fn write_wav(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        // A failed export must not truncate a WAV that it would replace.
        crate::atomic_file::write_with(path, |file| self.write_wav_to(file))
    }
    /// Encode validated PCM into a staged file owned by the caller's batch.
    pub(crate) fn write_wav_to(&self, file: &mut std::fs::File) -> Result<(), String> {
        if self.samples.len() > MAX_WAV_SAMPLES {
            return Err("The audio is too long for a WAV file.".into());
        }
        let spec = hound::WavSpec {
            channels: self.channels,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(std::io::BufWriter::new(file), spec)
            .map_err(|e| e.to_string())?;
        for &s in &self.samples {
            writer
                .write_sample(float_to_i16(s))
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())
    }
}
pub(crate) fn float_to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * 32768.0)
        .round()
        .clamp(-32768.0, 32767.0) as i16
}

pub fn read_wav(path: &Path) -> Result<Pcm, String> {
    let mut samples = Vec::new();
    let (spec, _) = visit_wav_samples(path, |_, sample| samples.push(sample))?;
    Ok(Pcm {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        samples,
    })
}

/// Frames at the end of a sound that can be a stray.
const STRAY_END_FRAMES: usize = 4;

/// Near silence, in normalized units: the 4 frames before a stray run.
const STRAY_SILENCE: f32 = 0.001;

/// Hide a stray end: many original product samples are silent or quiet and
/// then end on one or two frames that jump to a large value, which clicks
/// wherever the sample ends or loops. Per channel, a run of at most four
/// final frames is stray when all its frames rise above four times the peak
/// of the ten milliseconds before the last four frames, or above twice that
/// peak when the four frames just before the run are near silence, and above
/// 1% of full scale in both cases. It takes the value of the frame before
/// the run. A single last frame is also stray when its step from the frame
/// before is over four times the largest step in that window (and over 5% of
/// full scale). Real tails, such as a sound cut while loud, are as loud and
/// as jumpy as the audio before them and stay. The frame count never changes.
/// Playback and render read audio through this. Sample identity must not.
pub(crate) fn hide_stray_end(pcm: &mut Pcm) {
    let channels = usize::from(pcm.channels.max(1));
    let frames = pcm.samples.len() / channels;
    let window = (0.010 * f64::from(pcm.sample_rate)).round() as usize;
    if frames < window + STRAY_END_FRAMES + 1 || frames < 2 * STRAY_END_FRAMES {
        return;
    }
    for channel in 0..channels {
        let at = |frame: usize| frame * channels + channel;
        let peak = (frames - STRAY_END_FRAMES - window..frames - STRAY_END_FRAMES)
            .map(|frame| pcm.samples[at(frame)].abs())
            .fold(0.0, f32::max);
        let above = |threshold: f32| {
            (1..=STRAY_END_FRAMES)
                .take_while(|back| pcm.samples[at(frames - back)].abs() > threshold)
                .count()
        };
        let mut run = above((4.0 * peak).max(0.01));
        let loose = above((2.0 * peak).max(0.01));
        for candidate in (run + 1..=loose).rev() {
            let silent = (frames - candidate - STRAY_END_FRAMES..frames - candidate)
                .all(|frame| pcm.samples[at(frame)].abs() <= STRAY_SILENCE);
            if silent {
                run = candidate;
                break;
            }
        }
        if run == 0 {
            // A lone last frame that steps far more than anything in the
            // window steps, whatever the tail before it was doing.
            let steps = (frames - STRAY_END_FRAMES - window + 1..frames - STRAY_END_FRAMES)
                .map(|frame| (pcm.samples[at(frame)] - pcm.samples[at(frame - 1)]).abs())
                .fold(0.0, f32::max);
            let last = (pcm.samples[at(frames - 1)] - pcm.samples[at(frames - 2)]).abs();
            if last > (4.0 * steps).max(0.05) {
                run = 1;
            }
        }
        if run > 0 {
            let hold = pcm.samples[at(frames - run - 1)];
            for frame in frames - run..frames {
                pcm.samples[at(frame)] = hold;
            }
        }
    }
}

/// The most samples of all channels that a 16-bit WAV can hold: the data
/// chunk size and the RIFF size are 32-bit, and the RIFF payload also holds
/// 36 bytes of headers. This is a format limit, not a choice; a longer sound
/// cannot be saved or exported as one WAV file.
pub(crate) const MAX_WAV_SAMPLES: usize = ((u32::MAX - 36) / 2) as usize;

/// Validate and normalize WAV samples without retaining decoded audio.
pub(crate) fn visit_wav_samples(
    path: &Path,
    mut visit: impl FnMut(hound::WavSpec, f32),
) -> Result<(hound::WavSpec, u32), String> {
    let cannot = |error: String| format!("Cannot read {}: {error}", path.display());
    let mut file =
        std::io::BufReader::new(std::fs::File::open(path).map_err(|e| cannot(e.to_string()))?);
    let length = file
        .get_ref()
        .metadata()
        .map_err(|e| cannot(e.to_string()))?
        .len();
    let layout = crate::wav::read_layout(&mut file, length).map_err(cannot)?;
    let spec = layout.spec;
    let float = spec.sample_format == hound::SampleFormat::Float;
    crate::wav::visit_samples(&mut file, &layout, |value| {
        // Float audio may overshoot full scale a little; it plays clipped.
        let value = if float && value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            value
        };
        if !value.is_finite() || value.abs() > 1.0 {
            return Err(
                "The audio must contain normalized finite mono or stereo PCM samples.".into(),
            );
        }
        visit(spec, value);
        Ok(())
    })?;
    Ok((spec, layout.frames() as u32))
}

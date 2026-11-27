//! Pro-audio 6-bay effects chain (`Filter`, `Modulation`, `Echo`, `Robotize`,
//! `Reverb`, `Distortion / Output`) with non-destructive bypass, TailMode policies,
//! 16x FIR + ADAA2 nonlinear processing, and built-in presets.
use crate::audio::Pcm;
use crate::fx::oversample::FirOversampler;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

mod distortion;
mod echo;
mod filter;
mod modulation;
mod presets;
mod reverb;
mod robotize;

pub use distortion::sample_drive_curve;
pub use filter::sample_filter_curve;
pub use presets::{EffectPreset, effect_presets, matching_effect_preset};

/// The shortest echo delay: a sixteenth of a beat.
pub const MIN_ECHO_BEATS: f32 = 0.0625;
const OLD_MIN_ECHO_BEATS: f32 = 0.001;
const RING_OUT_MAX_SECONDS: f64 = 6.0;
const RING_OUT_SILENCE_RMS: f64 = 0.000_251_188_64; // -72 dBFS
const RING_OUT_CHUNK: usize = 1024;

pub(super) trait StereoTailEngine {
    fn step_tail(&mut self, dry: [f64; 2], produce: bool) -> [f64; 2];
}

pub(super) fn run_stereo_tail_engine<E: StereoTailEngine>(
    engine: &mut E,
    stereo: &mut Vec<[f64; 2]>,
    warm_passes: usize,
    ring_wait: Option<usize>,
    max_frames: usize,
    fade_frames: usize,
) -> bool {
    for _ in 0..warm_passes {
        for &frame in stereo.iter() {
            engine.step_tail(frame, false);
        }
    }
    for frame in stereo.iter_mut() {
        *frame = engine.step_tail(*frame, true);
    }
    if let Some(min_wait) = ring_wait {
        return extend_ring_out(stereo, max_frames, min_wait, fade_frames, |f| {
            engine.step_tail(f, true)
        });
    }
    false
}

#[derive(Clone, Copy)]
pub(super) struct TptPole {
    s: f64,
    g: f64,
    active: bool,
}
impl TptPole {
    pub(super) fn new(hz: f64, rate: f64, active: bool) -> Self {
        if !active {
            return Self {
                s: 0.0,
                g: 1.0,
                active: false,
            };
        }
        let w = (PI * hz.max(10.0).min(0.48 * rate) / rate).tan();
        Self {
            s: 0.0,
            g: w / (1.0 + w),
            active: true,
        }
    }

    #[inline]
    pub(super) fn tick(&mut self, x: f64) -> f64 {
        if !self.active {
            return x;
        }
        let v = (x - self.s) * self.g;
        let y = v + self.s;
        let next = y + v;
        self.s = if next.abs() < 1e-30 { 0.0 } else { next };
        y
    }
}

fn default_true() -> bool {
    true
}

fn deserialize_legacy_bay<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn echo_delay<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    Ok(if (OLD_MIN_ECHO_BEATS..MIN_ECHO_BEATS).contains(&value) {
        MIN_ECHO_BEATS
    } else {
        value
    })
}

/// Filter response topology.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    #[default]
    Lp12,
    Lp24,
    Bp12,
    Hp12,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub mode: FilterMode,
    pub cutoff_hz: f32,
    pub resonance: f32,
    pub drive: f32,
    pub wet: f32,
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: FilterMode::Lp12,
            cutoff_hz: 2000.0,
            resonance: 0.0,
            drive: 0.0,
            wet: 1.0,
        }
    }
}

/// Stereo modulation algorithm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulationMode {
    #[default]
    Chorus,
    Phaser,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modulation {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub mode: ModulationMode,
    pub rate_hz: f32,
    pub depth: f32,
    pub feedback: f32,
    pub wet: f32,
}
impl Default for Modulation {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: ModulationMode::Chorus,
            rate_hz: 0.65,
            depth: 0.45,
            feedback: 0.25,
            wet: 0.45,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Echo {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Four beats are one bar.
    #[serde(deserialize_with = "echo_delay")]
    pub delay_beats: f32,
    pub feedback: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub tone: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub ping_pong: bool,
    pub wet: f32,
}
impl Default for Echo {
    fn default() -> Self {
        Self {
            enabled: false,
            delay_beats: 0.5,
            feedback: 0.4,
            tone: 0.0,
            ping_pong: false,
            wet: 0.35,
        }
    }
}

/// Frequency/ring modulation topology.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotizeMode {
    #[default]
    RingMod,
    FreqShift,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Robotize {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub mode: RobotizeMode,
    pub frequency_hz: f32,
    pub wet: f32,
}
impl Default for Robotize {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: RobotizeMode::RingMod,
            frequency_hz: 80.0,
            wet: 0.5,
        }
    }
}

/// FDN reverb space geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReverbSpace {
    Room,
    #[default]
    Hall,
    Plate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reverb {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub space: ReverbSpace,
    pub decay_s: f32,
    pub damping: f32,
    pub wet: f32,
}
impl Default for Reverb {
    fn default() -> Self {
        Self {
            enabled: false,
            space: ReverbSpace::Hall,
            decay_s: 2.2,
            damping: 0.4,
            wet: 0.3,
        }
    }
}

/// Output waveshaper character.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistortionShape {
    #[default]
    Warm,
    Acid,
    Fuzz,
    Crush,
}

/// Effect Studio tail rendering mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TailMode {
    #[default]
    Trim,
    WrapLoop,
    RingOut,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectsState {
    #[serde(
        default,
        deserialize_with = "deserialize_legacy_bay",
        skip_serializing_if = "is_default"
    )]
    pub filter: Filter,
    #[serde(default, skip_serializing_if = "is_default")]
    pub modulation: Modulation,
    #[serde(
        default,
        deserialize_with = "deserialize_legacy_bay",
        skip_serializing_if = "is_default"
    )]
    pub echo: Echo,
    #[serde(
        default,
        deserialize_with = "deserialize_legacy_bay",
        skip_serializing_if = "is_default"
    )]
    pub robotize: Robotize,
    #[serde(default, skip_serializing_if = "is_default")]
    pub reverb: Reverb,
    #[serde(default, skip_serializing_if = "is_default")]
    pub dist_shape: DistortionShape,
    pub dirty: f32,
    pub noise: f32,
    pub volume: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub tail_mode: TailMode,
}

impl Default for EffectsState {
    fn default() -> Self {
        Self {
            filter: Filter::default(),
            modulation: Modulation::default(),
            echo: Echo::default(),
            robotize: Robotize::default(),
            reverb: Reverb::default(),
            dist_shape: DistortionShape::Warm,
            dirty: 0.0,
            noise: 0.0,
            volume: 1.0,
            tail_mode: TailMode::Trim,
        }
    }
}

impl EffectsState {
    /// True when no bay or output stage alters audio.
    pub fn is_bypassed(&self) -> bool {
        let bays = [
            (self.filter.enabled, self.filter.wet),
            (self.modulation.enabled, self.modulation.wet),
            (self.echo.enabled, self.echo.wet),
            (self.robotize.enabled, self.robotize.wet),
            (self.reverb.enabled, self.reverb.wet),
        ];
        bays.iter().all(|&(enabled, wet)| !enabled || wet == 0.0)
            && self.dirty == 0.0
            && self.noise == 0.0
            && self.volume == 1.0
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_output()?;
        self.filter.validate()?;
        self.modulation.validate()?;
        self.echo.validate()?;
        self.robotize.validate()?;
        self.reverb.validate()
    }

    fn validate_output(&self) -> Result<(), String> {
        if !unit_f32(self.dirty)
            || !unit_f32(self.noise)
            || !self.volume.is_finite()
            || !(0.0..=2.0).contains(&self.volume)
        {
            return Err("Effect volume or distortion is outside its permitted range.".into());
        }
        Ok(())
    }
}

#[inline]
fn unit_f32(x: f32) -> bool {
    x.is_finite() && (0.0..=1.0).contains(&x)
}

impl Filter {
    fn validate(&self) -> Result<(), String> {
        if !self.cutoff_hz.is_finite()
            // Saved patterns can use ultrasonic cutoffs at high sample rates.
            || !(20.0..=180_000.0).contains(&self.cutoff_hz)
            || !unit_f32(self.resonance)
            || !unit_f32(self.drive)
            || !unit_f32(self.wet)
        {
            return Err("Filter values are outside their permitted ranges.".into());
        }
        Ok(())
    }
}

impl Modulation {
    fn validate(&self) -> Result<(), String> {
        if !self.rate_hz.is_finite()
            || !(0.05..=8.0).contains(&self.rate_hz)
            || !unit_f32(self.depth)
            || !self.feedback.is_finite()
            || !(-0.85..=0.85).contains(&self.feedback)
            || !unit_f32(self.wet)
        {
            return Err("Modulation values are outside their permitted ranges.".into());
        }
        Ok(())
    }
}

impl Echo {
    fn validate(&self) -> Result<(), String> {
        if !self.delay_beats.is_finite()
            || !(MIN_ECHO_BEATS..=2.0).contains(&self.delay_beats)
            || !self.feedback.is_finite()
            || !(0.0..=0.95).contains(&self.feedback)
            || !unit_f32(self.tone)
            || !unit_f32(self.wet)
        {
            return Err("Echo delay, feedback, or volume is outside its permitted range.".into());
        }
        Ok(())
    }
}

impl Robotize {
    fn validate(&self) -> Result<(), String> {
        if !self.frequency_hz.is_finite()
            || !(1.0..=20000.0).contains(&self.frequency_hz)
            || !unit_f32(self.wet)
        {
            return Err("Robotize color or volume is outside its permitted range.".into());
        }
        Ok(())
    }
}

impl Reverb {
    fn validate(&self) -> Result<(), String> {
        if !self.decay_s.is_finite()
            || !(0.2..=8.0).contains(&self.decay_s)
            || !unit_f32(self.damping)
            || !unit_f32(self.wet)
        {
            return Err("Reverb decay, damping, or volume is outside its permitted range.".into());
        }
        Ok(())
    }
}

/// Process a standalone sample in Effect Studio honoring `state.tail_mode`.
pub fn process(input: &Pcm, state: &EffectsState, bpm: f64) -> Result<Pcm, String> {
    input.validate()?;
    let output = process_validated(input, state, bpm, 1.0, state.tail_mode, None)?;
    output.validate()?;
    Ok(output)
}

/// Sample-accurate per-frame filter modulation supplied by Step Synth.
#[derive(Clone, Copy)]
pub(crate) struct FilterMod<'a> {
    pub cutoff_octaves: &'a [f32],
    pub res_boost: &'a [f32],
}

#[cfg(test)]
pub(crate) fn process_synth(
    input: &Pcm,
    state: &EffectsState,
    bpm: f64,
) -> Result<(Pcm, f32), String> {
    process_synth_voiced(input, state, bpm, None)
}

/// Process a Step Synth sequence with optional per-sample cutoff and resonance modulation.
pub(crate) fn process_synth_voiced(
    input: &Pcm,
    state: &EffectsState,
    bpm: f64,
    filter_mod: Option<FilterMod<'_>>,
) -> Result<(Pcm, f32), String> {
    input.validate_with_peak_limit(4.0)?;
    let mut output =
        process_validated(input, state, bpm, f32::MAX, TailMode::WrapLoop, filter_mod)?;
    let peak = output
        .samples
        .iter()
        .fold(0.0_f32, |peak, x| peak.max(x.abs()));
    let gain = if peak > 1.0 { 1.0 / peak } else { 1.0 };
    if gain < 1.0 {
        for sample in &mut output.samples {
            *sample = (*sample * gain).clamp(-1.0, 1.0);
        }
    }
    output.validate()?;
    Ok((output, gain))
}

/// Shared zero-latency 16x linear-phase FIR runner with optional loop pre-roll.
pub(crate) fn run_oversampled_16x(
    samples: &mut [f64],
    looped: bool,
    shape: impl FnMut(f64) -> f64,
) {
    oversample_with_preroll(samples, looped, 256, shape);
}

fn oversample_with_preroll(
    samples: &mut [f64],
    looped: bool,
    warm_frames: usize,
    mut shape: impl FnMut(f64) -> f64,
) {
    let n = samples.len();
    if n == 0 {
        return;
    }
    crate::fx::prepare_tables();
    let lat = FirOversampler::LATENCY_16X;
    let pre = if looped { warm_frames } else { 0 };
    let pre_start = n - pre % n;
    let mut os = FirOversampler::new(true);
    let total = pre + n + lat;
    let src = samples.to_vec();
    for k in 0..total {
        let x = if k < pre {
            src[(pre_start + k) % n]
        } else if k < pre + n {
            src[k - pre]
        } else if looped {
            src[(k - pre - n) % n]
        } else {
            0.0
        };
        let y = os.process(x, &mut shape);
        if k >= pre + lat {
            samples[k - pre - lat] = y;
        }
    }
}

fn needs_stereo(input_channels: u16, state: &EffectsState) -> bool {
    if input_channels >= 2 {
        return true;
    }
    (state.modulation.enabled && state.modulation.wet > 0.0)
        || (state.reverb.enabled && state.reverb.wet > 0.0)
        || (state.echo.enabled && state.echo.wet > 0.0 && state.echo.ping_pong)
}

fn render_frame_limit(frames: usize, channels: usize, rate: f64) -> Result<usize, String> {
    let wav_frames = crate::audio::MAX_WAV_SAMPLES / channels;
    if frames > wav_frames {
        return Err("The sample is too long for a WAV file.".into());
    }
    let tail_frames = (rate * RING_OUT_MAX_SECONDS).round() as usize;
    Ok(wav_frames.min(frames.saturating_add(tail_frames)))
}

fn validate_filter_mod(filter_mod: Option<FilterMod<'_>>, frames: usize) -> Result<(), String> {
    if let Some(m) = filter_mod
        && (m.cutoff_octaves.len() != frames || m.res_boost.len() != frames)
    {
        return Err("Filter modulation length must match audio frames.".into());
    }
    Ok(())
}

fn apply_filter_stage(
    channels: &mut [Vec<f64>],
    filter: &Filter,
    rate: f64,
    looped: bool,
    filter_mod: Option<FilterMod<'_>>,
) {
    for ch in channels {
        filter::apply(ch, filter, rate, looped, filter_mod);
    }
}

fn process_validated(
    input: &Pcm,
    state: &EffectsState,
    bpm: f64,
    ceiling: f32,
    tail: TailMode,
    filter_mod: Option<FilterMod<'_>>,
) -> Result<Pcm, String> {
    state.validate()?;
    if !bpm.is_finite() || !crate::engine::TEMPO_RANGE.contains(&bpm) {
        return Err("Source tempo must be from 7.5 to 1200 BPM.".into());
    }
    let out_channels = if needs_stereo(input.channels, state) {
        2
    } else {
        1
    };
    let frames = input.frames() as usize;
    let rate = f64::from(input.sample_rate);
    validate_filter_mod(filter_mod, frames)?;
    let max_frames = render_frame_limit(frames, out_channels, rate)?;
    if state.is_bypassed() {
        return Ok(input.clone());
    }
    let looped = tail == TailMode::WrapLoop;

    let mut channels = extract_channels(input, out_channels);
    apply_noise(&mut channels, state.noise);
    apply_filter_stage(&mut channels, &state.filter, rate, looped, filter_mod);
    let mut stereo = to_stereo_frames(&channels);
    modulation::apply(&mut stereo, &state.modulation, rate, looped);

    let mut extended = run_echo_stage(&mut stereo, state, rate, bpm, tail, max_frames);
    run_robotize_stereo(&mut stereo, &state.robotize, rate, looped);
    extended |= run_reverb_stage(&mut stereo, &state.reverb, rate, tail, looped, max_frames);
    if extended {
        apply_tail_fade(&mut stereo[frames..], tail_fade_frames(rate));
    }
    finish_output(
        &stereo,
        out_channels,
        state,
        ceiling,
        looped,
        input.sample_rate,
    )
}

fn run_echo_stage(
    stereo: &mut Vec<[f64; 2]>,
    state: &EffectsState,
    rate: f64,
    bpm: f64,
    tail: TailMode,
    max_frames: usize,
) -> bool {
    if !state.echo.enabled || state.echo.wet <= 0.0 {
        return false;
    }
    let loop_frames = (tail == TailMode::WrapLoop).then_some(stereo.len());
    let mut engine = echo::EchoEngine::new(&state.echo, rate, bpm, loop_frames);
    let warm = engine.warm_passes(stereo.len(), tail);
    let wait = (tail == TailMode::RingOut).then(|| engine.delay_frames() + RING_OUT_CHUNK);
    run_stereo_tail_engine(
        &mut engine,
        stereo,
        warm,
        wait,
        max_frames,
        tail_fade_frames(rate),
    )
}

fn run_reverb_stage(
    stereo: &mut Vec<[f64; 2]>,
    reverb: &Reverb,
    rate: f64,
    tail: TailMode,
    looped: bool,
    max_frames: usize,
) -> bool {
    if !reverb.enabled || reverb.wet <= 0.0 {
        return false;
    }
    let mut engine = reverb::ReverbEngine::new(reverb, rate, looped.then_some(stereo.len()));
    let warm = reverb::warm_passes(stereo.len(), reverb.decay_s, rate, tail);
    let wait = (tail == TailMode::RingOut).then(|| (0.15 * rate).round() as usize);
    run_stereo_tail_engine(
        &mut engine,
        stereo,
        warm,
        wait,
        max_frames,
        tail_fade_frames(rate),
    )
}

fn extract_channels(input: &Pcm, out_channels: usize) -> Vec<Vec<f64>> {
    let in_ch = usize::from(input.channels);
    let frames = input.frames() as usize;
    let mut channels = vec![Vec::with_capacity(frames); out_channels];
    for frame in 0..frames {
        let base = frame * in_ch;
        let l = f64::from(input.samples[base]);
        let r = if in_ch == 2 {
            f64::from(input.samples[base + 1])
        } else {
            l
        };
        channels[0].push(l);
        if out_channels == 2 {
            channels[1].push(r);
        }
    }
    channels
}

fn to_stereo_frames(channels: &[Vec<f64>]) -> Vec<[f64; 2]> {
    let frames = channels[0].len();
    let mut stereo = Vec::with_capacity(frames);
    for i in 0..frames {
        let l = channels[0][i];
        let r = if channels.len() > 1 {
            channels[1][i]
        } else {
            l
        };
        stereo.push([l, r]);
    }
    stereo
}

fn apply_noise(channels: &mut [Vec<f64>], noise_amount: f32) {
    if noise_amount <= 0.0 {
        return;
    }
    let scale = f64::from(noise_amount);
    for (c, ch) in channels.iter_mut().enumerate() {
        let mut rng = 0x6d2b_79f5_u32.wrapping_add(c as u32);
        for sample in ch.iter_mut() {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let rand = rng as f64 / u32::MAX as f64 * 2.0 - 1.0;
            *sample += sample.abs() * rand * scale;
        }
    }
}

fn extend_ring_out(
    stereo: &mut Vec<[f64; 2]>,
    max_frames: usize,
    min_wait_frames: usize,
    fade_frames: usize,
    mut step: impl FnMut([f64; 2]) -> [f64; 2],
) -> bool {
    let before = stereo.len();
    let mut active_end = before;
    while stereo.len() < max_frames {
        let n = RING_OUT_CHUNK.min(max_frames - stereo.len());
        let mut energy = 0.0_f64;
        for _ in 0..n {
            let out = step([0.0, 0.0]);
            energy += out[0] * out[0] + out[1] * out[1];
            stereo.push(out);
        }
        let rms = (energy / (2.0 * n as f64)).sqrt();
        if rms >= RING_OUT_SILENCE_RMS {
            active_end = stereo.len();
        } else if stereo.len() - active_end >= min_wait_frames.max(fade_frames) {
            break;
        }
    }
    let extended = active_end > before;
    let keep = if extended {
        active_end + fade_frames
    } else {
        before
    };
    stereo.truncate(keep);
    extended
}

fn tail_fade_frames(rate: f64) -> usize {
    (0.015 * rate).round().max(1.0) as usize
}

fn apply_tail_fade(stereo: &mut [[f64; 2]], fade_frames: usize) {
    let fade = fade_frames.min(stereo.len());
    let start = stereo.len() - fade;
    for (i, frame) in stereo[start..].iter_mut().enumerate() {
        let w = 0.5 * (1.0 + (PI * (i + 1) as f64 / fade as f64).cos());
        frame[0] *= w;
        frame[1] *= w;
    }
}

fn run_robotize_stereo(stereo: &mut [[f64; 2]], robotize: &Robotize, rate: f64, looped: bool) {
    if !robotize.enabled || robotize.wet <= 0.0 || stereo.is_empty() {
        return;
    }
    for ch in 0..2 {
        let mut buf: Vec<f64> = stereo.iter().map(|f| f[ch]).collect();
        robotize::apply(&mut buf, robotize, rate, looped);
        for (dst, val) in stereo.iter_mut().zip(buf) {
            dst[ch] = val;
        }
    }
}

fn trim_gibbs_peak(channels: &mut [Vec<f64>]) {
    let max_peak = channels
        .iter()
        .flat_map(|ch| ch.iter())
        .fold(0.0_f64, |p, x| p.max(x.abs()));
    if max_peak > 0.995 {
        let trim = 0.995 / max_peak;
        for ch in channels.iter_mut() {
            for sample in ch.iter_mut() {
                *sample *= trim;
            }
        }
    }
}

fn finish_output(
    stereo: &[[f64; 2]],
    out_channels: usize,
    state: &EffectsState,
    ceiling: f32,
    looped: bool,
    sample_rate: u32,
) -> Result<Pcm, String> {
    let frames = stereo.len();
    let vol = f64::from(state.volume);
    let ceil = f64::from(ceiling);
    let mut channels = Vec::with_capacity(out_channels);
    for ch in 0..out_channels {
        let mut buf: Vec<f64> = stereo.iter().map(|f| f[ch]).collect();
        distortion::apply_and_scale(&mut buf, state.dist_shape, state.dirty, vol, ceil, looped);
        channels.push(buf);
    }
    if ceil <= 1.01 {
        trim_gibbs_peak(&mut channels);
    }
    let mut interleaved = vec![0.0_f32; frames * out_channels];
    for (ch, buf) in channels.iter().enumerate() {
        for (i, &val) in buf.iter().enumerate() {
            interleaved[i * out_channels + ch] = val as f32;
        }
    }
    Ok(Pcm {
        sample_rate,
        channels: out_channels as u16,
        samples: interleaved,
    })
}

#[cfg(test)]
mod tests;

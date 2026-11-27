//! 8x oversampled PolyBLEP / PolyBLAMP band-limited synth oscillators with
//! phase-locked sub-oscillator and sharp linear-phase halfband FIR decimation.
use super::OscWaveform;
use crate::fx::oversample::{FirDecimator8x, prepare_tables};

const OVERSAMPLE: usize = 8;
const C2_HZ: f64 = 65.406_391_325_149_66;
const ATTACK_SECONDS: f64 = 0.002;

#[inline]
pub(super) fn note_freq_hz(octave: i8, semitones: f64, sample_rate: f64) -> f64 {
    let min_hz = 8.0_f64;
    let max_hz = (0.45 * sample_rate).max(min_hz);
    (C2_HZ * 2.0_f64.powf(f64::from(octave) + semitones / 12.0)).clamp(min_hz, max_hz)
}

#[inline]
fn poly_blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let u = t / dt;
        u + u - u * u - 1.0
    } else if t > 1.0 - dt {
        let u = (t - 1.0) / dt;
        u * u + u + u + 1.0
    } else {
        0.0
    }
}

#[inline]
fn poly_blamp_corner(t: f64, dt: f64) -> f64 {
    let dist = t.min(1.0 - t);
    if dist < dt {
        let u = 1.0 - dist / dt;
        dt * u * u * u / 6.0
    } else {
        0.0
    }
}

#[inline]
fn eval_waveform(wave: OscWaveform, phase: f64, dt: f64) -> f64 {
    match wave {
        OscWaveform::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        OscWaveform::Square => {
            let mut s = if phase < 0.5 { 1.0 } else { -1.0 };
            s += poly_blep(phase, dt);
            s -= poly_blep((phase + 0.5).fract(), dt);
            s
        }
        OscWaveform::Triangle => {
            let mut tri = 1.0 - 4.0 * (phase - 0.5).abs();
            tri += 8.0 * poly_blamp_corner(phase, dt);
            tri -= 8.0 * poly_blamp_corner((phase + 0.5).fract(), dt);
            tri
        }
    }
}

pub(super) struct Oscillator {
    phase: f64,
    sub_phase: f64,
}

impl Oscillator {
    pub(super) fn new() -> Self {
        prepare_tables();
        Self {
            phase: 0.0,
            sub_phase: 0.0,
        }
    }

    pub(super) fn reset(&mut self) {
        self.phase = 0.0;
        self.sub_phase = 0.0;
    }

    #[inline]
    fn tick_8x(&mut self, wave: OscWaveform, sub_weight: f64, freq_hz: f64, rate: f64) -> f32 {
        let dt = (freq_hz / (OVERSAMPLE as f64 * rate)).clamp(1e-7, 0.49);
        let sub_dt = 0.5 * dt;
        let main = eval_waveform(wave, self.phase, dt);
        let combined = if sub_weight > 0.0 {
            let sub = eval_waveform(OscWaveform::Square, self.sub_phase, sub_dt);
            let norm = 0.70 / (1.0 + 0.65 * sub_weight);
            (main + sub_weight * sub) * norm
        } else {
            0.70 * main
        };
        self.phase = (self.phase + dt).fract();
        self.sub_phase = (self.sub_phase + sub_dt).fract();
        combined as f32
    }
}

pub(super) struct OscPhraseConfig {
    pub wave: OscWaveform,
    pub sub: f32,
    pub octave: i8,
    pub sample_rate: f64,
    pub fade_frames: f64,
}

pub(super) fn render_osc_phrase(
    osc: &mut Oscillator,
    cfg: &OscPhraseConfig,
    semitones_per_frame: &[f64],
    gains_per_frame: &[f64],
    out_stereo: &mut [f32],
    start_frame: usize,
) {
    let active = semitones_per_frame.len().min(gains_per_frame.len());
    if active == 0 {
        return;
    }
    osc.reset();
    let sub_weight = f64::from(cfg.sub).clamp(0.0, 1.0);
    let attack = (ATTACK_SECONDS * cfg.sample_rate).round().max(1.0);
    let fade = cfg.fade_frames.min(active as f64).max(1.0);
    let fade_start = active.saturating_sub(fade as usize);
    let mut dec = FirDecimator8x::new();
    let lat = FirDecimator8x::LATENCY;

    for frame in 0..(active + lat) {
        let block = if frame < active {
            let ramp = ((frame as f64 + 1.0) / attack).min(1.0);
            let rel = if frame >= fade_start {
                ((active - frame) as f64 / fade).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let env = gains_per_frame[frame] * ramp * rel;
            let hz = note_freq_hz(cfg.octave, semitones_per_frame[frame], cfg.sample_rate);
            std::array::from_fn(|_| {
                osc.tick_8x(cfg.wave, sub_weight, hz, cfg.sample_rate) * env as f32
            })
        } else {
            [0.0; 8]
        };
        let y = dec.decimate(block);
        if frame >= lat {
            let idx = (start_frame + frame - lat) * 2;
            if idx + 1 < out_stereo.len() {
                out_stereo[idx] = y;
                out_stereo[idx + 1] = y;
            }
        }
    }
}

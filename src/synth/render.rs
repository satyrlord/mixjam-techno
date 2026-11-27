//! Step Synth voice rendering: stereo 48-tap Kaiser-sinc sample resampling,
//! standalone 8x PolyBLEP synth phrases, legato 303 amp decay, and accent filter sweeps.
use super::osc::{OscPhraseConfig, Oscillator, render_osc_phrase};
use super::{END_FADE_SECONDS, Note, SourceMode, StepSynthState};
use crate::{
    audio::{Pcm, resample},
    effects::{self, FilterMod},
};

struct Timing {
    sample_rate: f64,
    frames_per_step: f64,
    total_frames: usize,
    fade_frames: f64,
    slide_frames: f64,
    amp_decay: f64,
}

impl Timing {
    fn new(state: &StepSynthState, sample_rate: f64) -> Self {
        let frames_per_step = sample_rate * 60.0 / state.bpm / 4.0;
        let total_frames = (state.sequence_steps() as f64 * frames_per_step).round() as usize;
        let amp_decay = if state.amp_decay_ms > 0.0 {
            let tau = (state.amp_decay_ms / 1000.0 / 3.0).max(0.001);
            (-1.0 / (tau * sample_rate)).exp()
        } else {
            1.0
        };
        Self {
            sample_rate,
            frames_per_step,
            total_frames,
            fade_frames: (END_FADE_SECONDS * sample_rate).round().max(1.0),
            slide_frames: (state.slide_ms / 1000.0 * sample_rate).max(1.0),
            amp_decay,
        }
    }

    #[inline]
    fn frame(&self, steps: f64) -> usize {
        (steps * self.frames_per_step).round() as usize
    }
}

#[inline]
fn interpolate_note(from: (f64, f64), to: Note, t: f64) -> (f64, f64) {
    (
        from.0 + (f64::from(to.note) - from.0) * t,
        from.1 + (to.gain - from.1) * t,
    )
}

fn next_phrase_slice<'a>(notes: &'a [Note], first: &mut usize) -> &'a [Note] {
    let start = *first;
    let mut after = start + 1;
    while after < notes.len() && notes[after].slide_in {
        after += 1;
    }
    *first = after;
    &notes[start..after]
}

fn phrase_pitch_gain(
    phrase: &[Note],
    timing: &Timing,
    semitones: &mut Vec<f64>,
    gains: &mut Vec<f64>,
) -> (usize, usize) {
    let start = timing.frame(phrase[0].start);
    let end = timing
        .frame(phrase[phrase.len() - 1].end)
        .min(timing.total_frames);
    semitones.clear();
    gains.clear();
    if start >= end {
        return (start.min(end), end);
    }
    let mut current = 0;
    let mut slide_from = (f64::from(phrase[0].note), phrase[0].gain);
    let mut env = 1.0_f64;
    for index in start..end {
        while current + 1 < phrase.len() && index >= timing.frame(phrase[current + 1].start) {
            let before = phrase[current];
            let t = ((timing.frame(phrase[current + 1].start) - timing.frame(before.start)) as f64
                / timing.slide_frames)
                .min(1.0);
            slide_from = if current == 0 {
                (f64::from(before.note), before.gain)
            } else {
                interpolate_note(slide_from, before, t)
            };
            current += 1;
        }
        let note = phrase[current];
        let (semi, gain) = if current > 0 {
            let t = ((index - timing.frame(note.start)) as f64 / timing.slide_frames).min(1.0);
            interpolate_note(slide_from, note, t)
        } else {
            (f64::from(note.note), note.gain)
        };
        semitones.push(semi);
        gains.push(gain * env);
        env *= timing.amp_decay;
    }
    (start, end)
}

#[inline]
fn read_source_frame(
    source: &Pcm,
    start_frame: f64,
    start_idx: usize,
    position: f64,
    speed: f64,
    looped: bool,
) -> [f32; 2] {
    let ch = usize::from(source.channels).clamp(1, 2);
    if (speed - 1.0).abs() < 1e-9 && (position - position.round()).abs() < 1e-9 {
        let idx = (position.round() as usize).min(source.frames() as usize - 1) * ch;
        let l = source.samples[idx];
        let r = if ch == 2 { source.samples[idx + 1] } else { l };
        return [l, r];
    }
    let scale = resample::scale(speed);
    if looped {
        resample::read_looped(&source.samples, ch, start_idx, position, scale)
    } else {
        resample::read(
            &source.samples[start_idx * ch..],
            ch,
            position - start_frame,
            scale,
        )
    }
}

struct SampleSlice<'a> {
    start_frame: f64,
    out_channels: usize,
    semitones: &'a [f64],
    gains: &'a [f64],
    start: usize,
}

fn render_sample_phrase(
    source: &Pcm,
    state: &StepSynthState,
    timing: &Timing,
    slice: SampleSlice<'_>,
    out: &mut [f32],
) {
    let source_frames = source.frames() as f64;
    let start_idx = slice.start_frame as usize;
    let loop_frames = source_frames - slice.start_frame;
    let mut position = slice.start_frame;
    let mut written_end = slice.start;
    for (offset, (&semi, &gain)) in slice.semitones.iter().zip(slice.gains).enumerate() {
        if state.loop_source && position >= source_frames {
            position = slice.start_frame + (position - slice.start_frame) % loop_frames;
        }
        if position >= source_frames {
            break;
        }
        let speed = 2.0_f64.powf(semi / 12.0);
        let frame_vals = read_source_frame(
            source,
            slice.start_frame,
            start_idx,
            position,
            speed,
            state.loop_source,
        );
        let frame_idx = slice.start + offset;
        let dst = frame_idx * slice.out_channels;
        out[dst] = frame_vals[0] * gain as f32;
        if slice.out_channels == 2 {
            out[dst + 1] = frame_vals[1] * gain as f32;
        }
        written_end = frame_idx + 1;
        position += speed;
    }
    let fade_start = written_end
        .saturating_sub(timing.fade_frames as usize)
        .max(slice.start);
    for index in fade_start..written_end {
        let w = ((written_end - index) as f64 / timing.fade_frames) as f32;
        let dst = index * slice.out_channels;
        out[dst] *= w;
        if slice.out_channels == 2 {
            out[dst + 1] *= w;
        }
    }
}

struct EnvState {
    filt_env: f64,
    accent_cap: f64,
    decay_normal: f64,
    decay_accent: f64,
    cap_decay: f64,
    amount: f64,
    accent_norm: f64,
    active_end: usize,
    active_accent: bool,
}

impl EnvState {
    fn new(state: &StepSynthState, rate: f64) -> Self {
        let tau_env = (state.filter_env_decay_ms / 1000.0 / 3.5).max(0.005);
        let tau_acc = tau_env.min(0.180 / 3.5);
        Self {
            filt_env: 0.0,
            accent_cap: 0.0,
            decay_normal: (-1.0 / (tau_env * rate)).exp(),
            decay_accent: (-1.0 / (tau_acc * rate)).exp(),
            cap_decay: (-1.0 / (0.28 * rate)).exp(),
            amount: f64::from(state.filter_env_amount),
            accent_norm: (state.accent_db / 12.0).clamp(0.0, 1.0),
            active_end: 0,
            active_accent: false,
        }
    }

    fn trigger_note(&mut self, note: Note, frame: usize, timing: &Timing) {
        self.active_end = timing.frame(note.end).max(frame);
        self.active_accent = note.accent;
        if !note.slide_in {
            self.filt_env = 1.0;
            if note.accent {
                self.accent_cap =
                    (self.accent_cap * 0.55 + 0.45 + 0.55 * self.accent_norm).min(1.35);
            }
        } else if note.accent {
            self.accent_cap = self
                .accent_cap
                .max((self.accent_cap * 0.70 + 0.35 * self.accent_norm).min(1.35));
        }
    }

    fn step_frame(&mut self, frame: usize) -> (f32, f32) {
        let accented = frame < self.active_end && self.active_accent;
        let (d, oct, boost) = if accented {
            (
                self.decay_accent,
                self.amount * (5.5 + 2.5 * self.accent_cap) * self.filt_env,
                0.12 * self.accent_norm * self.filt_env,
            )
        } else {
            (self.decay_normal, self.amount * 5.5 * self.filt_env, 0.0)
        };
        self.filt_env *= d;
        if !accented {
            self.accent_cap *= self.cap_decay;
        }
        (oct as f32, boost as f32)
    }

    fn run_pass(
        &mut self,
        notes: &[Note],
        timing: &Timing,
        record: bool,
    ) -> Option<(Vec<f32>, Vec<f32>)> {
        let n = timing.total_frames;
        let mut octs = if record { vec![0.0; n] } else { Vec::new() };
        let mut boosts = if record { vec![0.0; n] } else { Vec::new() };
        let mut next_idx = 0;
        self.active_end = 0;
        self.active_accent = false;

        for frame in 0..n {
            while next_idx < notes.len() && frame == timing.frame(notes[next_idx].start) {
                self.trigger_note(notes[next_idx], frame, timing);
                next_idx += 1;
            }
            let (oct, boost) = self.step_frame(frame);
            if record {
                octs[frame] = oct;
                boosts[frame] = boost;
            }
        }
        record.then_some((octs, boosts))
    }
}

fn build_filter_mod(
    state: &StepSynthState,
    notes: &[Note],
    timing: &Timing,
) -> Option<(Vec<f32>, Vec<f32>)> {
    if state.filter_env_amount.abs() <= 1e-6
        || !state.effects.filter.enabled
        || state.effects.filter.wet <= 0.0
        || timing.total_frames == 0
    {
        return None;
    }
    let mut env = EnvState::new(state, timing.sample_rate);
    env.run_pass(notes, timing, false);
    env.run_pass(notes, timing, true)
}

fn render_synth_voices(state: &StepSynthState, notes: &[Note], timing: &Timing) -> Vec<f32> {
    let mut samples = vec![0.0_f32; timing.total_frames * 2];
    let mut semitones = Vec::new();
    let mut gains = Vec::new();
    let mut osc = Oscillator::new();
    let cfg = OscPhraseConfig {
        wave: state.osc_waveform,
        sub: state.osc_sub,
        octave: state.osc_octave,
        sample_rate: timing.sample_rate,
        fade_frames: timing.fade_frames,
    };
    let mut first = 0;
    while first < notes.len() {
        let phrase = next_phrase_slice(notes, &mut first);
        let (start, _) = phrase_pitch_gain(phrase, timing, &mut semitones, &mut gains);
        render_osc_phrase(&mut osc, &cfg, &semitones, &gains, &mut samples, start);
    }
    samples
}

fn render_sample_voices(
    src: &Pcm,
    state: &StepSynthState,
    notes: &[Note],
    timing: &Timing,
    out_channels: usize,
) -> Result<Vec<f32>, String> {
    let start_frame = (state.start_ms / 1000.0 * timing.sample_rate).round();
    if start_frame >= src.frames() as f64 {
        return Err(format!(
            "The source start, {:.0} ms, is past the end of the source.",
            state.start_ms
        ));
    }
    resample::prepare();
    let mut samples = vec![0.0_f32; timing.total_frames * out_channels];
    let mut semitones = Vec::new();
    let mut gains = Vec::new();
    let mut first = 0;
    while first < notes.len() {
        let phrase = next_phrase_slice(notes, &mut first);
        let (start, _) = phrase_pitch_gain(phrase, timing, &mut semitones, &mut gains);
        render_sample_phrase(
            src,
            state,
            timing,
            SampleSlice {
                start_frame,
                out_channels,
                semitones: &semitones,
                gains: &gains,
                start,
            },
            &mut samples,
        );
    }
    Ok(samples)
}

pub(super) fn render_measured(
    source: Option<&Pcm>,
    state: &StepSynthState,
    sample_rate: u32,
) -> Result<(Pcm, f64), String> {
    state.validate()?;
    let timing = Timing::new(state, f64::from(sample_rate));
    let notes = state.notes();
    let (out_channels, samples) = if state.source_mode == SourceMode::Synth {
        (2, render_synth_voices(state, &notes, &timing))
    } else {
        let src = source.ok_or("Choose a nonempty sample.")?;
        let ch = usize::from(src.channels.clamp(1, 2));
        (ch, render_sample_voices(src, state, &notes, &timing, ch)?)
    };

    let mod_bufs = build_filter_mod(state, &notes, &timing);
    let filter_mod = mod_bufs.as_ref().map(|(oct, res)| FilterMod {
        cutoff_octaves: oct,
        res_boost: res,
    });
    let (pcm, gain) = effects::process_synth_voiced(
        &Pcm {
            sample_rate,
            channels: out_channels as u16,
            samples,
        },
        &state.effects,
        state.bpm,
        filter_mod,
    )?;
    Ok((pcm, -20.0 * f64::from(gain).log10()))
}

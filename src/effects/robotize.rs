//! 16x FIR oversampled Ring Modulator and SSB Hilbert Frequency Shifter.
use super::{Robotize, RobotizeMode, run_oversampled_16x};
use std::f64::consts::TAU;

const HILBERT_I: [f64; 4] = [
    0.692_387_8,
    0.936_065_432_295_9,
    0.988_229_522_686_0,
    0.998_748_845_273_7,
];
const HILBERT_Q: [f64; 4] = [
    0.402_192_116_242_6,
    0.856_171_088_242_0,
    0.972_290_954_565_1,
    0.995_288_479_127_8,
];

#[derive(Clone, Copy)]
struct ApBank {
    x1: [f64; 4],
    x2: [f64; 4],
    y1: [f64; 4],
    y2: [f64; 4],
}
impl ApBank {
    const ZERO: Self = Self {
        x1: [0.0; 4],
        x2: [0.0; 4],
        y1: [0.0; 4],
        y2: [0.0; 4],
    };

    #[inline]
    fn tick(&mut self, mut x: f64, coeffs: &[f64; 4]) -> f64 {
        for (i, &a) in coeffs.iter().enumerate() {
            let c = a * a;
            let y = c * (x + self.y2[i]) - self.x2[i];
            self.x2[i] = self.x1[i];
            self.x1[i] = x;
            self.y2[i] = self.y1[i];
            self.y1[i] = if y.abs() < 1e-30 { 0.0 } else { y };
            x = y;
        }
        x
    }
}

/// Quantize Robotize carrier to whole cycles per loop when a cycle fits below Nyquist limit.
pub(super) fn robotize_frequency(hz: f64, sample_rate: f64, loop_frames: Option<usize>) -> f64 {
    const RATE_FRACTION: f64 = 0.45;
    let frequency = hz.min(sample_rate * RATE_FRACTION);
    let Some(frames) = loop_frames else {
        return frequency;
    };
    let max_cycles = (frames as f64 * RATE_FRACTION).floor();
    if max_cycles < 1.0 {
        return frequency;
    }
    let cycles = (frequency * frames as f64 / sample_rate)
        .round()
        .clamp(1.0, max_cycles);
    cycles * sample_rate / frames as f64
}

pub(super) fn apply(channel: &mut [f64], robotize: &Robotize, rate: f64, looped: bool) {
    if !robotize.enabled || robotize.wet <= 0.0 || channel.is_empty() {
        return;
    }
    let freq = robotize_frequency(
        f64::from(robotize.frequency_hz),
        rate,
        looped.then_some(channel.len()),
    );
    let wet = f64::from(robotize.wet).clamp(0.0, 1.0);
    let dry = channel.to_vec();
    match robotize.mode {
        RobotizeMode::RingMod => modulate_carrier(channel, freq, rate, looped, 0.0),
        RobotizeMode::FreqShift => run_freq_shift(channel, freq, rate, looped),
    }
    for (out, &d) in channel.iter_mut().zip(&dry) {
        *out = d + (*out - d) * wet;
    }
}

fn modulate_carrier(samples: &mut [f64], freq: f64, rate: f64, looped: bool, mut phase: f64) {
    let os_rate = rate * 16.0;
    let step = TAU * freq / os_rate;
    run_oversampled_16x(samples, looped, |x| {
        let out = x * phase.sin();
        phase = (phase + step).rem_euclid(TAU);
        out
    });
}

fn run_freq_shift(samples: &mut [f64], freq: f64, rate: f64, looped: bool) {
    // These quadrature coefficients operate at the source rate. In loop
    // mode, let the slowest all-pass pole settle on the repeated source.
    let mut bank_i = ApBank::ZERO;
    let mut bank_q = ApBank::ZERO;
    let mut prev_i = 0.0_f64;
    if looped {
        let warm = (1e-9_f64.ln() / HILBERT_I[3].ln()).ceil() as usize;
        let start = samples.len() - warm % samples.len();
        for &x in samples
            .iter()
            .cycle()
            .skip(start % samples.len())
            .take(warm)
        {
            prev_i = bank_i.tick(x, &HILBERT_I);
            bank_q.tick(x, &HILBERT_Q);
        }
    }
    let mut quadrature = Vec::with_capacity(samples.len());
    for sample in samples.iter_mut() {
        let x = *sample;
        *sample = prev_i;
        prev_i = bank_i.tick(x, &HILBERT_I);
        quadrature.push(bank_q.tick(x, &HILBERT_Q));
    }

    // Shift above the source rate so the decimator removes sidebands
    // that cross Nyquist instead of folding them back into the output.
    modulate_carrier(samples, freq, rate, looped, TAU * 0.25);
    modulate_carrier(&mut quadrature, freq, rate, looped, 0.0);
    for (sample, q) in samples.iter_mut().zip(quadrature) {
        *sample += q;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifted_sideband_above_nyquist_is_removed() {
        let rate = 44_100.0;
        let mut tone: Vec<_> = (0..4410)
            .map(|i| 0.5 * (TAU * 14_000.0 * i as f64 / rate).sin())
            .collect();
        run_freq_shift(&mut tone, 12_000.0, rate, false);
        let peak = tone[1000..4000].iter().fold(0.0_f64, |p, x| p.max(x.abs()));
        assert!(peak < 0.01, "{peak}");
    }

    #[test]
    fn wrapped_frequency_shift_starts_in_the_settled_hilbert_state() {
        let mut period = vec![0.25; 256];
        let mut repeated = period.repeat(4);
        run_freq_shift(&mut period, 375.0, 48_000.0, true);
        run_freq_shift(&mut repeated, 375.0, 48_000.0, true);
        for (&short, &long) in period.iter().zip(&repeated[768..]) {
            assert!((short - long).abs() < 1e-5, "{short} {long}");
        }
    }
}

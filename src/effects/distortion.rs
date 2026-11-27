//! 16x FIR + ADAA2 Distortion (`Warm`, `Acid`, `Fuzz`, `Crush`) and clean
//! Effect Studio output peak ceiling.
use super::{DistortionShape, EffectsState, run_oversampled_16x};
use crate::audio::resample;
use crate::fx::DistortionShape as FxShape;
use crate::fx::adaa::{Adaa2State, crusher_levels, curve_only, quantize};

const CEILING_KNEE: f64 = 0.78;
const CEILING_PEAK: f64 = 0.985;

fn shape_drive(shape: DistortionShape, dirty: f64) -> (FxShape, f64, f64, f64, f64) {
    let (fx_shape, g, asym) = match shape {
        DistortionShape::Warm => (FxShape::Tube, 1.0 + 24.0 * dirty, 0.0),
        DistortionShape::Acid => (FxShape::Overdrive, 1.0 + 26.0 * dirty, 0.28 * dirty),
        DistortionShape::Fuzz => (FxShape::Fuzz, 1.0 + 8.5 * dirty, 0.0),
        DistortionShape::Crush => return (FxShape::HardClip, 1.0, 0.0, 0.0, 1.0),
    };
    let dc = if asym != 0.0 {
        curve_only(fx_shape, asym)
    } else {
        0.0
    };
    let peak = (curve_only(fx_shape, g + asym) - dc)
        .abs()
        .max((curve_only(fx_shape, -g + asym) - dc).abs())
        .max(0.5);
    // Keep 0.75 peak ceiling so bandlimited harmonics stay clear of the 1x output knee.
    (fx_shape, g, asym, dc, 0.75 / peak)
}

#[inline]
fn soft_ceiling(scaled: f64) -> f64 {
    const SPAN: f64 = CEILING_PEAK - CEILING_KNEE;
    let abs = scaled.abs();
    if abs > CEILING_KNEE {
        scaled.signum() * (CEILING_KNEE + SPAN * ((abs - CEILING_KNEE) / SPAN).tanh())
    } else {
        scaled
    }
}

/// Keep the distortion's unity-gain transfer and fit extra gain smoothly
/// into the remaining headroom. Clipping the boosted waveshaper again can
/// generate harmonics above even the 16x Nyquist frequency.
#[inline]
fn boosted_drive(shaped: f64, volume: f64) -> f64 {
    if volume <= 1.0 {
        return soft_ceiling(shaped * volume);
    }
    let base = soft_ceiling(shaped);
    let room = CEILING_PEAK - base.abs();
    let boost = shaped * (volume - 1.0);
    base + room * boost / (room * room + boost * boost).sqrt()
}

fn scale_with_ceiling(channel: &mut [f64], volume: f64, ceiling: f64, looped: bool) {
    let peak = channel.iter().fold(0.0_f64, |p, x| p.max(x.abs())) * volume.abs();
    if ceiling <= 1.01 && peak > CEILING_KNEE {
        run_oversampled_16x(channel, looped, |x| soft_ceiling(x * volume));
    } else {
        for sample in channel.iter_mut() {
            *sample = (*sample * volume).clamp(-ceiling, ceiling);
        }
    }
}

fn crush_levels(dirty: f64) -> f64 {
    crusher_levels((14.0 - 11.0 * dirty).round() as u8)
}

/// Supply the sinc taps with the actual signal on each side of the cut.
fn padded_signal(samples: &[f64], pad: usize, looped: bool) -> Vec<f32> {
    let mut padded = vec![0.0; samples.len() + 2 * pad];
    for (dst, &src) in padded[pad..].iter_mut().zip(samples) {
        *dst = src as f32;
    }
    if looped {
        for i in 0..pad {
            padded[pad - 1 - i] = samples[samples.len() - 1 - i % samples.len()] as f32;
            padded[pad + samples.len() + i] = samples[i % samples.len()] as f32;
        }
    }
    padded
}

fn apply_crush(channel: &mut [f64], dirty: f64, looped: bool) {
    let levels = crush_levels(dirty);
    let hold = 1 + (dirty * 10.0).floor() as usize;
    if hold == 1 {
        run_oversampled_16x(channel, looped, |x| quantize(x, levels, 0.0));
        return;
    }
    // Fit a whole number of reduced-clock samples in a repeating pattern.
    let count = if looped {
        (channel.len() + hold / 2) / hold
    } else {
        channel.len().div_ceil(hold)
    }
    .max(1);
    let stride = if looped {
        channel.len() as f64 / count as f64
    } else {
        hold as f64
    };
    let pad = (resample::HALF_WIDTH as f64 * stride).ceil() as usize + 1;
    let mut reduced: Vec<f64> = {
        let source = padded_signal(channel, pad, looped);
        let scale = resample::scale(stride);
        (0..count)
            .map(|i| {
                let position = pad as f64 + i as f64 * stride;
                f64::from(resample::read(&source, 1, position, scale)[0])
            })
            .collect()
    };
    run_oversampled_16x(&mut reduced, looped, |x| quantize(x, levels, 0.0));
    let pad = resample::HALF_WIDTH + 1;
    let source = padded_signal(&reduced, pad, looped);
    for (i, sample) in channel.iter_mut().enumerate() {
        let position = pad as f64 + i as f64 / stride;
        *sample = f64::from(resample::read(&source, 1, position, 1.0)[0]);
    }
}

pub(super) fn apply_and_scale(
    channel: &mut [f64],
    shape: DistortionShape,
    dirty: f32,
    volume: f64,
    ceiling: f64,
    looped: bool,
) {
    if channel.is_empty() {
        return;
    }
    if dirty > 0.0 {
        let dirty_d = f64::from(dirty).clamp(0.0, 1.0);
        if shape != DistortionShape::Crush {
            let (fx_shape, gain, asym, dc, norm) = shape_drive(shape, dirty_d);
            let limit = ceiling <= 1.01;
            let mut adaa = Adaa2State::default();
            adaa.rest_at(fx_shape, asym);
            run_oversampled_16x(channel, looped, |x| {
                let y = (adaa.process(fx_shape, x * gain + asym) - dc) * norm;
                if limit {
                    boosted_drive(y, volume)
                } else {
                    y * volume
                }
            });
            return;
        }
        apply_crush(channel, dirty_d, looped);
    }
    scale_with_ceiling(channel, volume, ceiling, looped);
}

/// Sample the normalized static waveshaper transfer curve over `[-1, 1]` for UI scopes.
pub fn sample_drive_curve(state: &EffectsState, points: usize) -> Vec<f32> {
    if points == 0 {
        return Vec::new();
    }
    let denom = (points.saturating_sub(1)).max(1) as f64;
    let dirty = f64::from(state.dirty).clamp(0.0, 1.0);
    let volume = f64::from(state.volume).clamp(0.0, 2.0);
    let bypassed = state.is_bypassed();
    let (fx_shape, gain, asym, dc, norm) = shape_drive(state.dist_shape, dirty);
    let levels = crush_levels(dirty);
    (0..points)
        .map(|i| {
            let x = -1.0 + 2.0 * (i as f64 / denom);
            let shaped = if dirty <= 0.0 {
                x
            } else if state.dist_shape == DistortionShape::Crush {
                quantize(x, levels, 0.0)
            } else {
                (curve_only(fx_shape, x * gain + asym) - dc) * norm
            };
            if bypassed {
                x as f32
            } else if dirty > 0.0 && state.dist_shape != DistortionShape::Crush {
                boosted_drive(shaped, volume) as f32
            } else {
                soft_ceiling(shaped * volume) as f32
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_drive_keeps_total_folded_energy_below_minus_90_db() {
        let period = 11_025;
        let phase = std::f64::consts::TAU * 4048.0 / 44_100.0;
        let mut tone: Vec<_> = (0..3 * period)
            .map(|i| 0.707_946 * (phase * i as f64).sin())
            .collect();
        apply_and_scale(&mut tone, DistortionShape::Warm, 1.0, 2.0, 1.0, false);
        // In one exact period, DC and the five harmonics below Nyquist
        // account for all legitimate output energy.
        let samples = &tone[period..2 * period];
        let mean = |sum: f64| sum / period as f64;
        let mut permitted = mean(samples.iter().sum()).powi(2);
        for harmonic in 1..=5 {
            let mut real = 0.0;
            let mut imag = 0.0;
            for (i, x) in samples.iter().enumerate() {
                let (sin, cos) = (phase * f64::from(harmonic) * i as f64).sin_cos();
                real += x * cos;
                imag += x * sin;
            }
            permitted += 2.0 * (mean(real).powi(2) + mean(imag).powi(2));
        }
        let power = mean(samples.iter().map(|x| x * x).sum());
        let alias_db = 10.0 * (2.0 * (power - permitted)).max(1e-30).log10();
        assert!(alias_db < -90.0, "{alias_db} dB");
    }

    #[test]
    fn crush_suppresses_out_of_band_tones_and_preserves_low_notes() {
        let rate = 44_100.0;
        let render = |hz: f64| {
            let mut audio: Vec<f64> = (0..44_100)
                .map(|i| 0.5 * (std::f64::consts::TAU * hz * f64::from(i) / rate).sin())
                .collect();
            apply_crush(&mut audio, 1.0, true);
            audio
        };
        let high = render(4048.0);
        let high_peak = high.iter().fold(0.0_f64, |p, x| p.max(x.abs()));
        assert!(high_peak < 10.0_f64.powf(-90.0 / 20.0), "{high_peak}");
        let bass = render(220.0);
        let magnitude = |hz: f64| {
            let sum: f64 = bass
                .iter()
                .enumerate()
                .map(|(i, x)| x * (std::f64::consts::TAU * hz * i as f64 / rate).sin())
                .sum();
            2.0 * sum.abs() / bass.len() as f64
        };
        assert!((0.45..0.55).contains(&magnitude(220.0)));
        assert!(magnitude(660.0) > 0.005, "quantization adds harmonics");
    }

    #[test]
    fn raising_clean_output_level_never_reverses_the_transfer_curve() {
        let rendered_level = |input: f64| {
            let mut dc = vec![input; 512];
            scale_with_ceiling(&mut dc, 1.0, 1.0, true);
            dc[256]
        };
        assert!(rendered_level(0.986) > rendered_level(0.984));
        let bypass = sample_drive_curve(&EffectsState::default(), 1025);
        assert_eq!(bypass.last(), Some(&1.0));
        assert!(bypass.windows(2).all(|w| w[0] < w[1]));
    }
}

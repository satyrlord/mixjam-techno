//! Band-limited reads of a decoded source at a fractional position.
//!
//! A clip that plays off its source's tempo or sample rate, or under a Pitch
//! lane, reads its source between frames. `read` reconstructs the source
//! there with a Kaiser-windowed sinc kernel: 48 taps at speeds up to 1. At a
//! speed above 1 the kernel's cutoff falls by the speed, and its span widens
//! by the same factor, to attenuate content above the new Nyquist frequency
//! before it can fold back. The finite transition band can leave audible
//! images of near-Nyquist content when a source slows or has a low sample
//! rate. The kernel is symmetric and centred on the read, so it adds no
//! delay: a frame reads the same position as the straight-line read did, and
//! a clip starting at frame N still sounds at N.
//!
//! The kernel lives in one table, `DENSITY` points per source frame from
//! its centre to its edge, with the slope to the next point beside each, so
//! a tap is a straight-line read between two points. `prepare` builds the
//! table; the mix calls it before playback reaches the audio thread.
//!
//! A clip reads only its slice of the source, from its offset on, as the
//! straight-line read did. Before the slice's first frame the read holds
//! that frame, and after the source's last frame it holds that one, as the
//! straight-line read held its last frame. A clip then starts at its
//! sound's level, whatever came before its offset, and its end and fades
//! keep their frames.
use std::sync::LazyLock;

#[cfg(test)]
static TABLE_INITIALIZATIONS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Zero crossings on each side of the read at speeds up to 1, and so half
/// the kernel's taps.
pub(crate) const HALF_WIDTH: usize = 24;
/// Table points per source frame.
const DENSITY: usize = 128;
/// The kernel's cutoff, in cycles per source frame, at speeds up to 1. The
/// measured pass band is flat to about 19.5 kHz at 44.1 kHz. Rejection near
/// the source's Nyquist frequency depends on the finite transition band.
const CUTOFF: f64 = 0.49;
/// The Kaiser window's shape: about 85 dB of rejection in the stop band.
const BETA: f64 = 8.5;
/// The last table point: the kernel's edge.
const EDGE: usize = HALF_WIDTH * DENSITY;

/// Each point's kernel value and the step to the next point. The point past
/// the edge is zero.
static TABLE: LazyLock<Box<[[f32; 2]]>> = LazyLock::new(|| {
    #[cfg(test)]
    TABLE_INITIALIZATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let value = |point: usize| {
        if point > EDGE {
            return 0.0;
        }
        let distance = point as f64 / DENSITY as f64;
        2.0 * CUTOFF * sinc(2.0 * CUTOFF * distance) * kaiser(distance)
    };
    (0..=EDGE)
        .map(|point| {
            let here = value(point);
            [here as f32, (value(point + 1) - here) as f32]
        })
        .collect()
});

/// Build the table, off the audio thread.
pub(crate) fn prepare() {
    LazyLock::force(&TABLE);
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
    }
}

fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    loop {
        term *= (x / (2.0 * k)).powi(2);
        sum += term;
        if term < sum * 1e-17 {
            return sum;
        }
        k += 1.0;
    }
}

/// The Kaiser window `distance` source frames from the kernel's centre.
fn kaiser(distance: f64) -> f64 {
    let ratio = distance / HALF_WIDTH as f64;
    bessel_i0(BETA * (1.0 - ratio * ratio).max(0.0).sqrt()) / bessel_i0(BETA)
}

/// The kernel's scale for a read that moves `speed` source frames for each
/// output frame: 1 up to a speed of 1, then the speed's inverse, so that the
/// cutoff follows the output's Nyquist frequency.
pub(crate) fn scale(speed: f64) -> f32 {
    if speed > 1.0 {
        (1.0 / speed) as f32
    } else {
        1.0
    }
}

/// Whether the kernel's tap spacing can represent this speed. Mix
/// preparation checks the fastest Pitch value before an audio read.
pub(crate) fn supports_speed(speed: f64) -> bool {
    speed.is_finite() && speed > 0.0 && quantized_scale(scale(speed)) > 0.0
}

/// Use the tap spacing for both the kernel and its gain. Otherwise a
/// rounded spacing changes the gain at very high source speeds.
fn quantized_scale(scale: f32) -> f32 {
    let units = (DENSITY * (1 << POINT_BITS)) as f64;
    ((f64::from(scale) * units).round() / units) as f32
}

/// The source's value at `position`, in source frames, for each of its
/// channels (the second is zero for a mono source), with the kernel at
/// `scale`. The source holds its first frame before its start and its last
/// after its end, as the straight-line read holds its last frame, so a
/// clip that starts at a sound's first frame starts at full level, and the
/// read never reaches past either end.
#[inline]
pub(crate) fn read(samples: &[f32], channels: usize, position: f64, scale: f32) -> [f32; 2] {
    read_inner(samples, channels, None, position, scale)
}

/// Read `samples` with modular wrap-around inside `[loop_start .. frames)`,
/// keeping Kaiser-sinc reconstruction continuous across loop seams.
#[inline]
pub(crate) fn read_looped(
    samples: &[f32],
    channels: usize,
    loop_start: usize,
    position: f64,
    scale: f32,
) -> [f32; 2] {
    read_inner(samples, channels, Some(loop_start), position, scale)
}

#[inline]
fn read_inner(
    samples: &[f32],
    channels: usize,
    loop_start: Option<usize>,
    position: f64,
    scale: f32,
) -> [f32; 2] {
    if samples.is_empty() {
        return [0.0; 2];
    }
    let scale = quantized_scale(scale);
    let sums = if channels == 1 {
        taps::<1>(samples, position, scale, loop_start)
    } else {
        taps::<2>(samples, position, scale, loop_start)
    };
    [sums[0] * scale, sums[1] * scale]
}

/// The bits of a table position below its point.
const POINT_BITS: u32 = 16;

struct KernelPlan {
    below: i64,
    before: i64,
    after: i64,
    earlier: usize,
    later: usize,
    step: i64,
}

#[inline]
fn tap_count(at: i64, edge: i64, step: i64, per_step: f64) -> usize {
    if at > edge {
        return 0;
    }
    let mut count = ((edge - at) as f64 * per_step) as i64 + 1;
    if at + count * step <= edge {
        count += 1;
    } else if at + (count - 1) * step > edge {
        count -= 1;
    }
    count as usize
}

#[inline]
fn plan_kernel(position: f64, scale: f32) -> KernelPlan {
    let unit = f64::from(1_u32 << POINT_BITS);
    let density = f64::from(scale) * DENSITY as f64 * unit;
    let step = density.round() as i64;
    let edge = (EDGE as i64) << POINT_BITS;
    let below_f = position.floor();
    let fraction = position - below_f;
    let before = (fraction * density).round() as i64;
    let after = ((1.0 - fraction) * density).round() as i64;
    let per_step = 1.0 / step as f64;
    KernelPlan {
        below: below_f as i64,
        before,
        after,
        earlier: tap_count(before, edge, step, per_step),
        later: tap_count(after, edge, step, per_step),
        step,
    }
}

#[inline]
fn tap_weight(table: &[[f32; 2]], at: i64) -> f32 {
    let [value, slope] = table[(at >> POINT_BITS) as usize];
    let fraction = (at & ((1 << POINT_BITS) - 1)) as f32;
    value + slope * (fraction * (1.0 / (1 << POINT_BITS) as f32))
}

#[inline]
fn boundary_frame<const CHANNELS: usize>(
    samples: &[f32],
    frames: i64,
    frame: i64,
    loop_start: Option<usize>,
) -> [f32; CHANNELS] {
    let idx = match loop_start {
        Some(ls) => {
            let ls = ls.min(frames as usize - 1) as i64;
            let len = (frames - ls).max(1);
            ((frame - ls).rem_euclid(len) + ls) as usize
        }
        None => frame.clamp(0, frames - 1) as usize,
    };
    let start = idx * CHANNELS;
    let mut values = [0.0; CHANNELS];
    values.copy_from_slice(&samples[start..start + CHANNELS]);
    values
}

#[inline]
fn add_frame<const CHANNELS: usize>(sum: &mut [f64; 2], frame: &[f32; CHANNELS], weight: f32) {
    for (sum, sample) in sum.iter_mut().zip(frame) {
        *sum += f64::from(sample * weight);
    }
}

#[inline]
fn taps_edge<const CHANNELS: usize>(
    samples: &[f32],
    frames: i64,
    plan: &KernelPlan,
    loop_start: Option<usize>,
    table: &[[f32; 2]],
) -> [f32; 2] {
    let mut sums = [0.0_f64; 2];
    for (dir, base, count, mut at) in [
        (-1_i64, plan.below, plan.earlier, plan.before),
        (1_i64, plan.below + 1, plan.later, plan.after),
    ] {
        for tap in 0..count as i64 {
            let f = boundary_frame::<CHANNELS>(samples, frames, base + dir * tap, loop_start);
            add_frame(&mut sums, &f, tap_weight(table, at));
            at += plan.step;
        }
    }
    [sums[0] as f32, sums[1] as f32]
}

/// The kernel's sum over the taps around `position`, before its scale.
#[inline]
fn taps<const CHANNELS: usize>(
    samples: &[f32],
    position: f64,
    scale: f32,
    loop_start: Option<usize>,
) -> [f32; 2] {
    let table = &**TABLE;
    let frames = (samples.len() / CHANNELS) as i64;
    let plan = plan_kernel(position, scale);
    let (first, last) = (
        plan.below + 1 - plan.earlier as i64,
        plan.below + plan.later as i64,
    );
    let lower_bound = loop_start.unwrap_or(0) as i64;
    if first < lower_bound || last >= frames {
        return taps_edge::<CHANNELS>(samples, frames, &plan, loop_start, table);
    }
    let mut sums = [[0.0_f64; 2]; 2];
    let (mut at_before, mut at_after) = (plan.before, plan.after);
    let (head, tail) = samples[first as usize * CHANNELS..(last as usize + 1) * CHANNELS]
        .split_at(plan.earlier * CHANNELS);
    let mut head = head.as_chunks::<CHANNELS>().0.iter().rev();
    let mut tail = tail.as_chunks::<CHANNELS>().0.iter();
    for (back, ahead) in head
        .by_ref()
        .zip(tail.by_ref())
        .take(plan.earlier.min(plan.later))
    {
        add_frame(&mut sums[0], back, tap_weight(table, at_before));
        add_frame(&mut sums[1], ahead, tap_weight(table, at_after));
        at_before += plan.step;
        at_after += plan.step;
    }
    for back in head {
        add_frame(&mut sums[0], back, tap_weight(table, at_before));
        at_before += plan.step;
    }
    for ahead in tail {
        add_frame(&mut sums[1], ahead, tap_weight(table, at_after));
        at_after += plan.step;
    }
    [
        (sums[0][0] + sums[1][0]) as f32,
        (sums[0][1] + sums[1][1]) as f32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Read `source` (mono) from `start` at `step` source frames per frame.
    fn run(source: &[f32], start: f64, step: f64, frames: usize) -> Vec<f64> {
        (0..frames)
            .map(|frame| f64::from(read(source, 1, start + frame as f64 * step, scale(step))[0]))
            .collect()
    }

    fn tone(cycles_per_frame: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|frame| (0.5 * (2.0 * PI * cycles_per_frame * frame as f64 + 0.3).sin()) as f32)
            .collect()
    }

    /// The amplitude of the sine at `cycles_per_frame` that best fits `out`
    /// by least squares, and the RMS level of what is left, as a sine's
    /// peak, both in dB relative to `reference`.
    fn fit(out: &[f64], cycles_per_frame: f64, reference: f64) -> (f64, f64) {
        let (mut cc, mut ss, mut cs, mut yc, mut ys) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let basis = |n: usize| {
            let angle = 2.0 * PI * cycles_per_frame * n as f64;
            (angle.cos(), angle.sin())
        };
        for (n, y) in out.iter().enumerate() {
            let (c, s) = basis(n);
            cc += c * c;
            ss += s * s;
            cs += c * s;
            yc += y * c;
            ys += y * s;
        }
        let det = cc * ss - cs * cs;
        let a = (yc * ss - ys * cs) / det;
        let b = (ys * cc - yc * cs) / det;
        let power: f64 = out
            .iter()
            .enumerate()
            .map(|(n, y)| {
                let (c, s) = basis(n);
                (y - a * c - b * s).powi(2)
            })
            .sum::<f64>()
            / out.len() as f64;
        let db = |value: f64| 20.0 * (value / reference).max(1e-12).log10();
        (db(a.hypot(b)), db((2.0 * power).sqrt()))
    }

    /// The typical reads: 172 to 180 BPM at 44.1 kHz, a 48 kHz source at its
    /// own tempo, a 180 BPM loop at 165 BPM, and the same 48 kHz source
    /// 4.7% fast.
    const STEPS: [(f64, f64); 4] = [
        (1.047, 44_100.0),
        (48_000.0 / 44_100.0, 48_000.0),
        (0.9, 44_100.0),
        (48_000.0 / 44_100.0 * 1.047, 48_000.0),
    ];

    #[test]
    fn the_pass_band_is_flat_and_the_images_and_aliases_stay_80_db_down() {
        let mut worst = f64::MAX;
        for (step, rate) in STEPS {
            // A stepped sweep up to 19 kHz at the output for these ratios.
            for hertz in [
                40.0, 400.0, 1_000.0, 3_000.0, 6_000.0, 10_000.0, 14_000.0, 17_000.0, 19_000.0,
            ] {
                let source_hertz = hertz / step * rate / 44_100.0;
                let cycles = source_hertz / rate;
                let source = tone(cycles, 20_000);
                let out = run(&source, 2_000.0, step, 12_000);
                let (gain, rest) = fit(&out, cycles * step, 0.5);
                // The kernel's own frequency at 44.1 kHz: the output's above
                // a speed of 1, the source's below it, where a slowed sound
                // keeps the top that its source had. Flat within 0.01 dB to
                // 17 kHz; at 19 kHz the edge loses under 0.5 dB.
                let kernel = if step > 1.0 {
                    hertz
                } else {
                    source_hertz * 44_100.0 / rate
                };
                if kernel <= 19_000.0 {
                    let tolerance = if kernel <= 17_000.0 { 0.01 } else { 0.5 };
                    assert!(
                        gain.abs() < tolerance,
                        "step {step}: {hertz} Hz at {gain} dB"
                    );
                }
                // These tones within 20 kHz of the kernel's own rate leave
                // images and aliases at least 80 dB down. Content above
                // that lies in the transition band and is not covered.
                if kernel <= 20_000.0 {
                    assert!(rest < -80.0, "step {step}: {hertz} Hz leaves {rest} dB");
                    worst = worst.min(-rest);
                }
            }
        }
        println!("the worst image or alias is {worst:.1} dB down");
    }

    #[test]
    fn a_representable_fast_read_keeps_dc_with_rounded_tap_spacing() {
        for speed in [20_000.0, 20_000.1] {
            assert!(supports_speed(speed));
            for position in [0.0, 20.4] {
                let value = read(&[0.5; 64], 1, position, scale(speed))[0];
                assert!((value - 0.5).abs() < 0.5e-4, "{speed}: {value}");
            }
        }
    }

    #[test]
    fn a_fast_read_removes_what_would_fold_into_the_audible_band() {
        // At these speeds each tone lies above the output's Nyquist frequency
        // and would fold to below 20 kHz. The straight-line read leaves them.
        for (step, hertz) in [(1.5, 17_000.0), (2.0, 13_000.0), (1.25, 19_500.0)] {
            let cycles = hertz / 44_100.0;
            let source = tone(cycles, 40_000);
            let out = run(&source, 2_000.0, step, 12_000);
            let alias = 1.0 - (cycles * step).fract();
            assert!(alias * 44_100.0 < 20_000.0);
            let (left, _) = fit(&out, alias, 0.5);
            assert!(left < -80.0, "step {step}: {hertz} Hz leaves {left} dB");
            let linear: Vec<f64> = (0..12_000)
                .map(|frame| {
                    let position = 2_000.0 + frame as f64 * step;
                    let index = position.floor() as usize;
                    let fraction = position - index as f64;
                    let (a, b) = (f64::from(source[index]), f64::from(source[index + 1]));
                    a + (b - a) * fraction
                })
                .collect();
            let (left, _) = fit(&linear, alias, 0.5);
            assert!(left > -40.0, "the straight-line read leaves {left} dB");
        }
    }

    #[test]
    fn dc_passes_and_an_impulse_lands_where_the_straight_line_read_puts_it() {
        for (step, _) in STEPS {
            // Within 80 dB at every phase of the read.
            let out = run(&[0.5; 4_000], 1_000.0, step, 1_000);
            let worst = out
                .iter()
                .map(|value| (value - 0.5).abs())
                .fold(0.0, f64::max);
            assert!(worst < 0.5e-4, "step {step}: {worst}");
        }
        // An impulse at frame 600 read from frame 100.25: its centre comes
        // at the output frame where the read passes 600, and its peak at the
        // frame where the straight-line read puts it.
        for step in [1.047, 48_000.0 / 44_100.0, 0.9, 0.37, 1.5] {
            let mut source = vec![0.0; 4_000];
            source[600] = 1.0;
            let start = 100.25;
            let out = run(&source, start, step, 1_500);
            let linear: Vec<f64> = (0..1_500)
                .map(|frame| {
                    let position = start + frame as f64 * step;
                    let index = position.floor() as usize;
                    let fraction = position - index as f64;
                    let (a, b) = (f64::from(source[index]), f64::from(source[index + 1]));
                    a + (b - a) * fraction
                })
                .collect();
            let centre = |values: &[f64]| {
                let weight: f64 = values.iter().sum();
                values
                    .iter()
                    .enumerate()
                    .map(|(n, value)| n as f64 * value)
                    .sum::<f64>()
                    / weight
            };
            let expected = (600.0 - start) / step;
            // The kernel puts the impulse's centre on the read's crossing to
            // within a hundredth of a frame.
            assert!((centre(&out) - expected).abs() < 0.01, "step {step}");
            if step > 1.1 {
                // The frames sample the straight-line read's triangle too
                // sparsely for its peak to say where it lies.
                continue;
            }
            let peak = |values: &[f64]| {
                (0..values.len())
                    .max_by(|a, b| values[*a].total_cmp(&values[*b]))
                    .unwrap()
            };
            assert_eq!(peak(&out), peak(&linear), "step {step}");
        }
    }

    #[test]
    fn the_source_holds_its_end_frames_and_the_read_never_reaches_past_them() {
        let source: Vec<f32> = (0..64).map(|frame| (frame as f32 * 0.37).sin()).collect();
        // Beyond either end by more than the kernel reaches: the end frame,
        // to within the kernel's DC accuracy.
        for (position, frame) in [(-100.0, 0), (64.0 + 100.0, 63), (63.0 + 24.0 * 4.0, 63)] {
            for scale in [1.0, 0.25] {
                let value = read(&source, 1, position, scale)[0];
                assert!((value - source[frame]).abs() < 1e-4, "{position}: {value}");
            }
        }
        // Next to the ends the kernel reads only the frames there are.
        for position in [0.0, 0.5, 62.75, 63.0, 63.999] {
            for scale in [1.0, 0.5, 0.1] {
                assert!(read(&source, 1, position, scale)[0].is_finite());
            }
        }
        // A steady sound starts and ends at its level: the held ends add
        // no step at the clip's start and no ringing at the source's end.
        let steady = [0.5_f32; 300];
        for position in [0.0, 0.3, 1.0, 10.7, 298.5, 299.0, 299.9] {
            for scale in [1.0, 0.7] {
                let value = read(&steady, 1, position, scale)[0];
                assert!((value - 0.5).abs() < 1e-4, "{position}: {value}");
            }
        }
        // A tone well inside the pass band reads back as itself, at a whole
        // frame or between frames.
        for position in [30.0, 31.0, 31.5, 32.25] {
            let value = read(&source, 1, position, 1.0)[0];
            let want = (position as f32 * 0.37).sin();
            assert!((value - want).abs() < 1e-4, "{position}: {value} {want}");
        }
        assert!((read(&[0.25], 1, 0.0, 1.0)[0] - 0.25).abs() < 1e-5);
        assert_eq!(read(&[], 1, 0.0, 1.0), [0.0; 2]);
    }

    #[test]
    fn stereo_reads_each_channel_as_a_mono_source_would() {
        let left: Vec<f32> = (0..500).map(|frame| (frame as f32 * 0.11).sin()).collect();
        let right: Vec<f32> = (0..500).map(|frame| (frame as f32 * 0.29).cos()).collect();
        let stereo: Vec<f32> = left
            .iter()
            .zip(&right)
            .flat_map(|(l, r)| [*l, *r])
            .collect();
        for position in [0.0, 17.3, 250.5, 499.9] {
            for scale in [1.0, 0.8] {
                let both = read(&stereo, 2, position, scale);
                assert_eq!(both[0], read(&left, 1, position, scale)[0]);
                assert_eq!(both[1], read(&right, 1, position, scale)[0]);
                assert_eq!(read(&left, 1, position, scale)[1], 0.0);
            }
        }
    }

    #[test]
    fn the_kernel_moves_smoothly_with_the_speed() {
        // A cutoff that follows a moving speed changes the read gradually:
        // a small change of speed changes a full-scale bright source by
        // little, and the same speed always reads the same value.
        let source: Vec<f32> = (0..400)
            .map(|frame| if frame % 2 == 0 { 0.5 } else { -0.5 } * (frame as f32 * 0.05).cos())
            .collect();
        let mut previous = read(&source, 1, 200.4, scale(1.0))[0];
        for step in 1..=200 {
            let speed = 1.0 + f64::from(step) * 0.005;
            let value = read(&source, 1, 200.4, scale(speed))[0];
            assert!(
                (value - previous).abs() < 0.02,
                "{speed}: {value} {previous}"
            );
            assert_eq!(value, read(&source, 1, 200.4, scale(speed))[0]);
            previous = value;
        }
    }

    #[test]
    fn a_mix_prepares_the_table_before_playback() {
        const CHILD: &str = "MIXJAM_RESAMPLE_TABLE_INIT_TEST";
        const CHECKED: &str = "The resampling table is ready before playback.";
        if std::env::var_os(CHILD).is_some() {
            use std::sync::atomic::Ordering;
            assert_eq!(TABLE_INITIALIZATIONS.load(Ordering::Relaxed), 0);
            let mut project = crate::engine::Project {
                master_gain: 1.0,
                ..Default::default()
            };
            let mut editor = crate::engine::Editor::new(project.clone());
            editor.place(1, 0, 0, 256).unwrap();
            project = editor.project().clone();
            // A 48 kHz source plays off its rate, so it reads band-limited.
            let samples = std::collections::HashMap::from([(
                1,
                crate::audio::Pcm {
                    sample_rate: 48_000,
                    channels: 1,
                    samples: vec![0.25; 48_000],
                },
            )]);
            let mix = crate::audio::mix_with_samples(&project, &samples).unwrap();
            assert_eq!(TABLE_INITIALIZATIONS.load(Ordering::Relaxed), 1);
            let mut out = vec![0.0; 2048];
            mix.read(0, &mut out);
            assert_eq!(TABLE_INITIALIZATIONS.load(Ordering::Relaxed), 1);
            println!("{CHECKED}");
            return;
        }
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "audio::resample::tests::a_mix_prepares_the_table_before_playback",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            result.status.success() && String::from_utf8_lossy(&result.stdout).contains(CHECKED),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    #[test]
    fn looped_read_wraps_periodically_across_the_loop_seam() {
        // Exact period over frames 50..250 -> length 200.
        let mut source = vec![0.0_f32; 250];
        for (i, sample) in source[50..].iter_mut().enumerate() {
            *sample = (2.0 * PI * 3.0 * i as f64 / 200.0).sin() as f32;
        }
        for offset in [0.0, 0.37, 199.4, 200.25, 210.75] {
            let pos = 50.0 + offset;
            let got = read_looped(&source, 1, 50, pos, 1.0)[0];
            let expected = (2.0 * PI * 3.0 * offset / 200.0).sin() as f32;
            assert!(
                (got - expected).abs() < 1e-4,
                "offset {offset}: got {got}, expected {expected}"
            );
        }
    }
}

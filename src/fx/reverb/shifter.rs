//! The shimmer's pitch shifter: a stereo phase vocoder.
//!
//! One complex transform carries both channels. Each analysis hop finds the
//! spectral peaks and measures their true frequency from their phase advance.
//! The bins around a peak turn together with that peak, so a partial keeps its
//! shape. The overlap-added result is a time-stretched copy of the sound, read
//! back at the pitch ratio. The hop grid depends only on the number of frames
//! that passed, so the block size never changes the sound.
use super::super::RenderQuality;
use crate::fx::parts::{OnePole, cubic_lagrange, sin_cos_rad};
use crate::master::dsp::RATE;
use std::f64::consts::TAU;
use std::sync::OnceLock;

const SIZE: usize = 2048;
const BINS: usize = SIZE / 2 + 1;
const CAP: usize = SIZE * 4;
const MASK: usize = CAP - 1;

/// A vector as a boxed array of exactly `N` items.
fn boxed<T, const N: usize>(items: Vec<T>) -> Box<[T; N]> {
    items
        .into_boxed_slice()
        .try_into()
        .ok()
        .expect("the length of the array")
}

struct Tables {
    window: Box<[f64; SIZE]>,
    reverse: Box<[u16; SIZE]>,
    /// The twiddles of the stage with half-length `h` are at `h..2 * h`.
    cos: Box<[f64; SIZE]>,
    sin_forward: Box<[f64; SIZE]>,
    sin_inverse: Box<[f64; SIZE]>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let bits = SIZE.trailing_zeros();
        let mut cos = vec![0.0; SIZE];
        let mut sin_forward = vec![0.0; SIZE];
        let mut sin_inverse = vec![0.0; SIZE];
        let mut half = 1;
        while half < SIZE {
            for k in 0..half {
                let angle = TAU * k as f64 / (2 * half) as f64;
                cos[half + k] = angle.cos();
                sin_inverse[half + k] = angle.sin();
                sin_forward[half + k] = -angle.sin();
            }
            half *= 2;
        }
        Tables {
            window: boxed(
                (0..SIZE)
                    .map(|n| 0.5 - 0.5 * (TAU * n as f64 / SIZE as f64).cos())
                    .collect(),
            ),
            reverse: boxed(
                (0..SIZE)
                    .map(|n| (n.reverse_bits() >> (usize::BITS - bits)) as u16)
                    .collect(),
            ),
            cos: boxed(cos),
            sin_forward: boxed(sin_forward),
            sin_inverse: boxed(sin_inverse),
        }
    })
}

/// Build the transform tables before a processor reaches the audio thread.
pub(super) fn warm() {
    tables();
}

/// The synthesis hop and the sum of the squared windows at that hop.
fn hop(quality: RenderQuality) -> (usize, f64) {
    match quality {
        RenderQuality::Live => (SIZE / 4, 1.5),
        RenderQuality::Offline => (SIZE / 8, 3.0),
    }
}

/// The bins below the Nyquist frequency after the shift. The band limit
/// keeps the sound out of the others.
fn active_bins(ratio: f64) -> usize {
    (((SIZE / 2) as f64 / ratio).floor() as usize).clamp(2, BINS)
}

fn wrap(x: f64) -> f64 {
    x - TAU * (x / TAU).round()
}

fn bit_reverse(re: &mut [f64; SIZE], im: &mut [f64; SIZE]) {
    for (i, &j) in tables().reverse.iter().enumerate() {
        let j = usize::from(j);
        if j > i {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
}

/// An in-place radix-2 transform of `SIZE` points. The inverse scales by
/// `1 / SIZE`.
fn fft(re: &mut [f64; SIZE], im: &mut [f64; SIZE], inverse: bool) {
    bit_reverse(re, im);
    let t = tables();
    let sin = if inverse {
        &t.sin_inverse
    } else {
        &t.sin_forward
    };
    for (r, i) in re
        .as_chunks_mut::<2>()
        .0
        .iter_mut()
        .zip(im.as_chunks_mut::<2>().0)
    {
        let (r0, r1, i0, i1) = (r[0], r[1], i[0], i[1]);
        (r[0], r[1], i[0], i[1]) = (r0 + r1, r0 - r1, i0 + i1, i0 - i1);
    }
    let mut half = 2;
    while half < SIZE {
        let (cos, sin) = (&t.cos[half..2 * half], &sin[half..2 * half]);
        for (r, i) in re
            .chunks_exact_mut(2 * half)
            .zip(im.chunks_exact_mut(2 * half))
        {
            let ((r_low, r_high), (i_low, i_high)) = (r.split_at_mut(half), i.split_at_mut(half));
            let low = r_low.iter_mut().zip(r_high.iter_mut());
            let high = i_low.iter_mut().zip(i_high.iter_mut());
            for (((r_pair, i_pair), &c), &s) in low.zip(high).zip(cos).zip(sin) {
                let (r_lo, r_hi) = r_pair;
                let (i_lo, i_hi) = i_pair;
                let tr = *r_hi * c - *i_hi * s;
                let ti = *r_hi * s + *i_hi * c;
                (*r_hi, *i_hi) = (*r_lo - tr, *i_lo - ti);
                (*r_lo, *i_lo) = (*r_lo + tr, *i_lo + ti);
            }
        }
        half *= 2;
    }
    if inverse {
        let scale = 1.0 / SIZE as f64;
        re.iter_mut().chain(im.iter_mut()).for_each(|v| *v *= scale);
    }
}

/// Four-point cubic Lagrange read of a ring at a fractional position.
#[inline]
fn read_cubic(ring: &[f64; CAP], position: f64) -> f64 {
    let whole = position.floor();
    let f = position - whole;
    let n = whole as usize;
    let y = |k: usize| ring[(n + k + MASK) & MASK];
    cubic_lagrange([y(0), y(1), y(2), y(3)], f)
}

/// How far a peak's phase moved in one hop, and the turn that its bins take
/// so that they keep their place against the peak. `previous` is the bin of
/// the last hop, `turn` the peak's last turn.
fn peak_turn(bin: (f64, f64), previous: (f64, f64), turn: f64, k: usize, hops: (f64, f64)) -> f64 {
    let (re, im) = bin;
    let (pr, pi) = previous;
    let (ha, hs) = hops;
    let advance = if pr == 0.0 && pi == 0.0 {
        im.atan2(re)
    } else {
        (im * pr - re * pi).atan2(re * pr + im * pi)
    };
    let centre = TAU * k as f64 / SIZE as f64;
    let omega = centre + wrap(advance - centre * ha) / ha;
    wrap(turn + omega * hs - advance)
}

/// The bins that peak `j` of `count` leads: those nearer to it than to
/// another peak.
fn region(peaks: &[usize], j: usize, count: usize, active: usize) -> (usize, usize) {
    let low = if j == 0 {
        0
    } else {
        (peaks[j - 1] + peaks[j]) / 2 + 1
    };
    let high = if j + 1 == count {
        active
    } else {
        (peaks[j] + peaks[j + 1]) / 2 + 1
    };
    (low, high)
}

/// The state of one channel.
struct Channel {
    band: [OnePole; 2],
    input: Box<[f64; CAP]>,
    overlap: Box<[f64; CAP]>,
    previous_re: Box<[f64; BINS]>,
    previous_im: Box<[f64; BINS]>,
    turns: Box<[f64; BINS]>,
}
impl Channel {
    fn new() -> Self {
        let bins = || -> Box<[f64; BINS]> { boxed(vec![0.0; BINS]) };
        Self {
            band: [OnePole::default(); 2],
            input: boxed(vec![0.0; CAP]),
            overlap: boxed(vec![0.0; CAP]),
            previous_re: bins(),
            previous_im: bins(),
            turns: bins(),
        }
    }
    fn clear(&mut self) {
        self.band.iter_mut().for_each(OnePole::clear);
        self.input.fill(0.0);
        self.overlap.fill(0.0);
        self.forget(BINS);
    }
    fn forget(&mut self, active: usize) {
        for buffer in [
            &mut self.previous_re,
            &mut self.previous_im,
            &mut self.turns,
        ] {
            buffer[..active].fill(0.0);
        }
    }
}

/// Work space that the hops reuse, so processing never allocates.
struct Scratch {
    re: Box<[f64; SIZE]>,
    im: Box<[f64; SIZE]>,
    bins_re: [Box<[f64; BINS]>; 2],
    bins_im: [Box<[f64; BINS]>; 2],
    power: Box<[f64; BINS]>,
    peaks: Box<[usize; BINS]>,
}
impl Scratch {
    fn new() -> Self {
        let bins = || -> Box<[f64; BINS]> { boxed(vec![0.0; BINS]) };
        Self {
            re: boxed(vec![0.0; SIZE]),
            im: boxed(vec![0.0; SIZE]),
            bins_re: [bins(), bins()],
            bins_im: [bins(), bins()],
            power: bins(),
            peaks: boxed(vec![0; BINS]),
        }
    }

    /// Split the transform of the two channels into one spectrum for each.
    fn unpack(&mut self, active: usize) {
        for k in 0..active {
            let c = (SIZE - k) & (SIZE - 1);
            let (zr, zi, cr, ci) = (self.re[k], self.im[k], self.re[c], self.im[c]);
            self.bins_re[0][k] = 0.5 * (zr + cr);
            self.bins_im[0][k] = 0.5 * (zi - ci);
            self.bins_re[1][k] = 0.5 * (zi + ci);
            self.bins_im[1][k] = -0.5 * (zr - cr);
        }
    }

    /// Join the two spectra into the transform of one complex signal.
    fn pack(&mut self, active: usize) {
        self.re.fill(0.0);
        self.im.fill(0.0);
        for k in 1..active.min(SIZE / 2) {
            let (lr, li) = (self.bins_re[0][k], self.bins_im[0][k]);
            let (rr, ri) = (self.bins_re[1][k], self.bins_im[1][k]);
            self.re[k] = lr - ri;
            self.im[k] = li + rr;
            self.re[SIZE - k] = lr + ri;
            self.im[SIZE - k] = rr - li;
        }
        for k in [0, SIZE / 2].into_iter().filter(|k| *k < active) {
            self.re[k] = self.bins_re[0][k];
            self.im[k] = self.bins_re[1][k];
        }
    }

    /// The peaks of channel `ch` that rise above 1e-6 of the strongest power,
    /// and that power.
    fn find_peaks(&mut self, ch: usize, active: usize) -> (usize, f64) {
        let mut strongest = 0.0_f64;
        for k in 0..active {
            let (re, im) = (self.bins_re[ch][k], self.bins_im[ch][k]);
            self.power[k] = re * re + im * im;
            strongest = strongest.max(self.power[k]);
        }
        if strongest == 0.0 {
            return (0, 0.0);
        }
        let floor = strongest * 1e-6;
        let mut count = 0;
        for k in 1..active - 1 {
            let p = self.power[k];
            if p > floor && p >= self.power[k - 1] && p >= self.power[k + 1] {
                self.peaks[count] = k;
                count += 1;
            }
        }
        (count, strongest)
    }

    /// Turn the bins of channel `ch` to their new phases. Each peak moves by
    /// its own frequency and its bins follow it. A silent channel gives
    /// `false`.
    fn turn_channel(
        &mut self,
        ch: usize,
        chan: &mut Channel,
        active: usize,
        hops: (f64, f64),
    ) -> bool {
        let (mut count, strongest) = self.find_peaks(ch, active);
        if strongest == 0.0 {
            chan.forget(active);
            self.bins_re[ch][..active].fill(0.0);
            self.bins_im[ch][..active].fill(0.0);
            return false;
        }
        if count == 0 {
            let edge = if self.power[0] >= self.power[active - 1] {
                0
            } else {
                active - 1
            };
            self.peaks[0] = edge;
            count = 1;
        }
        for j in 0..count {
            let p = self.peaks[j];
            let (low, high) = region(&self.peaks[..], j, count, active);
            let bin = (self.bins_re[ch][p], self.bins_im[ch][p]);
            let previous = (chan.previous_re[p], chan.previous_im[p]);
            let turn = peak_turn(bin, previous, chan.turns[p], p, hops);
            let (sin, cos) = sin_cos_rad(turn);
            for k in low..high {
                let (re, im) = (self.bins_re[ch][k], self.bins_im[ch][k]);
                chan.previous_re[k] = re;
                chan.previous_im[k] = im;
                chan.turns[k] = turn;
                self.bins_re[ch][k] = re * cos - im * sin;
                self.bins_im[ch][k] = re * sin + im * cos;
            }
        }
        true
    }
}

/// A stereo shifter that moves the sound up by a ratio.
pub(super) struct Shifter {
    chans: [Channel; 2],
    scratch: Scratch,
    write: usize,
    out: usize,
    /// Frames since the last analysis hop.
    acc: f64,
}
impl Shifter {
    pub(super) fn new() -> Self {
        Self {
            chans: [Channel::new(), Channel::new()],
            scratch: Scratch::new(),
            write: 0,
            out: 0,
            acc: 0.0,
        }
    }

    /// Silence the sound. The hop grid keeps its phase, as the reverb's
    /// modulation does, so a bus that slept sounds as one that did not.
    pub(super) fn clear(&mut self) {
        self.chans.iter_mut().for_each(Channel::clear);
    }

    /// Move the hop grid on by `frames` frames that the bus slept through.
    pub(super) fn skip(&mut self, ratio: f64, frames: u64, quality: RenderQuality) {
        let ha = hop(quality).0 as f64 / ratio;
        self.acc = (self.acc + frames as f64).rem_euclid(ha);
    }

    /// Band-limit the input so the shifted copy stays under the Nyquist
    /// frequency and the tone's high cut.
    pub(super) fn tune(&mut self, ratio: f64, high_cut: f64) {
        let corner = (0.45 * RATE / ratio).min(high_cut);
        for chan in &mut self.chans {
            chan.band.iter_mut().for_each(|pole| pole.tune(corner));
        }
    }

    /// The analysis frame that ends `acc` frames before the newest sample,
    /// windowed, with the left channel real and the right one imaginary.
    fn load(&mut self) {
        let window = &tables().window;
        let whole = self.acc.floor();
        let frac = 1.0 - (self.acc - whole);
        let first = self.write + CAP - SIZE - whole as usize - 1;
        let [left, right] = &self.chans;
        for n in 0..SIZE {
            let (i, j) = ((first + n) & MASK, (first + n + 1) & MASK);
            let at = |chan: &Channel| chan.input[i] * (1.0 - frac) + chan.input[j] * frac;
            self.scratch.re[n] = at(left) * window[n];
            self.scratch.im[n] = at(right) * window[n];
        }
    }

    /// Clear the room for the next hop and move the output on.
    fn advance_overlap(&mut self, hs: usize) {
        for n in 0..hs {
            let at = (self.out + SIZE + n) & MASK;
            self.chans
                .iter_mut()
                .for_each(|chan| chan.overlap[at] = 0.0);
        }
        self.out = (self.out + hs) & MASK;
    }

    fn overlap_add(&mut self, hs: usize, norm: f64) {
        let window = &tables().window;
        for n in 0..SIZE {
            let at = (self.out + n) & MASK;
            let w = window[n] / norm;
            self.chans[0].overlap[at] += self.scratch.re[n] * w;
            self.chans[1].overlap[at] += self.scratch.im[n] * w;
        }
        self.advance_overlap(hs);
    }

    fn hop(&mut self, ratio: f64, ha: f64, hs: usize, norm: f64) {
        let active = active_bins(ratio);
        self.load();
        fft(&mut self.scratch.re, &mut self.scratch.im, false);
        self.scratch.unpack(active);
        let mut sounding = false;
        for (ch, chan) in self.chans.iter_mut().enumerate() {
            sounding |= self.scratch.turn_channel(ch, chan, active, (ha, hs as f64));
        }
        if !sounding {
            self.advance_overlap(hs);
            return;
        }
        self.scratch.pack(active);
        fft(&mut self.scratch.re, &mut self.scratch.im, true);
        self.overlap_add(hs, norm);
    }

    /// Take one stereo frame and give the shifted one, which comes about
    /// two synthesis hops later.
    #[inline]
    pub(super) fn process_pair(
        &mut self,
        input: [f64; 2],
        ratio: f64,
        quality: RenderQuality,
    ) -> [f64; 2] {
        for (chan, x) in self.chans.iter_mut().zip(input) {
            let first = chan.band[0].low(x);
            let limited = chan.band[1].low(first);
            chan.input[self.write] = limited;
        }
        self.write = (self.write + 1) & MASK;
        let (hs, norm) = hop(quality);
        let ha = hs as f64 / ratio;
        self.acc += 1.0;
        while self.acc >= ha {
            self.acc -= ha;
            self.hop(ratio, ha, hs, norm);
        }
        let position = ((self.out + CAP - 2 * hs) & MASK) as f64 + self.acc * ratio;
        [
            read_cubic(&self.chans[0].overlap, position),
            read_cubic(&self.chans[1].overlap, position),
        ]
    }
}

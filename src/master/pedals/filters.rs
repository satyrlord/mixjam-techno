//! The filter pedals: the trim EQ, the lift EQ, and the stereo imaging.
//! Each recomputes its filters while a setting glides.
use super::{REFRESH, Refresh};
use crate::master as settings;
use crate::master::dsp::{Biquad, Coefficients, Smoothed, cascade};
use std::f64::consts::FRAC_1_SQRT_2;

// Trim EQ -------------------------------------------------------------------------------

const MUD: (f64, f64) = (250.0, 2.5);
const HARSH: (f64, f64) = (3500.0, 3.0);

fn trim_eq_coefficients(high_pass: f64, mud: f64, harsh: f64) -> [Coefficients; 3] {
    [
        Coefficients::high_pass(high_pass, FRAC_1_SQRT_2),
        Coefficients::peaking_matched(MUD.0, MUD.1, mud),
        Coefficients::peaking_matched(HARSH.0, HARSH.1, harsh),
    ]
}

fn lift_eq_coefficients(low: f64, air: f64) -> [Coefficients; 2] {
    [
        Coefficients::low_shelf(LOW_SHELF_HZ, LIFT_SLOPE, low),
        Coefficients::high_shelf_matched(AIR_SHELF_HZ, LIFT_SLOPE, air),
    ]
}

/// Sample the steady-state magnitude response (in dB) of `TrimEq` or `LiftEq`
/// across logarithmically spaced frequencies from 20 Hz to 20 kHz.
pub fn sample_master_eq_curve(
    chain: &settings::MasterChain,
    kind: settings::PedalKind,
    out_db: &mut [f32],
) {
    let n = out_db.len();
    if n == 0 {
        return;
    }
    let trim;
    let lift;
    let sections: &[Coefficients] = match kind {
        settings::PedalKind::TrimEq => {
            trim = trim_eq_coefficients(
                f64::from(chain.trim_eq.high_pass),
                f64::from(chain.trim_eq.mud),
                f64::from(chain.trim_eq.harsh),
            );
            &trim
        }
        settings::PedalKind::LiftEq => {
            lift = lift_eq_coefficients(f64::from(chain.lift_eq.low), f64::from(chain.lift_eq.air));
            &lift
        }
        _ => {
            out_db.fill(0.0);
            return;
        }
    };
    let denom = (n - 1).max(1) as f64;
    for (i, slot) in out_db.iter_mut().enumerate() {
        let hz = 20.0 * 1000.0_f64.powf(i as f64 / denom);
        let mag = sections
            .iter()
            .fold(1.0_f64, |acc, section| acc * section.magnitude(hz));
        *slot = (20.0 * mag.max(1e-6).log10()) as f32;
    }
}

fn set_biquads<const N: usize>(filters: &mut [Biquad; N], coeffs: [Coefficients; N]) {
    for (filter, c) in filters.iter_mut().zip(coeffs) {
        filter.set(c);
    }
}

/// Cleanup: a 12 dB per octave Butterworth high-pass, and narrow cuts at
/// 250 Hz and 3.5 kHz.
#[derive(Clone, Copy)]
pub(in crate::master) struct TrimEq {
    filters: [Biquad; 3],
    high_pass: Smoothed,
    mud: Smoothed,
    harsh: Smoothed,
    refresh: Refresh,
}
impl TrimEq {
    pub(in crate::master) fn new() -> Self {
        Self {
            filters: [Biquad::FLAT; 3],
            high_pass: Smoothed::new(20.0),
            mud: Smoothed::new(0.0),
            harsh: Smoothed::new(0.0),
            refresh: Refresh(REFRESH),
        }
    }
    pub(in crate::master) fn set(&mut self, settings: &settings::TrimEq) {
        self.high_pass.set(f64::from(settings.high_pass));
        self.mud.set(f64::from(settings.mud));
        self.harsh.set(f64::from(settings.harsh));
    }
    fn design(&mut self) {
        set_biquads(
            &mut self.filters,
            trim_eq_coefficients(self.high_pass.value(), self.mud.value(), self.harsh.value()),
        );
    }
    pub(in crate::master) fn clear(&mut self) {
        self.filters.iter_mut().for_each(Biquad::clear);
        for setting in [&mut self.high_pass, &mut self.mud, &mut self.harsh] {
            setting.settle();
        }
        self.design();
    }
    fn settled(&self) -> bool {
        self.high_pass.settled() && self.mud.settled() && self.harsh.settled()
    }
    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let gliding = !self.settled();
        if gliding {
            for setting in [&mut self.high_pass, &mut self.mud, &mut self.harsh] {
                setting.next();
            }
        }
        if self.refresh.due(gliding, self.settled()) {
            self.design();
        }
        (
            cascade(&mut self.filters, 0, f64::from(left)) as f32,
            cascade(&mut self.filters, 1, f64::from(right)) as f32,
        )
    }
}

// Lift EQ -------------------------------------------------------------------------------------

const LOW_SHELF_HZ: f64 = 90.0;
const AIR_SHELF_HZ: f64 = 12_000.0;
const LIFT_SLOPE: f64 = 0.6;

/// Wide, gentle shelves that add weight at 90 Hz and air at 12 kHz.
#[derive(Clone, Copy)]
pub(in crate::master) struct LiftEq {
    filters: [Biquad; 2],
    low: Smoothed,
    air: Smoothed,
    refresh: Refresh,
}
impl LiftEq {
    pub(in crate::master) fn new() -> Self {
        Self {
            filters: [Biquad::FLAT; 2],
            low: Smoothed::new(0.0),
            air: Smoothed::new(0.0),
            refresh: Refresh(REFRESH),
        }
    }
    pub(in crate::master) fn set(&mut self, settings: &settings::LiftEq) {
        self.low.set(f64::from(settings.low));
        self.air.set(f64::from(settings.air));
    }
    fn design(&mut self) {
        set_biquads(
            &mut self.filters,
            lift_eq_coefficients(self.low.value(), self.air.value()),
        );
    }
    pub(in crate::master) fn clear(&mut self) {
        self.filters.iter_mut().for_each(Biquad::clear);
        self.low.settle();
        self.air.settle();
        self.design();
    }
    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let gliding = !(self.low.settled() && self.air.settled());
        if gliding {
            self.low.next();
            self.air.next();
        }
        if self
            .refresh
            .due(gliding, self.low.settled() && self.air.settled())
        {
            self.design();
        }
        (
            cascade(&mut self.filters, 0, f64::from(left)) as f32,
            cascade(&mut self.filters, 1, f64::from(right)) as f32,
        )
    }
}

// Stereo imaging ------------------------------------------------------------------------------

/// Mid and side: the mid passes untouched. The side loses everything below
/// the mono frequency through a 24 dB per octave Linkwitz-Riley high-pass,
/// and the width scales the rest. The two channels always sum to twice the
/// mid, so mono playback never changes.
#[derive(Clone, Copy)]
pub(in crate::master) struct Imaging {
    side: [Biquad; 2],
    width: Smoothed,
    mono_below: Smoothed,
    refresh: Refresh,
}
impl Imaging {
    pub(in crate::master) fn new() -> Self {
        Self {
            side: [Biquad::FLAT; 2],
            width: Smoothed::new(1.0),
            mono_below: Smoothed::new(120.0),
            refresh: Refresh(REFRESH),
        }
    }
    pub(in crate::master) fn set(&mut self, settings: &settings::Imaging) {
        self.width.set(f64::from(settings.width) / 100.0);
        self.mono_below.set(f64::from(settings.mono_below));
    }
    fn design(&mut self) {
        let coefficients = Coefficients::high_pass(self.mono_below.value(), FRAC_1_SQRT_2);
        self.side.iter_mut().for_each(|f| f.set(coefficients));
    }
    pub(in crate::master) fn clear(&mut self) {
        self.side.iter_mut().for_each(Biquad::clear);
        self.width.settle();
        self.mono_below.settle();
        self.design();
    }
    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let gliding = !self.mono_below.settled();
        if gliding {
            self.mono_below.next();
        }
        if self.refresh.due(gliding, self.mono_below.settled()) {
            self.design();
        }
        let width = self.width.next();
        let mid = 0.5 * (left + right);
        let side = self.side[0].process(0, f64::from(0.5 * (left - right)));
        let side = (self.side[1].process(0, side) * width) as f32;
        (mid + side, mid - side)
    }
}

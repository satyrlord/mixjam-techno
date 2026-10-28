//! The Booster pedals: the master effects of the D3, D4, and T3 products,
//! from their effects library. They follow the reference models in
//! `tools/runtime/fx-reference.py` and match the library's renders sample
//! for sample. They work on 16-bit samples, as the original does.
use super::EQ_FREQUENCIES;
use std::sync::LazyLock;

pub(super) fn clamp16(value: i32) -> i16 {
    value.clamp(-32768, 32767) as i16
}

/// The low 32 bits as a signed value, as a 32-bit register holds them.
fn s32(value: i64) -> i32 {
    value as i32
}

fn s16(value: i64) -> i16 {
    value as i16
}

// Equaliser ---------------------------------------------------------------

/// Ten biquads in direct form 1, computed in double precision and stored
/// in single precision, as the library does on the x87 unit at 53-bit
/// precision. A band at gain 0.5 is skipped.
#[derive(Clone, Copy)]
pub(super) struct Equalizer {
    gains: [f32; 10],
    coefficients: [[f32; 5]; 10],
    /// Per band and channel: x1, x2, y1, y2.
    state: [[[f32; 4]; 2]; 10],
}
impl Equalizer {
    pub(super) fn new() -> Self {
        Self {
            gains: [0.5; 10],
            coefficients: [[0.0; 5]; 10],
            state: [[[0.0; 4]; 2]; 10],
        }
    }
    pub(super) fn clear(&mut self) {
        self.state = [[[0.0; 4]; 2]; 10];
    }
    pub(super) fn set(&mut self, bands: &[u8; 10]) {
        for (band, value) in bands.iter().enumerate() {
            // The editor passes the saved value divided by 100.
            let gain = ((f64::from(*value) / 100.0) as f32).clamp(0.0, 1.0);
            self.gains[band] = gain;
            self.coefficients[band] = eq_coefficients(band, gain).map(|v| v as f32);
        }
    }
    pub(super) fn process(&mut self, channel: usize, input: f32) -> f32 {
        let mut x0 = input;
        for band in 0..10 {
            if self.gains[band] == 0.5 {
                continue;
            }
            let [b0, b1, b2, a1, a2] = self.coefficients[band].map(f64::from);
            let [x1, x2, y1, y2] = &mut self.state[band][channel];
            let y = (((f64::from(x0) * b0 + b1 * f64::from(*x1)) + b2 * f64::from(*x2)
                - a1 * f64::from(*y1))
                - a2 * f64::from(*y2)) as f32;
            (*x2, *x1, *y2, *y1) = (*x1, x0, *y1, y);
            x0 = y;
        }
        x0
    }
}

/// `b0, b1, b2, a1, a2` of the library's band routines. Bands 0 and 9 are
/// shelves with a 12 dB range, the others peaking with an 18 dB range and
/// Q 4. Where the library keeps a value in single precision, so does this.
fn eq_coefficients(band: usize, gain: f32) -> [f64; 5] {
    const Q: f64 = 4.0;
    let f32r = |value: f64| f64::from(value as f32);
    let gain = f64::from(gain);
    let shelf = band == 0 || band == 9;
    let level = if shelf {
        gain * 24.0 - 12.0
    } else {
        gain * 36.0 - 18.0
    };
    let tangent = if band == 0 {
        // Single-precision pi/44100.
        (EQ_FREQUENCIES[band] * f64::from(7.123_792_6e-5_f32)).tan()
    } else {
        // The single-precision value one step below pi, over 44,100.
        (EQ_FREQUENCIES[band] * f64::from(3.141_592_5_f32) * 2.267_573_696_145_124_8e-5).tan()
    };
    let k = f32r(tangent);
    let k2 = f32r(tangent * k);
    let mut v = (level.abs() * f64::from(0.05_f32) * 10f64.ln()).exp();
    let root2k = 2f64.sqrt() * k;
    let plain = k2 + root2k + 1.0;
    let normal = |b: [f64; 3], a: [f64; 2]| [b[0], b[1], b[2], a[0], a[1]];
    if band == 0 {
        v = f32r(v);
        let rvk = (v + v).sqrt() * k;
        let vk2 = f32r(v * k2);
        if level < 0.0 {
            let d = vk2 + rvk + 1.0;
            return normal(
                [
                    plain / d,
                    ((k2 - 1.0) + (k2 - 1.0)) / d,
                    ((1.0 - root2k) + k2) / d,
                ],
                [((vk2 - 1.0) + (vk2 - 1.0)) / d, (vk2 + (1.0 - rvk)) / d],
            );
        }
        return normal(
            [
                (vk2 + rvk + 1.0) / plain,
                ((vk2 - 1.0) + (vk2 - 1.0)) / plain,
                ((1.0 - rvk) + vk2) / plain,
            ],
            [
                ((k2 - 1.0) + (k2 - 1.0)) / plain,
                ((1.0 - root2k) + k2) / plain,
            ],
        );
    }
    if band == 9 {
        let rvk = (v + v).sqrt() * k;
        if level < 0.0 {
            let d = v + rvk + k2;
            let kv = f32r(k2 / v);
            let e = (2.0 / v).sqrt() * k;
            let d2 = kv + e + 1.0;
            return normal(
                [
                    plain / d,
                    ((k2 - 1.0) + (k2 - 1.0)) / d,
                    ((1.0 - root2k) + k2) / d,
                ],
                [((kv - 1.0) + (kv - 1.0)) / d2, (kv + (1.0 - e)) / d2],
            );
        }
        return normal(
            [
                (v + k2 + rvk) / plain,
                ((k2 - v) + (k2 - v)) / plain,
                ((v - rvk) + k2) / plain,
            ],
            [
                ((k2 - 1.0) + (k2 - 1.0)) / plain,
                ((1.0 - root2k) + k2) / plain,
            ],
        );
    }
    let kq = k / Q;
    if level < 0.0 {
        let vkq = (v * k) / Q;
        let d = f32r(vkq + k2 + 1.0);
        let middle = ((k2 - 1.0) + (k2 - 1.0)) / d;
        return normal(
            [(kq + k2 + 1.0) / d, middle, ((1.0 - kq) + k2) / d],
            [middle, ((1.0 - f32r(vkq)) + k2) / d],
        );
    }
    let vkq = f32r((v * k) / Q);
    let d = f32r(kq + k2 + 1.0);
    let middle = ((k2 - 1.0) + (k2 - 1.0)) / d;
    normal(
        [(vkq + k2 + 1.0) / d, middle, ((1.0 - vkq) + k2) / d],
        [middle, ((1.0 - kq) + k2) / d],
    )
}

// Compressor ----------------------------------------------------------------

/// `trunc(sin(i / 4096 * pi / 2) * 65535)`, as the library builds it.
static COMPRESSOR_GAIN: LazyLock<[u16; 4096]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        ((i as f64 / 4096.0 * std::f64::consts::PI / 2.0).sin() * 65535.0) as u16
    })
});
/// `trunc(tanh(sinh(6 i / 4096)) * 32767)`. The library's table reaches
/// 32767 from entry 2473, where this double-precision form still gives 32766.
static COMPRESSOR_SHAPE: LazyLock<[u16; 4096]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        if i >= 2473 {
            32767
        } else {
            ((6.0 * i as f64 / 4096.0).sinh().tanh() * 32767.0) as u16
        }
    })
});

/// The compressor of the original effects library. The level follows the absolute value
/// of `mono - previous / 2` through a fixed integer low-pass; the gain table
/// and a division set the gain, and the shape table limits each channel.
#[derive(Clone, Copy)]
pub(super) struct Compressor {
    gain_table: &'static [u16; 4096],
    shape_table: &'static [u16; 4096],
    /// Smoothed drive and makeup gain, in 1/65536 steps.
    drive: i64,
    gain: i64,
    envelope: i64,
    previous: i32,
    attack: i64,
    drive_target: i64,
    release: i64,
    gain_target: i64,
    x1: i64,
    y1: i64,
    y2: i64,
}
impl Compressor {
    /// Level filter coefficients `b0, b1, a0, a1, a2` as the library holds them.
    const FILTER: [i128; 5] = [209, 209, 16_777_216, -33_386_659, 16_609_863];

    /// The state that the library holds after it loads.
    pub(super) fn at_load() -> Self {
        Self {
            gain_table: &COMPRESSOR_GAIN,
            shape_table: &COMPRESSOR_SHAPE,
            drive: 0x40 << 16,
            gain: 0x40 << 16,
            envelope: 0,
            previous: 0,
            attack: 1,
            drive_target: 0,
            release: 0,
            gain_target: 0,
            x1: 0,
            y1: 0,
            y2: 0,
        }
    }
    /// Move the smoothed controls to their targets.
    pub(super) fn settle(&mut self) {
        self.drive = self.drive_target << 16;
        self.gain = self.gain_target << 16;
    }
    /// The enable call clears the level filter only.
    pub(super) fn clear(&mut self) {
        (self.x1, self.y1, self.y2) = (0, 0, 0);
    }
    pub(super) fn set(&mut self, drive: u16, speed: u16, gain: u16) {
        let (drive, speed, gain) = (i64::from(drive), i64::from(speed), i64::from(gain));
        self.drive_target = drive + 0x40;
        self.release = i64::from(s16(0x3FFF - speed * 0x1F / 1000));
        self.gain_target = i64::from(s16((gain << 9) / 1000));
    }
    fn shape(&self, value: i32) -> i32 {
        let scaled = (s32((i64::from(value).abs()) << 12) / 0x200) as u32;
        let (mut index, mut fraction) = ((scaled >> 7) as usize, scaled & 0x7F);
        if index >= 0xFFE {
            (index, fraction) = (0xFFE, 0);
        }
        let result = (u32::from(self.shape_table[index]) * (0x80 - fraction)
            + u32::from(self.shape_table[index + 1]) * fraction)
            / 128;
        if value < 0 {
            -(result as i32)
        } else {
            result as i32
        }
    }
    pub(super) fn process(&mut self, left: i32, right: i32) -> (i32, i32) {
        let mono = (left + right) >> 1;
        let drive = i64::from(s16(self.drive >> 16));
        let level = i64::from(s32(
            ((f64::from(mono) - f64::from(self.previous) * 0.5).trunc() as i64).abs() << 8,
        ));
        self.previous = mono;
        self.envelope = if level > self.envelope {
            ((0x80 - self.attack) * self.envelope + self.attack * level) / 0x80
        } else {
            (self.release * self.envelope) >> 14
        };
        let x0 = (self.envelope >> 8) * 0x100000;
        let [b0, b1, a0, a1, a2] = Self::FILTER;
        let y = ((b0 * i128::from(x0) - a1 * i128::from(self.y1) - a2 * i128::from(self.y2)
            + b1 * i128::from(self.x1))
            / a0) as i64;
        (self.y2, self.y1, self.x1) = (self.y1, y, x0);
        let smooth = i64::from(s32(y / 0x100000));
        let product = i64::from(s32(drive * smooth));
        let lookup = (product / 8).max(0);
        let factor = if (lookup & 0xFFFF_FF80) >= 0x7FF00 {
            0xFFFF
        } else {
            let index = (lookup >> 7) as usize;
            let fraction = lookup & 0x7F;
            (i64::from(self.gain_table[index]) * (0x80 - fraction)
                + i64::from(self.gain_table[index + 1]) * fraction)
                >> 7
        };
        let factor = (factor & 0xFFFF) >> 1;
        let divisor = if drive < 0x80 { product / 128 } else { smooth }.max(1);
        let makeup = i64::from(s32(self.gain >> 16));
        let channel =
            |sample: i32| self.shape(s32((i64::from(sample) * factor / divisor) * makeup / 0x800));
        let out = (channel(left), channel(right));
        self.drive = (self.drive * 0xFFF + (self.drive_target << 16)) / 0x1000;
        self.gain = (self.gain * 0xFFF + (self.gain_target << 16)) / 0x1000;
        out
    }
}

// Stereo wide -----------------------------------------------------------------

/// The side signal, delayed 44 samples and scaled, added to and subtracted
/// from the mid.
#[derive(Clone, Copy)]
pub(super) struct Wide {
    spread: i64,
    buffer: [i32; 0x800],
    position: usize,
}
impl Wide {
    const DELAY: usize = 44;

    pub(super) fn new() -> Self {
        Self {
            spread: 0,
            buffer: [0; 0x800],
            position: 0,
        }
    }
    pub(super) fn clear(&mut self) {
        self.buffer = [0; 0x800];
        self.position = 0;
    }
    pub(super) fn set(&mut self, spread: u16) {
        self.spread = i64::from(s16((i64::from(spread) << 6) / 1000));
    }
    pub(super) fn process(&mut self, left: i32, right: i32) -> (i32, i32) {
        let mid = left.wrapping_add(right);
        self.buffer[self.position] = left.wrapping_sub(right);
        let delayed = self.buffer[self.position.wrapping_sub(Self::DELAY) & 0x7FF];
        let side = s32(i64::from(delayed) * self.spread) / 16;
        self.position = (self.position + 1) & 0x7FF;
        (side.wrapping_add(mid) / 2, mid.wrapping_sub(side) / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master::{Board, FADE, MasterChain, MasterControl, MasterProcessor, PedalKind};
    use sha2::{Digest, Sha256};

    /// The oracle's test signal: an impulse, a 440 Hz burst, then noise.
    fn oracle_signal() -> Vec<i32> {
        let mut samples = vec![0; 98_304];
        samples[100] = 16_000;
        for (index, sample) in samples.iter_mut().enumerate().take(44_100).skip(22_050) {
            *sample = (12_000.0
                * (2.0 * std::f64::consts::PI * 440.0 * index as f64 / 44_100.0).sin())
            .round() as i32;
        }
        let mut state: u32 = 12_345;
        for sample in &mut samples[52_000..70_000] {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345) & 0x7FFF_FFFF;
            *sample = ((((state >> 8) & 0xFFFF) as i32) - 32_768) >> 1;
        }
        samples
    }

    fn sha256_hex(bytes: impl IntoIterator<Item = u8>) -> String {
        let mut hash = Sha256::new();
        hash.update(bytes.into_iter().collect::<Vec<_>>());
        hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    fn table_hash(table: &[u16; 4096]) -> String {
        sha256_hex(table.iter().flat_map(|v| v.to_le_bytes()))
    }

    #[test]
    fn signal_and_tables_match_the_oracle() {
        let signal = oracle_signal();
        assert_eq!(
            sha256_hex(signal.iter().flat_map(|v| (*v as i16).to_le_bytes())),
            "eef7f1e4036422534e8b07d1cd35946307a915e7e243c80718315322531d5387"
        );
        assert_eq!(
            table_hash(&COMPRESSOR_GAIN),
            "8802f09bf33ae29b09131de9c2e400fed4d91f113d1169f67b87fabf4e972462"
        );
        assert_eq!(
            table_hash(&COMPRESSOR_SHAPE),
            "a344a6147c9d30b2c1d95a3ef0dcbd7506ea1b398decbdbc811d6af382a43ed4"
        );
    }

    /// The stereo renders of `fx-oracle.py`, in the order the oracle made
    /// them. The compressor's smoothed controls and level never reset, so
    /// its state carries from one render to the next, as it did in the DLL.
    #[test]
    fn booster_matches_every_stereo_render_of_the_library() {
        struct Render {
            name: &'static str,
            block: usize,
            eq: Option<[f32; 10]>,
            compressor: Option<[u16; 3]>,
            wide: Option<u16>,
            /// The hash of the output; `None` for a render whose pitch stage
            /// this port does not model, run only to advance the compressor.
            sha256: Option<&'static str>,
        }
        const FLAT: [f32; 10] = [0.5; 10];
        let flat_with = |changes: &[(usize, f32)]| {
            let mut gains = FLAT;
            for (band, gain) in changes {
                gains[*band] = *gain;
            }
            Some(gains)
        };
        let all = |block, sha256| Render {
            name: "all",
            block,
            eq: flat_with(&[(4, 0.7)]),
            compressor: Some([600, 300, 600]),
            wide: Some(500),
            sha256: Some(sha256),
        };
        let renders = [
            Render {
                name: "dry",
                block: 4096,
                eq: None,
                compressor: None,
                wide: None,
                sha256: Some("d67b7391da1bda3e7f99679c325d02e86855198675f59b7edad81cc3b8af6b74"),
            },
            Render {
                name: "compressor 0",
                block: 4096,
                eq: None,
                compressor: Some([0, 500, 500]),
                wide: None,
                sha256: Some("730001bb532ff4f361bf9f773bde95e2e7082f04ced7a907527a3585cecd98c7"),
            },
            Render {
                name: "compressor 300",
                block: 4096,
                eq: None,
                compressor: Some([300, 500, 500]),
                wide: None,
                sha256: Some("d80cab845fc04e16a281e84e0b7015fe36dc77f4a432c61bc189351563f9dad6"),
            },
            Render {
                name: "compressor 800",
                block: 4096,
                eq: None,
                compressor: Some([800, 100, 700]),
                wide: None,
                sha256: Some("6d9438738b85b0dd4af337cb3d3e20ac7d673d9fd66da8fc104917ba87c2ab65"),
            },
            Render {
                name: "compressor 1000",
                block: 4096,
                eq: None,
                compressor: Some([1000, 1000, 1000]),
                wide: None,
                sha256: Some("e91b3c025075926a391830ed9427062d4cbe715173bffc80573d769626e45f9b"),
            },
            Render {
                name: "wide 0",
                block: 4096,
                eq: None,
                compressor: None,
                wide: Some(0),
                sha256: Some("3594d65c17f7c521b83c28e5a93e198c59c106ed34cb7a09725171de9dad583b"),
            },
            Render {
                name: "wide 500",
                block: 4096,
                eq: None,
                compressor: None,
                wide: Some(500),
                sha256: Some("cb7b8f034d83830d449b6e4d354e5793773e3ba6950407e560d67346d153cf50"),
            },
            Render {
                name: "wide 1000",
                block: 4096,
                eq: None,
                compressor: None,
                wide: Some(1000),
                sha256: Some("1f51ca1cb2aafcf3985d858b93e55fbc3f97307f2ea5a42eafccd112d8b861f8"),
            },
            Render {
                name: "eq",
                block: 4096,
                eq: flat_with(&[(1, 1.0), (8, 0.1)]),
                compressor: None,
                wide: None,
                sha256: Some("357be7e1cec9c29c85427398178c400197519760ad81a83da2885058aeeb5e78"),
            },
            Render {
                name: "eq loud",
                block: 4096,
                eq: flat_with(&[(0, 1.0), (3, 1.0), (4, 1.0), (5, 1.0), (9, 1.0)]),
                compressor: None,
                wide: None,
                sha256: Some("6149d51bf4498ed0e8b0af0dc6bcc5737a19c34b537f5377253a21d9e91a71b1"),
            },
            Render {
                name: "eq wide",
                block: 4096,
                eq: flat_with(&[(2, 0.8), (6, 0.3)]),
                compressor: None,
                wide: Some(700),
                sha256: Some("62ce4eebe7d4bac0f7495c413f7375d7b9071cd75b1b30c3a781e7257018d136"),
            },
            Render {
                name: "eq compressor wide",
                block: 4096,
                eq: flat_with(&[(0, 0.9), (9, 0.9)]),
                compressor: Some([600, 300, 600]),
                wide: Some(400),
                sha256: Some("6c50aa05d72377d0eab68d1cc92dad2dc0d04201ac8d745d06d886de5863d864"),
            },
            Render {
                name: "compressor wide pitch",
                block: 4096,
                eq: None,
                compressor: Some([600, 300, 600]),
                wide: Some(800),
                sha256: None,
            },
            all(
                1024,
                "537b248657a97023faac456c81e9bdeb86a324cd0dff2025731c5956dba1ca8d",
            ),
            all(
                3000,
                "4a99eb4bed0d7880f13ba0b9c1d7e37bef8b18dd06a9a998ef88d75b36fe1942",
            ),
            all(
                8192,
                "537b248657a97023faac456c81e9bdeb86a324cd0dff2025731c5956dba1ca8d",
            ),
        ];
        let left = oracle_signal();
        let right: Vec<i32> = (0..left.len())
            .map(|i| left[(i + 37) % left.len()].div_euclid(2))
            .collect();
        let mut compressor = Compressor::at_load();
        for render in renders {
            // The oracle sets the gains directly; express them as saved bands.
            let mut board = Board::new();
            board.compressor = compressor;
            board.chain.booster_eq.on = render.eq.is_some();
            if let Some(gains) = render.eq {
                for (band, gain) in gains.iter().enumerate() {
                    board.equalizer.gains[band] = *gain;
                    board.equalizer.coefficients[band] =
                        eq_coefficients(band, *gain).map(|v| v as f32);
                }
            }
            if let Some([drive, speed, gain]) = render.compressor {
                board.chain.booster_compressor.on = true;
                board.compressor.clear();
                board.compressor.set(drive, speed, gain);
            }
            if let Some(spread) = render.wide {
                board.chain.booster_wide.on = true;
                board.wide.set(spread);
            }
            // The oracle renders whole blocks only.
            let frames = left.len() / render.block * render.block;
            let output: Vec<u8> = (0..frames)
                .flat_map(|i| {
                    let (l, r) = board.process_integers(left[i], right[i]);
                    [l.to_le_bytes(), r.to_le_bytes()].concat()
                })
                .collect();
            if let Some(expected) = render.sha256 {
                assert_eq!(
                    sha256_hex(output),
                    expected,
                    "{} at block {}",
                    render.name,
                    render.block
                );
            }
            compressor = board.compressor;
        }
    }

    #[test]
    fn switching_a_pedal_on_clears_its_state_and_settings_reach_live_playback() {
        let mut chain = MasterChain::default();
        chain.booster_wide.on = true;
        chain.booster_wide.spread = 1000;
        let control = MasterControl::new(&chain);
        let mut processor = MasterProcessor::live(control.clone());
        let mut block = [0.5, -0.5].repeat(64);
        processor.process(&mut block);
        assert!(processor.board.wide.buffer.iter().any(|v| *v != 0));

        chain.booster_wide.on = false;
        control.set(&chain);
        // The switch crossfades to the dry mix, which then passes untouched.
        processor.process(&mut vec![0.0; 2 * (FADE + processor.latency())]);
        let mut quiet = [0.25, 0.0].repeat(processor.latency() + 1);
        processor.process(&mut quiet);
        assert_eq!(&quiet[processor.latency() * 2..], &[0.25, 0.0]);

        chain.booster_wide.on = true;
        control.set(&chain);
        processor.process(&mut [0.0, 0.0]);
        assert_eq!(
            processor
                .board
                .wide
                .buffer
                .iter()
                .filter(|v| **v != 0)
                .count(),
            0
        );
    }

    #[test]
    fn pedals_run_in_the_chosen_order() {
        use PedalKind::*;
        let mut chain = MasterChain::default();
        chain.move_pedal(2, 0);
        assert_eq!(
            chain.order[..3],
            [BoosterWide, BoosterEq, BoosterCompressor]
        );
        chain.move_pedal(0, 2);
        assert_eq!(chain.order, PedalKind::ALL);
        chain.move_pedal(1, 1);
        chain.move_pedal(0, 13);
        assert_eq!(chain.order, PedalKind::ALL);
        chain.move_pedal(0, 11);
        assert_eq!(chain.order[11], BoosterEq);
        assert_eq!(chain.order[..2], [BoosterCompressor, BoosterWide]);
        chain.move_pedal(11, 0);
        assert_eq!(chain.order, PedalKind::ALL);

        chain.move_pedal(2, 0);
        for kind in [BoosterEq, BoosterCompressor, BoosterWide] {
            chain.set_on(kind, true);
        }
        chain.booster_eq.bands = [90, 50, 50, 70, 50, 50, 50, 50, 20, 100];
        chain.booster_compressor.drive = 800;
        chain.booster_wide.spread = 1000;
        let mut board = Board::settled(&chain);
        // The same pedals by hand: wide on integers, the equaliser on floats,
        // a truncation, then the compressor.
        let mut wide = Wide::new();
        wide.set(1000);
        let mut equalizer = Equalizer::new();
        equalizer.set(&chain.booster_eq.bands);
        let mut compressor = Compressor::at_load();
        compressor.set(800, 500, 500);
        compressor.settle();
        let signal = oracle_signal();
        let mut differs_from_default = false;
        let mut original = Board::settled(&MasterChain {
            order: PedalKind::ALL,
            ..chain
        });
        for (index, sample) in signal.iter().enumerate().take(70_000) {
            let right = signal[(index + 37) % signal.len()] / 2;
            let (l, r) = wide.process(*sample, right);
            let (l, r) = (
                equalizer.process(0, l as f32),
                equalizer.process(1, r as f32),
            );
            let (l, r) = compressor.process(l as i32, r as i32);
            let expected = (clamp16(l), clamp16(r));
            assert_eq!(
                board.process_integers(*sample, right),
                expected,
                "frame {index}"
            );
            differs_from_default |= original.process_integers(*sample, right) != expected;
        }
        assert!(differs_from_default, "The order must change the sound.");
    }
}

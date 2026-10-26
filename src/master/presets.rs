//! Factory boards for the mastering pedals.
//!
//! A preset sets every pedal: its switch and knobs, and the first order. The
//! eJay pedals are off, since a preset does not carry a song's Booster
//! settings. The trim stays as it was: it matches the board to the mix, and
//! a mix that needed -9 dB would otherwise come out 9 dB louder. Every preset
//! keeps the true peak at -1 dBTP or below through the Limiter and stays
//! moderate, so that it suits a finished mix rather than reshaping it. A
//! board is "in" a preset while its pedals equal the preset's; the song saves
//! the board, not the name.
use super::{
    BusCompressor, Imaging, LiftEq, Limiter, MasterChain, Maximizer, Multiband, SoftClip, Tape,
    TrimEq, Tube,
};
use std::sync::LazyLock;

/// A named factory board, with its trim at 0 dB.
pub struct Preset {
    pub name: &'static str,
    pub chain: MasterChain,
}
impl Preset {
    /// The preset's pedals with the trim of `board`.
    pub fn applied_to(&self, board: &MasterChain) -> MasterChain {
        MasterChain {
            trim: board.trim,
            ..self.chain
        }
    }
}

/// Three general boards, from lightest to loudest, then the genre boards.
pub static PRESETS: LazyLock<Vec<Preset>> = LazyLock::new(|| {
    vec![
        preset("Subtle", |b| {
            b.trim_eq = trim_eq(20.0, -1.0, -0.5);
            b.bus_compressor = compressor(-14.0, 1.5, 30.0, 300.0);
            b.lift_eq = lift(0.5, 0.5);
            b.limiter = limiter(2.8);
        }),
        preset("Revive", |b| {
            b.tube = tube(2.0, 40.0);
            b.trim_eq = trim_eq(20.0, -1.5, -1.0);
            b.bus_compressor = compressor(-16.0, 2.0, 10.0, 200.0);
            b.lift_eq = lift(1.0, 1.5);
            b.tape = tape(1.5, 30);
            b.imaging = imaging(110.0, 120.0);
            b.limiter = limiter(5.6);
        }),
        preset("Loud", |b| {
            b.trim_eq = trim_eq(25.0, -1.5, -1.0);
            b.bus_compressor = compressor(-18.0, 2.5, 10.0, 150.0);
            b.maximizer = maximizer(16.0);
            b.lift_eq = lift(1.0, 1.0);
            b.multiband = multiband(30.0, 25.0, 30.0);
            b.limiter = limiter(9.6);
        }),
        preset("Techno", |b| {
            b.trim_eq = trim_eq(30.0, -1.5, -1.0);
            b.bus_compressor = compressor(-16.0, 2.0, 10.0, 150.0);
            b.maximizer = maximizer(12.0);
            b.lift_eq = lift(1.0, 0.5);
            b.tape = tape(2.0, 30);
            b.imaging = imaging(105.0, 120.0);
            b.multiband = multiband(25.0, 15.0, 20.0);
            b.limiter = limiter(6.9);
        }),
        preset("Minimal", |b| {
            b.trim_eq = trim_eq(25.0, -1.0, -0.5);
            b.bus_compressor = compressor(-15.0, 1.8, 20.0, 200.0);
            b.lift_eq = lift(0.5, 1.0);
            b.imaging = imaging(105.0, 120.0);
            b.limiter = limiter(5.6);
        }),
        preset("Acid", |b| {
            b.tube = tube(3.0, 50.0);
            b.trim_eq = trim_eq(30.0, -1.5, -1.5);
            b.bus_compressor = compressor(-16.0, 2.0, 10.0, 150.0);
            b.maximizer = maximizer(10.0);
            b.lift_eq = lift(1.0, 0.5);
            b.limiter = limiter(4.8);
        }),
        preset("Hard Techno", |b| {
            b.trim_eq = trim_eq(30.0, -2.0, -1.5);
            b.bus_compressor = compressor(-18.0, 3.0, 5.0, 100.0);
            b.maximizer = maximizer(16.0);
            b.tape = tape(3.0, 15);
            b.imaging = imaging(100.0, 150.0);
            b.multiband = multiband(35.0, 25.0, 30.0);
            b.limiter = limiter(9.8);
        }),
        preset("Dub Techno", |b| {
            b.trim_eq = trim_eq(25.0, -1.0, -2.0);
            b.bus_compressor = compressor(-18.0, 1.8, 30.0, 400.0);
            b.lift_eq = lift(1.5, 0.0);
            b.tape = tape(2.5, 15);
            b.imaging = imaging(120.0, 120.0);
            b.limiter = limiter(4.3);
        }),
        preset("House", |b| {
            b.tube = tube(2.0, 40.0);
            b.trim_eq = trim_eq(25.0, -1.5, -1.0);
            b.bus_compressor = compressor(-16.0, 2.0, 10.0, 200.0);
            b.maximizer = maximizer(10.0);
            b.lift_eq = lift(1.0, 1.0);
            b.imaging = imaging(110.0, 120.0);
            b.limiter = limiter(4.8);
        }),
        preset("Deep House", |b| {
            b.trim_eq = trim_eq(25.0, -1.0, -1.5);
            b.bus_compressor = compressor(-16.0, 1.8, 20.0, 300.0);
            b.lift_eq = lift(1.5, 0.5);
            b.tape = tape(2.0, 15);
            b.imaging = imaging(110.0, 120.0);
            b.limiter = limiter(5.1);
        }),
        preset("Tech House", |b| {
            b.trim_eq = trim_eq(30.0, -1.5, -1.0);
            b.bus_compressor = compressor(-17.0, 2.2, 8.0, 150.0);
            b.maximizer = maximizer(12.0);
            b.lift_eq = lift(1.0, 1.0);
            b.imaging = imaging(105.0, 120.0);
            b.multiband = multiband(30.0, 15.0, 20.0);
            b.limiter = limiter(7.5);
        }),
        preset("Trance", |b| {
            b.trim_eq = trim_eq(25.0, -1.5, -0.5);
            b.bus_compressor = compressor(-16.0, 2.0, 10.0, 250.0);
            b.maximizer = maximizer(12.0);
            b.lift_eq = lift(1.0, 1.5);
            b.imaging = imaging(115.0, 120.0);
            b.multiband = multiband(20.0, 15.0, 15.0);
            b.limiter = limiter(6.9);
        }),
        preset("Progressive", |b| {
            b.trim_eq = trim_eq(25.0, -1.5, -0.5);
            b.bus_compressor = compressor(-15.0, 1.8, 15.0, 300.0);
            b.lift_eq = lift(0.5, 1.5);
            b.tape = tape(1.5, 30);
            b.imaging = imaging(110.0, 120.0);
            b.limiter = limiter(6.3);
        }),
        preset("Electro", |b| {
            b.tube = tube(2.5, 50.0);
            b.trim_eq = trim_eq(30.0, -1.5, -1.0);
            b.bus_compressor = compressor(-16.0, 2.5, 5.0, 120.0);
            b.maximizer = maximizer(12.0);
            b.lift_eq = lift(1.0, 1.0);
            b.imaging = imaging(105.0, 120.0);
            b.limiter = limiter(5.3);
        }),
        preset("Breakbeat", |b| {
            b.trim_eq = trim_eq(30.0, -1.0, -1.0);
            // A slow attack lets the breaks' transients through.
            b.bus_compressor = compressor(-16.0, 2.0, 20.0, 150.0);
            b.lift_eq = lift(1.0, 0.5);
            b.tape = tape(2.0, 30);
            b.limiter = limiter(6.1);
        }),
        preset("Drum & Bass", |b| {
            b.trim_eq = trim_eq(30.0, -1.5, -1.5);
            b.bus_compressor = compressor(-17.0, 2.5, 15.0, 100.0);
            b.maximizer = maximizer(14.0);
            b.lift_eq = lift(1.5, 1.0);
            b.imaging = imaging(105.0, 150.0);
            b.multiband = multiband(30.0, 20.0, 25.0);
            b.limiter = limiter(8.1);
        }),
        preset("Hardcore", |b| {
            b.soft_clip = soft_clip(2.0, -0.5);
            b.trim_eq = trim_eq(35.0, -2.0, -2.0);
            b.bus_compressor = compressor(-18.0, 3.0, 5.0, 100.0);
            b.maximizer = maximizer(16.0);
            b.imaging = imaging(100.0, 150.0);
            b.multiband = multiband(35.0, 25.0, 35.0);
            b.limiter = limiter(10.2);
        }),
        preset("Ambient", |b| {
            b.trim_eq = trim_eq(20.0, -1.0, -1.0);
            b.bus_compressor = compressor(-20.0, 1.5, 30.0, 600.0);
            b.lift_eq = lift(0.5, 1.0);
            b.tape = tape(1.0, 30);
            b.imaging = imaging(120.0, 100.0);
            b.limiter = limiter(3.1);
        }),
        preset("Downtempo", |b| {
            b.tube = tube(2.0, 40.0);
            b.trim_eq = trim_eq(20.0, -1.5, -1.5);
            b.bus_compressor = compressor(-18.0, 2.0, 20.0, 300.0);
            b.lift_eq = lift(1.0, 0.5);
            b.tape = tape(2.0, 15);
            b.imaging = imaging(110.0, 120.0);
            b.limiter = limiter(4.9);
        }),
        preset("Hip-Hop", |b| {
            b.tube = tube(2.0, 40.0);
            b.trim_eq = trim_eq(25.0, -1.5, -1.0);
            b.bus_compressor = compressor(-17.0, 2.5, 15.0, 200.0);
            b.maximizer = maximizer(12.0);
            b.lift_eq = lift(1.5, 0.5);
            b.imaging = imaging(100.0, 150.0);
            b.limiter = limiter(5.4);
        }),
    ]
});

/// The name of the preset whose pedals the board's equal, whatever its trim.
pub fn matching(chain: &MasterChain) -> Option<&'static str> {
    PRESETS
        .iter()
        .find(|preset| preset.applied_to(chain) == *chain)
        .map(|preset| preset.name)
}

fn preset(name: &'static str, change: impl FnOnce(&mut MasterChain)) -> Preset {
    let mut chain = MasterChain::default();
    change(&mut chain);
    Preset { name, chain }
}

fn soft_clip(amount: f32, ceiling: f32) -> SoftClip {
    SoftClip {
        on: true,
        amount,
        ceiling,
    }
}
fn tube(drive: f32, mix: f32) -> Tube {
    Tube {
        on: true,
        drive,
        mix,
    }
}
fn trim_eq(high_pass: f32, mud: f32, harsh: f32) -> TrimEq {
    TrimEq {
        on: true,
        high_pass,
        mud,
        harsh,
    }
}
fn compressor(threshold: f32, ratio: f32, attack: f32, release: f32) -> BusCompressor {
    BusCompressor {
        on: true,
        threshold,
        ratio,
        attack,
        release,
    }
}
fn maximizer(boost: f32) -> Maximizer {
    Maximizer { on: true, boost }
}
fn lift(low: f32, air: f32) -> LiftEq {
    LiftEq { on: true, low, air }
}
fn tape(drive: f32, speed: u8) -> Tape {
    Tape {
        on: true,
        drive,
        speed,
    }
}
fn imaging(width: f32, mono_below: f32) -> Imaging {
    Imaging {
        on: true,
        width,
        mono_below,
    }
}
fn multiband(low: f32, mid: f32, high: f32) -> Multiband {
    Multiband {
        on: true,
        low,
        mid,
        high,
    }
}
/// Every preset delivers at -1 dBTP.
fn limiter(gain: f32) -> Limiter {
    Limiter {
        on: true,
        gain,
        ceiling: -1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master::{Knob, MasterControl, MasterProcessor, PedalKind, dsp};

    #[test]
    fn presets_are_valid_whole_boards_on_the_knob_grid() {
        assert_eq!(PRESETS.len(), 20);
        let names: Vec<_> = PRESETS.iter().map(|preset| preset.name).collect();
        assert_eq!(&names[..3], ["Subtle", "Revive", "Loud"]);
        for (index, preset) in PRESETS.iter().enumerate() {
            let chain = &preset.chain;
            chain.validate().unwrap();
            assert!(
                !names[index + 1..].contains(&preset.name),
                "{}",
                preset.name
            );
            assert_eq!(chain.trim, 0.0, "{}", preset.name);
            assert_eq!(chain.order, PedalKind::ALL, "{}", preset.name);
            assert!(
                PedalKind::ALL
                    .into_iter()
                    .filter(|kind| kind.is_ejay())
                    .all(|kind| !chain.is_on(kind))
            );
            assert!(
                chain.limiter.on && chain.limiter.ceiling == -1.0,
                "{}",
                preset.name
            );
            for knob in Knob::all() {
                let range = knob.range();
                let steps = f64::from(knob.get(chain) - range.low) / f64::from(range.step);
                assert!(
                    (steps - steps.round()).abs() < 1e-3,
                    "{} {knob:?}",
                    preset.name
                );
            }
            assert_eq!(matching(chain), Some(preset.name));
        }
        assert_eq!(matching(&MasterChain::default()), None);
        // A preset keeps the trim that suits the mix, and still names the board.
        let quiet = MasterChain {
            trim: -9.0,
            ..MasterChain::default()
        };
        let loaded = PRESETS[2].applied_to(&quiet);
        assert_eq!(loaded.trim, -9.0);
        assert_eq!(
            MasterChain {
                trim: 0.0,
                ..loaded
            },
            PRESETS[2].chain
        );
        assert_eq!(matching(&loaded), Some("Loud"));
        let mut edited = loaded;
        edited.limiter.gain -= 0.1;
        assert_eq!(matching(&edited), None);
    }

    #[test]
    fn presets_stay_moderate_and_hold_the_ceiling() {
        // The test mix stands in for an eJay song, which arrives near -18 LUFS
        // with its peaks well below full scale; the hot input is 6 dB above it.
        let program = dsp::program(3 * 44_100);
        let measure = |chain: &MasterChain, gain: f32| {
            let control = MasterControl::new(chain);
            let mut processor = MasterProcessor::live(control.clone());
            let mut audio: Vec<f32> = program
                .iter()
                .flat_map(|(l, r)| [l * gain, r * gain])
                .collect();
            for block in audio.chunks_mut(1024) {
                processor.process(block);
            }
            control.meters()
        };
        let dry = measure(&MasterChain::default(), 1.0);
        let readings: Vec<_> = std::thread::scope(|scope| {
            let jobs: Vec<_> = PRESETS
                .iter()
                .map(|preset| {
                    scope.spawn(|| (measure(&preset.chain, 1.0), measure(&preset.chain, 2.0)))
                })
                .collect();
            jobs.into_iter().map(|job| job.join().unwrap()).collect()
        });
        for (preset, (usual, hot)) in PRESETS.iter().zip(&readings) {
            let name = preset.name;
            for reading in [usual, hot] {
                assert!(reading.true_peak_max <= -1.0 + 1e-3, "{name} {reading:?}");
                // Peaks keep their distance from the loudness: no preset
                // flattens the mix.
                assert!(
                    reading.true_peak_max - reading.integrated >= 7.0,
                    "{name} {reading:?}"
                );
            }
            let lift = usual.integrated - dry.integrated;
            assert!((1.0..=9.0).contains(&lift), "{name} lifts {lift} LU");
        }
        let general: Vec<f32> = readings[..3]
            .iter()
            .map(|(usual, _)| usual.integrated)
            .collect();
        assert!(
            general[0] < general[1] && general[1] < general[2],
            "{general:?}"
        );
    }
}

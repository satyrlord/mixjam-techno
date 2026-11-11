//! Clip waveforms in the arrangement. A clip shows what it plays: the slice
//! of its sound from its trim start at the song's tempo, mirrored when
//! reversed, ending where the sound ends, and shaped by its fades. The
//! columns are read from the sound's peak tables once per clip and zoom; each
//! frame draws only the columns on screen.
use super::*;
use mixjam_techno::application::{Fades, Peaks};

/// The quietest peak a clip's waveform scales up to its full height; quieter
/// sounds stay small rather than show their noise floor.
const WAVE_FLOOR: f32 = 0.05;
/// The shortest band below a clip's name that holds its waveform. A shorter
/// clip draws the waveform behind the name in a lighter tone instead.
const WAVE_BAND: f32 = 6.0;

/// What a clip's waveform depends on. Any change reads the columns again.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct ClipWaveKey {
    sample_id: u32,
    offset_frames: u32,
    length_ticks: u32,
    reverse: bool,
    fades: Fades,
    /// Points per tick and source frames per tick, as bits.
    tick_width: u32,
    frames_per_tick: u64,
    /// Catalog replacement invalidates a table even if its address is reused.
    catalog_revision: u64,
    /// The peak tables' address within this catalog.
    peaks: usize,
}

impl ClipWaveKey {
    pub(super) fn new(
        clip: &Clip,
        peaks: &Arc<Peaks>,
        catalog_revision: u64,
        tick_width: f32,
        frames_per_tick: f64,
    ) -> Self {
        Self {
            sample_id: clip.sample_id,
            offset_frames: clip.offset_frames,
            length_ticks: clip.length_ticks,
            reverse: clip.reverse,
            fades: clip.fades,
            tick_width: tick_width.to_bits(),
            frames_per_tick: frames_per_tick.to_bits(),
            catalog_revision,
            peaks: Arc::as_ptr(peaks) as usize,
        }
    }
}

/// One clip's waveform at one zoom.
pub(super) struct ClipWave {
    pub(super) key: ClipWaveKey,
    /// The one-point column, from the clip's left edge, where the sound
    /// starts: after the silence that leads a reversed clip longer than it.
    first: usize,
    /// The lowest and highest sample of each column from `first`, from -1 to
    /// 1 of the sound's own peak, after the fades.
    columns: Vec<[f32; 2]>,
}

impl ClipWave {
    pub(super) fn new(key: ClipWaveKey, peaks: &Peaks) -> Self {
        let tick_width = f64::from(f32::from_bits(key.tick_width));
        let frames_per_tick = f64::from_bits(key.frames_per_tick);
        if tick_width <= 0.0 || !frames_per_tick.is_finite() || frames_per_tick <= 0.0 {
            return Self {
                key,
                first: 0,
                columns: Vec::new(),
            };
        }
        let length = f64::from(key.length_ticks);
        let offset = f64::from(key.offset_frames);
        // The sound runs out before a long clip ends. Reversed, the clip
        // ends with the sound, so the silence comes first.
        let sound = ((peaks.frames as f64 - offset).max(0.0) / frames_per_tick).min(length);
        let (start, end) = if key.reverse {
            (length - sound, length)
        } else {
            (0.0, sound)
        };
        let first = (start * tick_width).floor() as usize;
        let last = (end * tick_width).ceil() as usize;
        let frames_per_point = frames_per_tick / tick_width;
        let scale = 1.0 / peaks.loudest().max(WAVE_FLOOR);
        let fade_in = key.fades.fade_in;
        let fade_out = key.fades.fade_out;
        let columns = (first..last)
            .map(|column| {
                let from = (column as f64 / tick_width).max(start);
                let to = ((column + 1) as f64 / tick_width).min(end);
                let (a, b) = if key.reverse {
                    (length - to, length - from)
                } else {
                    (from, to)
                };
                let (low, high) = peaks.range(
                    offset + a * frames_per_tick,
                    offset + b * frames_per_tick,
                    frames_per_point,
                );
                // The fade gains at the column's middle, as playback applies
                // them: the fade in from the clip's start, the fade out to the
                // sound's end.
                let middle = (from + to) / 2.0;
                let mut gain = 1.0;
                if fade_in.ticks > 0 && middle < f64::from(fade_in.ticks) {
                    gain *= fade_in
                        .curve
                        .gain((middle / f64::from(fade_in.ticks)) as f32);
                }
                let out_start = end - f64::from(fade_out.ticks);
                if fade_out.ticks > 0 && middle > out_start {
                    gain *= fade_out
                        .curve
                        .gain(((end - middle) / f64::from(fade_out.ticks)) as f32);
                }
                let gain = gain * scale;
                [
                    (low * gain).clamp(-1.0, 1.0),
                    (high * gain).clamp(-1.0, 1.0),
                ]
            })
            .collect();
        Self {
            key,
            first,
            columns,
        }
    }

    /// Paint the columns that `painter` shows inside `area`, one point each
    /// from `left`, centred on the area's middle.
    fn paint(&self, painter: &egui::Painter, area: Rect, left: f32, color: Color32) {
        let visible = painter.clip_rect().intersect(area);
        if !visible.is_positive() || self.columns.is_empty() {
            return;
        }
        let from = ((visible.left() - left).floor().max(0.0) as usize).max(self.first);
        let to = ((visible.right() - left).ceil().max(0.0) as usize)
            .min(self.first + self.columns.len());
        if from >= to {
            return;
        }
        let middle = area.center().y;
        let half = area.height() / 2.0;
        let mut mesh = egui::Mesh::default();
        mesh.reserve_vertices(2 * (to - from + 1));
        mesh.reserve_triangles(2 * (to - from));
        for column in from..to {
            let [low, high] = self.columns[column - self.first];
            let top = middle - high * half;
            let bottom = (middle - low * half).max(top + 1.0);
            // Each column's edges at its middle, with the first and last
            // reaching their outer edges.
            let x = left
                + column as f32
                + if column == self.first {
                    0.0
                } else if column + 1 == self.first + self.columns.len() {
                    1.0
                } else {
                    0.5
                };
            let index = mesh.vertices.len() as u32;
            mesh.colored_vertex(Pos2::new(x, top), color);
            mesh.colored_vertex(Pos2::new(x, bottom), color);
            if column > from {
                mesh.add_triangle(index - 2, index - 1, index);
                mesh.add_triangle(index - 1, index + 1, index);
            }
        }
        if to - from == 1 {
            // A single column still covers its point.
            let [top, bottom] = [mesh.vertices[0].pos, mesh.vertices[1].pos];
            mesh.add_colored_rect(
                Rect::from_min_max(top, Pos2::new(bottom.x + 1.0, bottom.y)),
                color,
            );
        }
        painter.add(mesh);
    }
}

/// A darker tone of a category color, for a waveform over its fill.
fn tone(color: Color32, keep: f32) -> Color32 {
    let scale = |channel: u8| (f32::from(channel) * keep).round() as u8;
    Color32::from_rgb(scale(color.r()), scale(color.g()), scale(color.b()))
}

/// Paint a clip's waveform below its name. `name_bottom` is where the name's
/// text ends. A lane too short for a band below the name draws the waveform
/// behind it in a lighter tone, so the name stays readable.
pub(super) fn paint_clip_wave(
    painter: &egui::Painter,
    rect: Rect,
    name_bottom: f32,
    wave: &ClipWave,
    category: Color32,
) {
    let inner = rect.shrink2(Vec2::new(2.0, 2.0));
    let band = Rect::from_min_max(Pos2::new(inner.left(), name_bottom), inner.max);
    let (area, color) = if band.height() >= WAVE_BAND {
        (band, tone(category, 0.55))
    } else {
        (inner.shrink2(Vec2::new(0.0, 1.0)), tone(category, 0.82))
    };
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(inner));
    wave.paint(&clipped, area, rect.left(), color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mixjam_techno::application::{Fade, FadeCurve, Pcm};

    /// One second of a rising ramp: each frame is louder than the one before.
    fn ramp() -> Arc<Peaks> {
        let frames = 44_100;
        Arc::new(Peaks::from_pcm(&Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..frames).map(|f| f as f32 / frames as f32).collect(),
        }))
    }

    fn clip(length_ticks: u32, offset_frames: u32, reverse: bool) -> Clip {
        Clip {
            id: 1,
            sample_id: 1,
            track: 0,
            start_tick: 0,
            length_ticks,
            offset_frames,
            gain: 1.0,
            fades: Fades::default(),
            pitch_cents: 0,
            reverse,
        }
    }

    /// 100 source frames per tick and two points per tick.
    fn wave(clip: &Clip, peaks: &Arc<Peaks>) -> ClipWave {
        ClipWave::new(ClipWaveKey::new(clip, peaks, 0, 2.0, 100.0), peaks)
    }

    #[test]
    fn a_new_catalog_rebuilds_a_waveform_when_the_peak_address_is_reused() {
        let clip = clip(200, 0, false);
        let mut peaks = ramp();
        let address = Arc::as_ptr(&peaks);
        let mut cached = wave(&clip, &peaks);
        // Replace the table in place to model a new catalog whose allocation
        // reuses the old table's address, without relying on allocator behavior.
        *Arc::get_mut(&mut peaks).unwrap() = Peaks::from_pcm(&Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![-0.5; 44_100],
        });
        assert_eq!(Arc::as_ptr(&peaks), address);
        let key = ClipWaveKey::new(&clip, &peaks, 1, 2.0, 100.0);
        if cached.key != key {
            cached = ClipWave::new(key, &peaks);
        }
        assert!(
            cached.columns.iter().all(|[_, high]| *high < 0.0),
            "The new catalog's negative waveform must replace the old positive waveform."
        );
    }

    #[test]
    fn a_waveform_starts_at_the_trim_and_ends_with_the_sound() {
        let peaks = ramp();
        // 441 ticks of sound; the clip is 600 ticks long.
        let whole = wave(&clip(600, 0, false), &peaks);
        assert_eq!(whole.first, 0);
        assert_eq!(whole.columns.len(), 882);
        let trimmed = wave(&clip(600, 22_050, false), &peaks);
        assert_eq!(trimmed.columns.len(), 441);
        // The trimmed clip starts halfway up the ramp.
        assert!((trimmed.columns[0][1] - 0.5).abs() < 0.02);
        assert!(whole.columns[0][1] < 0.02);
    }

    #[test]
    fn a_reversed_waveform_mirrors_the_slice_and_leads_with_silence() {
        let peaks = ramp();
        let forward = wave(&clip(200, 0, false), &peaks);
        let reversed = wave(&clip(200, 0, true), &peaks);
        assert_eq!(forward.columns.len(), reversed.columns.len());
        let count = forward.columns.len();
        for column in [0, 100, count - 1] {
            let mirror = count - 1 - column;
            assert!(
                (forward.columns[column][1] - reversed.columns[mirror][1]).abs() < 0.01,
                "{column}"
            );
        }
        // A reversed clip longer than its sound ends with it.
        let long = wave(&clip(600, 0, true), &peaks);
        assert_eq!(long.first, 2 * (600 - 441));
        assert_eq!(long.first + long.columns.len(), 1200);
        assert!(long.columns[0][1] > 0.95);
    }

    #[test]
    fn fades_shape_the_waveform_to_silence_at_their_ends() {
        let peaks = Arc::new(Peaks::from_pcm(&Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 44_100],
        }));
        let mut faded = clip(400, 0, false);
        faded.fades = Fades {
            fade_in: Fade {
                ticks: 100,
                curve: FadeCurve::Linear,
            },
            fade_out: Fade {
                ticks: 100,
                curve: FadeCurve::Linear,
            },
        };
        let wave = wave(&faded, &peaks);
        let heights: Vec<f32> = wave.columns.iter().map(|[_, high]| *high).collect();
        assert!(heights[0] < 0.01);
        assert!((heights[100] - 0.5).abs() < 0.01, "{}", heights[100]);
        assert!((heights[400] - 1.0).abs() < 0.01);
        assert!(heights[heights.len() - 1] < 0.01);
    }
}

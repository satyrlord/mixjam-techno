//! Painting a clip: its body, fades, gain line, handles, and labels, the
//! readouts of a handle drag, and the Step synth patterns that a clip draws as
//! notes instead of a waveform.
use super::*;

/// What the score's clips paint, kept between frames.
#[derive(Default)]
pub(in super::super) struct ClipPaintCache {
    /// Each clip's waveform at the zoom it was last drawn at.
    pub(in super::super) waves: std::collections::HashMap<u64, waveform::ClipWave>,
    /// The pattern saved beside each Step synth sound the song uses, or None
    /// when it has none, read once per catalog revision.
    pub(in super::super) step_patterns: std::collections::HashMap<u32, Option<Arc<StepSynthState>>>,
    pub(in super::super) step_patterns_revision: u64,
}

impl MixJamApp {
    /// Read the pattern beside each Step synth sound the song's clips use, once
    /// per sound and catalog revision. A sound without one keeps its waveform;
    /// a pattern that cannot be read says so once in the status line.
    pub(in super::super) fn load_step_patterns(&mut self) {
        let revision = self.session.catalog_revision();
        if revision != self.clip_paint.step_patterns_revision {
            self.clip_paint.step_patterns.clear();
            self.clip_paint.step_patterns_revision = revision;
        }
        let wanted: BTreeSet<u32> = self
            .session
            .project()
            .clips
            .iter()
            .map(|clip| clip.sample_id)
            .filter(|id| !self.clip_paint.step_patterns.contains_key(id))
            .collect();
        let mut failed = None;
        for id in wanted {
            let sample = self.session.library().sample_by_id(id);
            let pattern = sample
                .filter(|sample| sample.group_name == "Step synth")
                .and_then(|sample| sample.wav_path.as_deref())
                .map(mixjam_techno::application::pattern_beside)
                .filter(|path| path.is_file())
                .and_then(|path| match self.session.load_pattern(&path) {
                    Ok(pattern) => Some(Arc::new(pattern)),
                    Err(error) => {
                        let name = sample.map_or("A Step synth sound", |sample| &sample.name);
                        failed.get_or_insert(format!(
                            "{name} draws its waveform: its pattern cannot be read ({error})."
                        ));
                        None
                    }
                });
            self.clip_paint.step_patterns.insert(id, pattern);
        }
        if let Some(text) = failed {
            self.error(text);
        }
    }
}

/// Mark a clip that plays backwards: "rev" in its bottom right corner. Over
/// a waveform, a `plate` of the clip's fill keeps it readable.
fn draw_reverse_mark(painter: &egui::Painter, rect: Rect, plate: Option<Color32>) -> Option<Rect> {
    // The shortest lane height still shows it.
    if rect.width() < 36.0 || rect.height() < 16.0 {
        return None;
    }
    let painter = painter.with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)));
    let galley = painter.layout_no_wrap("rev".into(), FontId::proportional(10.0), roles().on_light);
    let mark = Align2::RIGHT_BOTTOM
        .anchor_size(rect.right_bottom() + Vec2::new(-6.0, -3.0), galley.size());
    if let Some(fill) = plate {
        painter.rect_filled(mark.expand2(Vec2::new(2.0, 0.0)), 2.0, fill);
    }
    painter.galley(mark.min, galley, roles().on_light);
    Some(mark)
}

/// Shade a clip's fades above their curves. The fade out ends where the
/// clip's sound ends, which can be before the clip's end.
pub(super) fn draw_fades(
    painter: &egui::Painter,
    rect: Rect,
    clip: &Clip,
    sample: Option<&Sample>,
    bar_width: f32,
    bpm: f64,
) {
    if clip.fades.is_none() {
        return;
    }
    let tick_width = bar_width / TICKS_PER_BAR as f32;
    let sound_end = sample.map_or(rect.right(), |sample| {
        (rect.left() + sound_ticks(clip, sample, bpm) as f32 * tick_width).min(rect.right())
    });
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(rect));
    let fade_in = clip.fades.fade_in;
    let fade_out = clip.fades.fade_out;
    let spans = [
        (
            rect.left(),
            rect.left() + fade_in.ticks as f32 * tick_width,
            fade_in,
            true,
        ),
        (
            sound_end - fade_out.ticks as f32 * tick_width,
            sound_end,
            fade_out,
            false,
        ),
    ];
    for (from, to, fade, rising) in spans {
        if fade.ticks == 0 || to <= from {
            continue;
        }
        const STEPS: usize = 24;
        let points: Vec<Pos2> = (0..=STEPS)
            .map(|step| {
                let t = step as f32 / STEPS as f32;
                let gain = fade.curve.gain(if rising { t } else { 1.0 - t });
                Pos2::new(from + (to - from) * t, rect.bottom() - gain * rect.height())
            })
            .collect();
        let mut mesh = egui::Mesh::default();
        let shade = Color32::from_black_alpha(110);
        for pair in points.windows(2) {
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(Pos2::new(pair[0].x, rect.top()), shade);
            mesh.colored_vertex(pair[0], shade);
            mesh.colored_vertex(Pos2::new(pair[1].x, rect.top()), shade);
            mesh.colored_vertex(pair[1], shade);
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 2);
        }
        clipped.add(egui::Shape::mesh(mesh));
        clipped.add(egui::Shape::line(
            points,
            Stroke::new(1.0_f32, Color32::from_black_alpha(190)),
        ));
    }
}

/// Source frames in one tick at the project's effective source tempo.
pub(super) fn frames_per_tick(sample: &Sample, bpm: f64) -> f64 {
    f64::from(sample.sample_rate) * 240.0 / bpm / f64::from(TICKS_PER_BAR)
}

/// The ticks from a clip's start to where its sound ends, at most its length.
pub(super) fn sound_ticks(clip: &Clip, sample: &Sample, bpm: f64) -> u32 {
    // Reversed, the sound ends with the clip.
    if sample.sample_rate == 0 || clip.reverse {
        return clip.length_ticks;
    }
    let frames = f64::from(sample.frame_count.saturating_sub(clip.offset_frames));
    ((frames / frames_per_tick(sample, bpm)) as u32).min(clip.length_ticks)
}

/// The height of a clip's gain line: unity in the middle, +6 dB at the top.
fn gain_y(rect: Rect, gain: f32) -> f32 {
    rect.bottom() - (gain.min(2.0) / 2.0 * rect.height()).max(2.0)
}

/// The centers of a clip's fade handles on its top edge: where the fade in
/// ends and where the fade out starts. A handle without a fade sits in its corner.
fn fade_handles(rect: Rect, clip: &Clip, sample: &Sample, bar_width: f32, bpm: f64) -> [Pos2; 2] {
    let tick_width = bar_width / TICKS_PER_BAR as f32;
    let sound_end =
        (rect.left() + sound_ticks(clip, sample, bpm) as f32 * tick_width).min(rect.right());
    let inset = FADE_HANDLE / 2.0;
    let place = |x: f32| {
        Pos2::new(
            x.clamp(
                rect.left() + inset,
                (rect.right() - inset).max(rect.left() + inset),
            ),
            rect.top() + inset + 1.0,
        )
    };
    [
        place(rect.left() + clip.fades.fade_in.ticks as f32 * tick_width),
        place(sound_end - clip.fades.fade_out.ticks as f32 * tick_width),
    ]
}

/// The side of a fade handle's square.
const FADE_HANDLE: f32 = 9.0;

/// The vertical travel of one gain step: one dB, or a tenth with Ctrl.
pub(super) const GAIN_PIXELS: f32 = 4.0;

/// The size of the gain handle.
const GAIN_HANDLE: Vec2 = Vec2::new(16.0, 8.0);

/// The center of a clip's gain handle: on the gain line, in the middle of
/// the part of the clip in view.
pub(in super::super) fn gain_handle(rect: Rect, visible: Rect, gain: f32) -> Pos2 {
    let shown = rect.intersect(visible);
    let x = if shown.width() > 0.0 {
        shown.center().x
    } else {
        rect.center().x
    };
    Pos2::new(x, gain_y(rect, gain))
}

/// The handle under `pos` in a clip's rectangle. The fade handles sit on the
/// top edge, the trims take the side edges, and the gain handle sits on the
/// gain line in the middle of the part in `visible`. The rest moves the clip.
pub(super) fn handle_at(
    pos: Pos2,
    rect: Rect,
    visible: Rect,
    clip: &Clip,
    sample: &Sample,
    bar_width: f32,
    bpm: f64,
) -> Option<ClipHandle> {
    // A narrow clip is only for moving.
    if rect.width() < 16.0 {
        return None;
    }
    // The gain handle is painted last. It must receive input where it
    // covers a fade handle or a trim edge.
    if Rect::from_center_size(
        gain_handle(rect, visible, clip.gain),
        GAIN_HANDLE + Vec2::splat(2.0),
    )
    .contains(pos)
    {
        return Some(ClipHandle::Gain);
    }
    if pos.y <= rect.top() + 12.0 {
        let [fade_in, fade_out] = fade_handles(rect, clip, sample, bar_width, bpm);
        let (to_in, to_out) = ((pos.x - fade_in.x).abs(), (pos.x - fade_out.x).abs());
        if to_in.min(to_out) <= 7.0 {
            return Some(if to_in <= to_out {
                ClipHandle::FadeIn
            } else {
                ClipHandle::FadeOut
            });
        }
    }
    if pos.x <= rect.left() + 5.0 {
        Some(ClipHandle::Start)
    } else if pos.x >= rect.right() - 5.0 {
        Some(ClipHandle::End)
    } else {
        None
    }
}

/// The gain handle of a hovered clip, which drags the gain line.
pub(super) fn draw_gain_handle(painter: &egui::Painter, rect: Rect, visible: Rect, gain: f32) {
    if rect.width() < 16.0 {
        return;
    }
    let handle = Rect::from_center_size(gain_handle(rect, visible, gain), GAIN_HANDLE);
    painter.rect_filled(handle, 2.0, roles().on_light);
    painter.rect_stroke(
        handle,
        2.0,
        Stroke::new(1.0_f32, roles().text),
        egui::StrokeKind::Inside,
    );
}

pub(super) fn draw_fade_handles(
    painter: &egui::Painter,
    rect: Rect,
    clip: &Clip,
    sample: &Sample,
    bar_width: f32,
    bpm: f64,
) {
    if rect.width() < 16.0 {
        return;
    }
    // A light rim keeps the dark squares visible on fills and fade shading.
    for center in fade_handles(rect, clip, sample, bar_width, bpm) {
        let square = Rect::from_center_size(center, Vec2::splat(FADE_HANDLE));
        painter.rect_filled(square, 1.5, roles().on_light);
        painter.rect_stroke(
            square,
            1.5,
            Stroke::new(1.0_f32, roles().text),
            egui::StrokeKind::Inside,
        );
    }
}

/// What a handle drag changes, for the label at the pointer.
pub(super) fn handle_readout(handle: ClipHandle, clip: &Clip) -> String {
    let beats = |ticks: u32| f64::from(ticks) / f64::from(TICKS_PER_BEAT);
    match handle {
        ClipHandle::Start => format!(
            "Start: bar {:.2}",
            f64::from(clip.start_tick) / f64::from(TICKS_PER_BAR) + 1.0
        ),
        ClipHandle::End => format!(
            "Length: {:.2} bars",
            f64::from(clip.length_ticks) / f64::from(TICKS_PER_BAR)
        ),
        ClipHandle::FadeIn => format!("Fade in: {:.2} beats", beats(clip.fades.fade_in.ticks)),
        ClipHandle::FadeOut => format!("Fade out: {:.2} beats", beats(clip.fades.fade_out.ticks)),
        ClipHandle::Gain if clip.gain == 0.0 => "Gain: -inf dB".into(),
        ClipHandle::Gain => format!("Gain: {:+.1} dB", clip_gain_db(clip.gain)),
    }
}

/// A clip: its category fill, waveform, name, and marks. A missing sound
/// has no waveform.
pub(in super::super) fn draw_clip(
    painter: &egui::Painter,
    rect: Rect,
    sample: Option<&Sample>,
    missing: Option<&MissingSample>,
    selected: bool,
    reverse: bool,
    body: Option<waveform::ClipBody<'_>>,
) {
    let color = if missing.is_some() {
        roles().raised
    } else {
        sample
            .map(|s| group_color(&s.group_name))
            .unwrap_or(roles().muted)
    };
    // The clip's silhouette: a tight tab on Studio, a rounded bubble on Aero,
    // and a painted label on Vintage. Category colors stay the same.
    let radius = clip_radius(rect);
    theme::surface(painter, rect, radius, color);
    if missing.is_some() {
        painter.rect_stroke(
            rect.shrink(0.5),
            radius,
            Stroke::new(1.0_f32, roles().coral),
            egui::StrokeKind::Inside,
        );
    }
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)));
    let label = missing
        .map(|missing| format!("Missing: {}", missing.name))
        .or_else(|| sample.map(|sample| sample.name.clone()));
    let text_color = if missing.is_some() {
        roles().text
    } else {
        roles().on_light
    };
    let size = if rect.height() < 28.0 { 10.0 } else { 12.0 };
    // The 20-point lane leaves no room below the name at the usual inset.
    let top = if rect.height() < 20.0 { 2.0 } else { 4.0 };
    let at = rect.left_top() + Vec2::new(6.0, top);
    let text = label.as_ref().map(|label| {
        clipped.layout_job(elided(
            label,
            size,
            text_color,
            rect.width() - 12.0,
            clip_label_rows(rect.height()),
        ))
    });
    let name_bottom = text
        .as_ref()
        .map_or(rect.top(), |text| at.y + text.size().y);
    // A Step synth clip with a known pattern shows notes, any other its waveform.
    let body = body.filter(|_| missing.is_none());
    match body {
        Some(waveform::ClipBody::Notes(notes, tick_width)) => {
            notes.paint(painter, rect, name_bottom, tick_width, color);
        }
        Some(waveform::ClipBody::Wave(wave)) => {
            waveform::paint_clip_wave(painter, rect, name_bottom, wave, color);
        }
        None => {}
    }
    if selected {
        painter.rect_stroke(
            rect.shrink(0.5),
            radius,
            Stroke::new(2.0_f32, roles().text),
            egui::StrokeKind::Inside,
        );
        // A dark inner line keeps the selection visible on every category fill.
        painter.rect_stroke(
            rect.shrink(2.5),
            (radius - 1.0).max(1.0),
            Stroke::new(1.0_f32, roles().on_light),
            egui::StrokeKind::Inside,
        );
    }
    let reverse_mark = reverse
        .then(|| draw_reverse_mark(painter, rect, body.map(|_| color)))
        .flatten();
    if let (Some(label), Some(mut text)) = (label, text) {
        let mut label_painter = clipped;
        if let Some(mark) = reverse_mark
            && text.mesh_bounds.translate(at.to_vec2()).intersects(mark)
        {
            let right = mark.left() - 4.0;
            text = label_painter.layout_job(elided(
                &label,
                size,
                text_color,
                right - at.x,
                clip_label_rows(rect.height()),
            ));
            let mut clip = label_painter.clip_rect();
            clip.max.x = clip.max.x.min(right);
            label_painter = label_painter.with_clip_rect(clip);
        }
        label_painter.galley(at, text, text_color);
    }
}

/// The corner radius of a clip in the current theme. Aero's bubble stays
/// small enough that a clip's label clears its curve.
pub(in super::super) fn clip_radius(rect: Rect) -> f32 {
    match roles().material {
        theme::Material::Flat => 2.0,
        theme::Material::Plastic => (rect.height() / 2.0).min(8.0),
        theme::Material::Metal => 3.0,
    }
}

/// The gain line of a selected or hovered clip. Its handle drags it.
pub(super) fn draw_gain_line(painter: &egui::Painter, rect: Rect, gain: f32) {
    let y = gain_y(rect, gain);
    painter
        .with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)))
        .line_segment(
            [
                Pos2::new(rect.left() + 4.0, y),
                Pos2::new(rect.right() - 4.0, y),
            ],
            Stroke::new(1.5_f32, Color32::from_black_alpha(170)),
        );
}

pub(in super::super) fn missing_clip_details(clip: &Clip) -> String {
    format!(
        "Track {}: start {} ticks; length {} ticks ({:.3} bars); source offset {} frames; gain {}.",
        clip.track + 1,
        clip.start_tick,
        clip.length_ticks,
        clip.length_ticks as f64 / TICKS_PER_BAR as f64,
        clip.offset_frames,
        clip.gain
    )
}

pub(in super::super) fn missing_sample_details(missing: &MissingSample) -> String {
    let mut rows = vec![
        format!("Missing: {}", missing.name),
        "Playback: silence".into(),
        format!("Reason: {}", missing.reason),
        format!("Project sample ID: {}", missing.id),
    ];
    match &missing.reference {
        MissingReference::Native { product, native_id } => {
            rows.push(format!("Original product: {}", product.label()));
            rows.push(format!("Native sample ID: {native_id}"));
        }
        MissingReference::External { path, origin } => {
            rows.push(format!("Original path: {}", path.display()));
            if let Some(product) = origin {
                rows.push(format!("Original product: {}", product.label()));
            }
        }
        MissingReference::Generated {
            product,
            kind,
            data,
        } => {
            rows.push(format!("Original product: {}", product.label()));
            rows.push(format!("Generator: {kind}"));
            rows.push(format!("Generator data: {data}"));
        }
    }
    rows.push(format!("Source tempo: {} BPM", missing.source_bpm));
    rows.push(format!("Trim reference rate: {} Hz", missing.sample_rate));
    rows.push(format!(
        "Source length: {} ticks ({:.3} bars; {})",
        missing.nominal_ticks,
        missing.nominal_ticks as f64 / TICKS_PER_BAR as f64,
        if missing.length_inferred {
            "inferred"
        } else {
            "known"
        }
    ));
    for (name, value) in &missing.metadata {
        rows.push(format!("{name}: {value}"));
    }
    display_paths(&rows.join("\n"))
}

/// A clip shows its name on one line below 38 points of height, else on two.
pub(in super::super) fn clip_label_rows(height: f32) -> usize {
    if height < 38.0 { 1 } else { 2 }
}

/// Text limited to `rows` lines of `width`. The measured glyphs decide where
/// it ends, and an ellipsis on the last visible line marks a shortened name.
pub(in super::super) fn elided(
    text: &str,
    size: f32,
    color: Color32,
    width: f32,
    rows: usize,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        FontId::proportional(size),
        color,
        width.max(1.0),
    );
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = rows == 1;
    job
}

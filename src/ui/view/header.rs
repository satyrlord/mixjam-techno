//! The top bar: the brand, the song name, the transport buttons, and the
//! position readout, packed tighter in a compact window.
use super::*;

/// The song name's size at the left of the transport, and the floor that it
/// shrinks to before an absurdly long name goes below it.
const TRANSPORT_TITLE_SIZE: f32 = 15.0;

const TRANSPORT_TITLE_FLOOR: f32 = 9.0;

/// The transport row inside the panel's 10-point vertical margins.
const TRANSPORT_ROW_HEIGHT: f32 = 48.0;

/// Transport buttons are square. In a compact window they narrow when the song
/// name cannot otherwise keep its full size.
const TRANSPORT_BUTTON: f32 = 42.0;

const TRANSPORT_BUTTON_NARROW: f32 = 34.0;

const TRANSPORT_BUTTON_COUNT: f32 = 8.0;

/// The position readout's container in the top bar, and in a narrow window.
const POSITION_SIZE: Vec2 = Vec2::new(112.0, 42.0);

const POSITION_SIZE_COMPACT: Vec2 = Vec2::new(76.0, 40.0);

/// The top bar: the transport row inside its vertical margins.
const TOP_BAR_MARGIN: i8 = 8;

pub(in super::super) const TOP_BAR_HEIGHT: f32 = TRANSPORT_ROW_HEIGHT + 2.0 * TOP_BAR_MARGIN as f32;

/// Below this width the top bar packs tighter: a stacked brand, narrower
/// buttons, and no bar and beat beside the position.
const TOP_BAR_COMPACT_WIDTH: f32 = 1240.0;

impl MixJamApp {
    /// The top bar across the window: the brand, the song name, the
    /// transport, the recording track, the position, the tempo, and Undo,
    /// Redo and Help at the right end.
    pub(in super::super) fn header(&mut self, ctx: &egui::Context) {
        let icons = self.icons(ctx);
        egui::TopBottomPanel::top("header")
            .exact_height(TOP_BAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(roles().canvas)
                    .inner_margin(egui::Margin::symmetric(18, TOP_BAR_MARGIN)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let compact = ui.available_width() < TOP_BAR_COMPACT_WIDTH;
                    ui.spacing_mut().item_spacing.x = if compact { 4.0 } else { 6.0 };
                    if let Some(icons) = &icons {
                        icons.brand_image(ui);
                    }
                    stacked_brand(ui, compact);
                    // A narrow bar gives the separator's room to the name.
                    if compact {
                        ui.add_space(6.0);
                    } else {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(2.0);
                    }
                    self.transport(ui, icons.as_deref(), compact);
                });
            });
    }

    pub(in super::super) fn transport(
        &mut self,
        ui: &mut egui::Ui,
        icons: Option<&RailIcons>,
        compact: bool,
    ) {
        // The controls after the name keep their own widths: the name gets
        // the rest of the row, measured in the previous pass, and is never
        // clipped. Its slot keeps room for the unsaved mark, so the first
        // edit does not move the controls.
        let rest_id = egui::Id::new(("transport-rest-width", compact));
        let rest = ui.ctx().data(|data| data.get_temp::<f32>(rest_id));
        let available = ui.available_width();
        let room =
            |button: f32| available - rest.unwrap_or_default() - TRANSPORT_BUTTON_COUNT * button;
        let title = &self.session.project().title;
        let sizing = format!("{title} *");
        let mut button = TRANSPORT_BUTTON;
        let mut fit = transport_title_fit(ui, &sizing, room(button));
        if compact && fit.size < TRANSPORT_TITLE_SIZE {
            button = TRANSPORT_BUTTON_NARROW;
            fit = transport_title_fit(ui, &sizing, room(button));
        }
        let width = room(button);
        let slot = transport_title_galley(ui, &sizing, fit, width).size();
        let shown = if self.session.unsaved() {
            sizing.clone()
        } else {
            title.clone()
        };
        // The saved name lays out in the slot sized for the marked one: alone
        // it could fit one row wider than the slot.
        let galley = transport_title_galley(ui, &shown, fit, (slot.x + 0.5).min(width));
        // Under a name on one line at full size, the song's length, where it
        // fits the name's room.
        let meta = (fit.size >= TRANSPORT_TITLE_SIZE && galley.rows.len() == 1)
            .then(|| song_meta(self.session.project()))
            .flatten()
            .map(|text| {
                ui.painter()
                    .layout_no_wrap(text, FontId::proportional(11.0), roles().muted)
            })
            .filter(|meta| meta.size().x <= slot.x);
        let block = galley.size().y + meta.as_ref().map_or(0.0, |meta| meta.size().y + 1.0);
        let (name, response) =
            ui.allocate_exact_size(Vec2::new(slot.x, slot.y.max(30.0)), Sense::hover());
        let top = name.center().y - block / 2.0;
        let title_height = galley.size().y;
        ui.painter()
            .galley(Pos2::new(name.left(), top), galley, roles().text);
        if let Some(meta) = meta {
            ui.painter().galley(
                Pos2::new(name.left(), top + title_height + 1.0),
                meta,
                roles().muted,
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title_text(&shown))
        });
        ui.add_space(if compact { 4.0 } else { 12.0 });
        let sections = self.sections.bookmarks && !self.session.project().sections.is_empty();
        if transport_button(
            ui,
            button,
            TransportIcon::Loop,
            self.session.project().loop_region.is_some(),
            if sections {
                "Loop the section at the playhead. Drag the bar ruler to loop a range."
            } else {
                "Loop the whole song. Drag the bar ruler to loop a range."
            },
        )
        .clicked()
        {
            let project = self.session.project();
            let region = if project.loop_region.is_some() {
                None
            } else if sections {
                project.section_span(self.session.cursor_tick())
            } else {
                Some([0, project.end_tick().max(TICKS_PER_BAR)])
            };
            let result = self.session.edit(EditCommand::SetLoop(region));
            self.apply(result);
        }
        if transport_button(ui, button, TransportIcon::Start, false, "Go to start").clicked() {
            self.seek(0);
        }
        if transport_button(ui, button, TransportIcon::Back, false, "Back one bar").clicked() {
            self.seek(self.session.cursor_tick().saturating_sub(TICKS_PER_BAR));
        }
        if transport_button(ui, button, TransportIcon::Stop, false, "Stop").clicked() {
            self.stop();
        }
        let play = transport_button(
            ui,
            button,
            if self.session.wants_playback() {
                TransportIcon::Pause
            } else {
                TransportIcon::Play
            },
            self.session.wants_playback(),
            "Play / pause (Space)",
        );
        if self.capture.transport_focus {
            play.request_focus();
            self.capture.focus_ready = play.has_focus();
            if !self.capture.focus_ready {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        if play.clicked() {
            if self.session.wants_playback() {
                self.pause();
            } else {
                self.start_playback();
            }
        }
        if transport_button(ui, button, TransportIcon::Forward, false, "Forward one bar").clicked()
        {
            self.seek(self.session.cursor_tick().saturating_add(TICKS_PER_BAR));
        }
        if transport_button(ui, button, TransportIcon::End, false, "Go to end").clicked() {
            self.seek(self.session.project().end_tick());
        }
        if transport_button(
            ui,
            button,
            TransportIcon::Record,
            self.session.is_recording(),
            "Record into the armed track",
        )
        .clicked()
        {
            if self.session.is_recording() {
                self.stop();
            } else {
                self.start_recording();
            }
        }
        // The recording track sits beside Record.
        if let Some(icons) = icons {
            icon_heading(
                ui,
                icons,
                RailIcon::Record,
                if compact { 18.0 } else { 24.0 },
            );
        }
        let occupied = occupied_tracks(self.session.project());
        recording_destination(ui, &mut self.recording.track, &occupied);
        ui.add_space(if compact { 2.0 } else { 10.0 });
        transport_position(
            ui,
            if compact {
                POSITION_SIZE_COMPACT
            } else {
                POSITION_SIZE
            },
            self.session.cursor_tick(),
            self.session.project().bpm,
            !compact,
        );
        ui.add_space(if compact { 2.0 } else { 10.0 });
        ui.label(egui::RichText::new("BPM").color(roles().muted));
        let mut bpm = self.session.project().bpm;
        if name_control(
            bpm_picker(
                ui,
                "Project BPM",
                &mut bpm,
                if compact { 64.0 } else { 88.0 },
            ),
            "Project BPM",
        )
        .changed()
        {
            let result = self.session.edit(EditCommand::SetTempo(bpm));
            self.apply(result);
        }
        // Undo, Redo and Help against the right edge.
        let action_width = if compact { 32.0 } else { 40.0 };
        let help_gap = if compact { 4.0 } else { 8.0 };
        let spacing = ui.spacing().item_spacing.x;
        let actions = 3.0 * action_width + 2.0 * spacing + help_gap;
        // Remember what follows the name, less the transport buttons. A
        // changed width redoes this pass before it is shown.
        let measured = ui.min_rect().right() - name.right() - TRANSPORT_BUTTON_COUNT * button
            + spacing
            + actions;
        ui.add_space((ui.available_width() - actions).max(0.0));
        let (can_undo, can_redo) = (
            self.session.has_history_step(false),
            self.session.has_history_step(true),
        );
        let action = |ui: &mut egui::Ui, icon: RailIcon, label: &str, enabled: bool| match icons {
            Some(icons) => {
                action_icon_button(ui, icons, icon, label, enabled, icon.hint(), action_width)
            }
            None => action_button(ui, label, false).hover_hint(label),
        };
        if action(ui, RailIcon::Undo, "Undo", can_undo).clicked() {
            self.history_step(false);
        }
        if action(ui, RailIcon::Redo, "Redo", can_redo).clicked() {
            self.history_step(true);
        }
        ui.add_space(help_gap);
        if action(ui, RailIcon::Help, "Help", true).clicked() {
            self.help.main = !self.help.main;
        }
        if rest.is_none_or(|rest| (rest - measured).abs() > 0.5) {
            ui.ctx()
                .data_mut(|data| data.insert_temp(rest_id, measured));
            ui.ctx().request_discard("The transport layout changed.");
        }
    }
}

fn transport_position(
    ui: &mut egui::Ui,
    size: Vec2,
    tick: u32,
    bpm: f64,
    bar_and_beat: bool,
) -> Response {
    let bars = tick / TICKS_PER_BAR + 1;
    let beat = (tick % TICKS_PER_BAR) / (TICKS_PER_BAR / 4) + 1;
    let seconds = (tick as f64 * 240.0 / bpm / TICKS_PER_BAR as f64) as u64;
    let text = format!("{:02} : {:02}", seconds / 60, seconds % 60);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    // Vintage reads time from an amber display window; the other themes from
    // the header itself.
    let display = roles().material == theme::Material::Metal;
    if display {
        ui.painter().rect_filled(rect.shrink(2.0), 4.0, roles().ink);
        ui.painter().rect_stroke(
            rect.shrink(2.0),
            4.0,
            Stroke::new(1.0_f32, roles().line),
            egui::StrokeKind::Inside,
        );
    } else {
        ui.painter()
            .rect_filled(rect, roles().corner(6.0), roles().canvas);
    }
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .text(
            rect.center(),
            Align2::CENTER_CENTER,
            &text,
            FontId::proportional(if size.x < POSITION_SIZE.x { 19.0 } else { 23.0 }),
            if display {
                roles().accent
            } else {
                roles().text
            },
        );
    // Bar and beat beside the time, where the top bar has room.
    if bar_and_beat {
        let (beside, _) = ui.allocate_exact_size(Vec2::new(44.0, size.y), Sense::hover());
        ui.painter().text(
            beside.left_center(),
            Align2::LEFT_CENTER,
            format!("{bars} . {beat}"),
            FontId::proportional(12.0),
            roles().muted,
        );
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
    response.hover_hint(format!(
        "{text} (minutes : seconds). Bar {bars}, beat {beat}"
    ))
}

/// How the song name is laid out at the left of the transport.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TitleFit {
    pub(super) size: f32,
    pub(super) rows: usize,
    pub(super) break_anywhere: bool,
}

fn transport_title_galley(
    ui: &egui::Ui,
    text: &str,
    fit: TitleFit,
    width: f32,
) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(
        title_text(text),
        transport_title_font(fit.size),
        roles().text,
        width.max(1.0),
    );
    job.wrap.max_rows = fit.rows;
    job.wrap.break_anywhere = fit.break_anywhere;
    for section in &mut job.sections {
        section.format.line_height = Some(fit.size * theme::HEADING_LINE);
    }
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

fn transport_title_font(size: f32) -> FontId {
    FontId::new(size, theme::heading_family())
}

/// The largest layout that shows the whole song name within `width`: from 15
/// points down to the 9-point floor, on one line or two lines wrapped between
/// words, then at the floor in as many lines as the row holds. Only a name too
/// long for that goes below the floor, breaking words if it must, so the name
/// is never clipped.
fn transport_title_fit(ui: &egui::Ui, text: &str, width: f32) -> TitleFit {
    let fits = |fit: TitleFit| {
        let galley = transport_title_galley(ui, text, fit, width);
        !galley.elided && galley.size().x <= width + 0.5 && galley.size().y <= TRANSPORT_ROW_HEIGHT
    };
    let rows = |size: f32| {
        let row = size * theme::HEADING_LINE;
        ((TRANSPORT_ROW_HEIGHT / row).floor() as usize).max(1)
    };
    let upper = title_text(text);
    // Shrink the name rather than break one of its words.
    let words = |size: f32| {
        upper.split_whitespace().all(|word| {
            let galley = ui.fonts_mut(|fonts| {
                fonts.layout_no_wrap(word.into(), transport_title_font(size), roles().text)
            });
            galley.size().x <= width + 0.5
        })
    };
    let mut size = TRANSPORT_TITLE_SIZE;
    while size >= TRANSPORT_TITLE_FLOOR {
        let fit = TitleFit {
            size,
            rows: 2,
            break_anywhere: false,
        };
        if words(size) && fits(fit) {
            return fit;
        }
        size -= 1.0;
    }
    // At the floor, a very long name takes as many lines as the row holds.
    size = TRANSPORT_TITLE_FLOOR;
    let fit = TitleFit {
        size,
        rows: rows(size),
        break_anywhere: false,
    };
    if words(size) && fits(fit) {
        return fit;
    }
    loop {
        size -= 0.5;
        let fit = TitleFit {
            size,
            rows: rows(size),
            break_anywhere: true,
        };
        if size <= 1.0 {
            return TitleFit {
                rows: usize::MAX,
                ..fit
            };
        }
        if fits(fit) {
            return fit;
        }
    }
}

/// The brand: MixJam over Techno, smaller in a narrow top bar.
fn stacked_brand(ui: &mut egui::Ui, compact: bool) {
    let galley = |text: egui::RichText| {
        egui::WidgetText::from(text).into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
    };
    let (large, small) = if compact { (14.0, 9.0) } else { (20.0, 11.0) };
    let top = galley(brand("MixJam", large));
    let bottom = galley(brand("Techno", small).color(roles().muted));
    let size = Vec2::new(
        top.size().x.max(bottom.size().x),
        top.size().y + bottom.size().y,
    );
    let (_, rect) = ui.allocate_space(size);
    // Two labels, so each word keeps its own accessible name.
    let mut stack = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    stack.spacing_mut().item_spacing.y = 0.0;
    stack.label(top);
    stack.label(bottom);
}

/// The song's length under its name: bars and minutes. None for an empty
/// song.
pub(in super::super) fn song_meta(project: &Project) -> Option<String> {
    let end = project.end_tick();
    if end == 0 {
        return None;
    }
    let bars = end.div_ceil(TICKS_PER_BAR);
    let seconds = (f64::from(end) * 240.0 / project.bpm / f64::from(TICKS_PER_BAR)).round() as u64;
    Some(format!(
        "{bars} {}  ·  {}:{:02}",
        if bars == 1 { "bar" } else { "bars" },
        seconds / 60,
        seconds % 60
    ))
}

#[derive(Clone, Copy)]
pub(in super::super) enum TransportIcon {
    Loop,
    Start,
    Back,
    Stop,
    Pause,
    Play,
    Forward,
    End,
    Record,
}

pub(in super::super) fn transport_button(
    ui: &mut egui::Ui,
    size: f32,
    icon: TransportIcon,
    active: bool,
    tooltip: &str,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let primary = matches!(icon, TransportIcon::Play | TransportIcon::Pause);
    let fill = if primary {
        roles().accent
    } else if response.hovered() {
        roles().hover
    } else {
        roles().raised
    };
    theme::surface(ui.painter(), rect, roles().corner(6.0), fill);
    let center = rect.center();
    let painter = ui.painter();
    let color = if primary {
        roles().on_light
    } else if active {
        roles().accent
    } else {
        roles().text
    };
    // Snapped corners keep the flat edge on a pixel boundary; a fractional
    // edge leaves a faint ghost column beside the glyph.
    let triangle = |x: f32, direction: f32| {
        painter.add(egui::Shape::convex_polygon(
            [
                Vec2::new(x - direction * 5.0, -9.0),
                Vec2::new(x - direction * 5.0, 9.0),
                Vec2::new(x + direction * 7.0, 0.0),
            ]
            .map(|offset| {
                egui::emath::GuiRounding::round_to_pixels(
                    center + offset,
                    painter.pixels_per_point(),
                )
            })
            .to_vec(),
            color,
            Stroke::NONE,
        ));
    };
    match icon {
        TransportIcon::Play => triangle(0.0, 1.0),
        TransportIcon::Pause => {
            for x in [-5.0, 5.0] {
                painter.rect_filled(
                    Rect::from_center_size(center + Vec2::new(x, 0.0), Vec2::new(5.0, 18.0)),
                    0.0,
                    color,
                );
            }
        }
        TransportIcon::Stop => {
            painter.rect_filled(
                Rect::from_center_size(center, Vec2::splat(17.0)),
                0.0,
                color,
            );
        }
        TransportIcon::Record => {
            painter.circle_filled(center, 10.0, roles().coral);
            if active {
                painter.circle_stroke(center, 13.0, Stroke::new(1.0_f32, roles().text));
            }
        }
        TransportIcon::Back => {
            triangle(-5.0, -1.0);
            triangle(7.0, -1.0);
        }
        TransportIcon::Forward => {
            triangle(-7.0, 1.0);
            triangle(5.0, 1.0);
        }
        TransportIcon::Start => {
            triangle(3.0, -1.0);
            painter.line_segment(
                [
                    center + Vec2::new(-10.0, -10.0),
                    center + Vec2::new(-10.0, 10.0),
                ],
                Stroke::new(3.0_f32, color),
            );
        }
        TransportIcon::End => {
            triangle(-3.0, 1.0);
            painter.line_segment(
                [
                    center + Vec2::new(10.0, -10.0),
                    center + Vec2::new(10.0, 10.0),
                ],
                Stroke::new(3.0_f32, color),
            );
        }
        TransportIcon::Loop => {
            // Two opposed arrows around a rounded rectangle; the second half
            // is the first turned half a circle.
            for side in [1.0_f32, -1.0] {
                let at = |x: f32, y: f32| center + Vec2::new(x, y) * side;
                let mut points = vec![at(-7.0, 0.5)];
                points.extend((0..=6).map(|i| {
                    let a = std::f32::consts::PI * (1.0 + i as f32 / 12.0);
                    at(-4.0 + 3.0 * a.cos(), -2.0 + 3.0 * a.sin())
                }));
                points.push(at(4.5, -5.0));
                painter.add(egui::Shape::line(points, Stroke::new(2.2_f32, color)));
                painter.add(egui::Shape::convex_polygon(
                    vec![at(10.0, -5.0), at(4.0, -9.5), at(4.0, -0.5)],
                    color,
                    Stroke::NONE,
                ));
            }
        }
    }
    // An outline marks an engaged loop, so the state does not rest on color.
    if active && matches!(icon, TransportIcon::Loop) {
        painter.rect_stroke(
            rect.shrink(0.5),
            6.0,
            Stroke::new(1.0_f32, roles().accent),
            egui::StrokeKind::Inside,
        );
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(2.0),
            6.0,
            Stroke::new(
                2.0_f32,
                if primary {
                    roles().on_light
                } else {
                    roles().accent
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| {
        if matches!(icon, TransportIcon::Loop | TransportIcon::Record) {
            egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, tooltip)
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tooltip)
        }
    });
    response.hover_hint(tooltip)
}

//! The toolbar above the score: its heading and counts, the Follow, Sections,
//! and Zoom controls, the Maximise button, and the lane menu that chooses the
//! automation or return lane to show.
use super::*;

/// The timeline zoom: the Zoom slider's range and the factor of one wheel step.
const ZOOM_RANGE: std::ops::RangeInclusive<f32> = 0.55..=2.0;

const ZOOM_WHEEL_STEP: f32 = 1.1;

/// What the lanes edit: clips, erased clips, an automation lane, or a return
/// lane, as the toolbar's lane menu and the Erase switch choose.
#[derive(Default)]
pub(in super::super) struct ScoreTools {
    pub(in super::super) erase: bool,
    /// The automation lane that the arrangement shows and edits, if any.
    /// While one shows, the lanes edit its points instead of clips.
    pub(in super::super) automation_lane: Option<automation::Lane>,
    /// The FX bus whose return lane the arrangement shows and edits, if any.
    /// It takes the place of a track lane.
    pub(in super::super) return_lane: Option<usize>,
}

impl MixJamApp {
    /// The score's toolbar: its heading and counts, then the automation lane,
    /// Follow, Sections, Zoom and Maximise against the right edge.
    pub(super) fn score_toolbar(&mut self, ui: &mut egui::Ui, track_count: usize) {
        // Keep the toolbar in one row beside the Sounds panel, so its
        // controls do not take height from the lane viewport.
        let narrow = ui.available_width() < 840.0;
        let counts = format!(
            "{} tracks  /  {} clips",
            track_count,
            self.session.project().clips.len()
        );
        // Below this width, the heading would push actions behind a side panel.
        let show_heading = ui.available_width() >= 600.0;
        ui.horizontal(|ui| {
            score_toolbar_heading(ui, show_heading, narrow, &counts);
            let compact = ui.available_width() < 700.0;
            // A compact toolbar packs its controls closer.
            if compact {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.spacing_mut().button_padding.x = 6.0;
            }
            // The controls' width is measured in the previous pass; a change
            // redoes this pass before it is shown.
            let width_id = ui.id().with(("score-toolbar-width", compact));
            let width = ui.ctx().data(|data| data.get_temp::<f32>(width_id));
            ui.add_space((ui.available_width() - width.unwrap_or_default()).max(16.0));
            let start = ui.cursor().left();
            self.automation_selector(ui, !narrow);
            ui.add_space(if compact { 4.0 } else { 10.0 });
            self.score_toolbar_toggles(ui);
            ui.add_space(if compact { 4.0 } else { 10.0 });
            self.score_toolbar_zoom_and_maximise(ui, compact);
            let measured = ui.min_rect().right() - start;
            if width.is_none_or(|width| (width - measured).abs() > 0.5) {
                ui.ctx()
                    .data_mut(|data| data.insert_temp(width_id, measured));
                ui.ctx().request_discard("The score toolbar changed.");
            }
        });
    }

    fn score_toolbar_toggles(&mut self, ui: &mut egui::Ui) {
        let follow = toggled(
            ui.toggle_value(&mut self.score.follow_playhead, "Follow"),
            "Follow",
            self.score.follow_playhead,
        );
        if follow
            .hover_hint("Keep the playhead in view during playback and seeking.")
            .changed()
        {
            self.score.last_follow_position = None;
        }
        let sections = toggled(
            ui.toggle_value(&mut self.sections.bookmarks, "Sections"),
            "Sections",
            self.sections.bookmarks,
        );
        if sections
            .hover_hint(
                "Show the song's section bookmarks on the bar ruler, the lanes and the song \
                 overview. With sections shown, double-click the bar ruler to add a section, \
                 double-click a name to rename it, right-click a name to delete it, and Loop \
                 loops the section at the playhead.",
            )
            .changed()
            && let Err(error) = self
                .session
                .remember_section_bookmarks(self.sections.bookmarks)
        {
            self.sections.bookmarks = self.session.options().show_section_bookmarks;
            self.error(error);
        }
    }

    fn score_toolbar_zoom_and_maximise(&mut self, ui: &mut egui::Ui, compact: bool) {
        ui.label(egui::RichText::new("Zoom").size(12.0).color(roles().muted));
        ui.spacing_mut().slider_width = if compact { 48.0 } else { 72.0 };
        let zoom = name_control(
            ui.add(
                egui::Slider::new(&mut self.score.zoom, ZOOM_RANGE)
                    .show_value(false)
                    .clamping(egui::SliderClamping::Always),
            ),
            "Zoom",
        )
        .hover_hint("Zoom: the width of a bar. The wheel over it zooms too.");
        // An egui slider ignores the wheel; each notch scales the zoom by
        // a tenth, as the other wheel-driven controls take one step.
        let turn = controls::wheel(ui, &zoom);
        if turn != 0.0 {
            self.score.zoom = (self.score.zoom * ZOOM_WHEEL_STEP.powf(turn))
                .clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end());
        }
        let maximised = self.session.layout().maximised;
        if maximise_button(ui, maximised).clicked() {
            self.set_layout(|layout| layout.maximised = !layout.maximised);
        }
    }
}

fn score_toolbar_heading(ui: &mut egui::Ui, show_heading: bool, narrow: bool, counts: &str) {
    if show_heading && narrow {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(heading("Arrangement", 12.0));
            ui.label(egui::RichText::new(counts).size(10.0).color(roles().muted));
        });
    } else if show_heading {
        ui.label(heading("Arrangement", 18.0));
        ui.add_space(10.0);
        ui.label(egui::RichText::new(counts).size(12.0).color(roles().muted));
    }
}

impl MixJamApp {
    /// The arrangement's automation lane: Off, or the lane that the track
    /// rows show and edit.
    fn automation_selector(&mut self, ui: &mut egui::Ui, show_label: bool) {
        if show_label {
            ui.label(
                egui::RichText::new("Automation")
                    .size(12.0)
                    .color(roles().muted),
            );
        }
        let shown = (
            self.score_tools.automation_lane,
            self.score_tools.return_lane,
        );
        let current = automation_label(shown, show_label);
        let mut choice = shown;
        if self.capture_view_is("automation-menu") {
            // egui keeps a combo box's popup state under its id with "popup",
            // and makes that id from the salt's `Id`.
            let button = ui.make_persistent_id(egui::Id::new("automation_lane"));
            egui::Popup::open_id(ui.ctx(), button.with("popup"));
        }
        let project = self.session.project();
        let ctx = ui.ctx().clone();
        let combo = egui::ComboBox::from_id_salt("automation_lane")
            .selected_text(current)
            // "Repeat gate" and "FX 1 return" still fit the narrow width.
            .width(if show_label { 112.0 } else { 96.0 })
            // The whole grouped list shows without a scroll bar.
            .height(f32::INFINITY);
        let response = tall_menu(&ctx, || {
            combo.show_ui(ui, |ui| automation_menu(ui, project, &mut choice))
        })
        .response;
        name_control(response, "Automation lane").hover_hint(
            "Show and edit one automation lane on every track, or the return lane of one FX \
             bus across the arrangement. Click a lane to add a point, drag a point to move it, \
             and double-click or right-click a point to delete it. Points snap to 1/16 bar; \
             hold Ctrl for single ticks. Off edits clips again.",
        );
        if choice != shown {
            self.choose_lane(choice);
        }
    }

    /// Draw `lane` on the score, or a bus's return lane, or edit clips when
    /// both are `None`.
    pub(in super::super) fn choose_lane(
        &mut self,
        lane: (Option<automation::Lane>, Option<usize>),
    ) {
        if lane
            != (
                self.score_tools.automation_lane,
                self.score_tools.return_lane,
            )
        {
            (
                self.score_tools.automation_lane,
                self.score_tools.return_lane,
            ) = lane;
            self.gestures.automation_drag = None;
            self.session.end_automation_gesture();
        }
    }

    /// Whether the arrangement shows an automation lane, whose points the
    /// lanes then edit instead of clips.
    pub(in super::super) fn automating(&self) -> bool {
        self.score_tools.automation_lane.is_some() || self.score_tools.return_lane.is_some()
    }
}

/// The score toolbar's Maximise switch: corners pointing out, or in while
/// the score fills the window.
fn maximise_button(ui: &mut egui::Ui, on: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(30.0, 26.0), Sense::click());
    controls::icon_button_surface(ui, &response, on);
    let color = rail_icon_color(ui, on);
    let stroke = Stroke::new(1.5_f32, color);
    let mark = Rect::from_center_size(rect.center(), Vec2::splat(12.0));
    let arm = 4.0;
    for (corner, x, y) in [
        (mark.left_top(), 1.0, 1.0),
        (mark.right_top(), -1.0, 1.0),
        (mark.left_bottom(), 1.0, -1.0),
        (mark.right_bottom(), -1.0, -1.0),
    ] {
        // Restored, each corner is an outward bracket; maximised, the
        // brackets turn inward.
        let (tip, dx, dy) = if on {
            (corner + Vec2::new(x * arm, y * arm), -x, -y)
        } else {
            (corner, x, y)
        };
        ui.painter()
            .line_segment([tip, tip + Vec2::new(dx * arm, 0.0)], stroke);
        ui.painter()
            .line_segment([tip, tip + Vec2::new(0.0, dy * arm)], stroke);
    }
    focus_outline(ui, &response, roles().accent);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            on,
            "Maximise the score",
        )
    });
    response.hover_hint(if on {
        "Restore the tool rail, the Sounds panel, the Inspector and the lower panel (F)"
    } else {
        "Maximise the score: hide the tool rail, the Sounds panel, the Inspector and the lower \
         panel (F)"
    })
}

/// The Automation menu's first group: the lanes on every track other than
/// the sends, which have their own group.
pub(in super::super) const TRACK_LANES: [automation::Lane; 7] = [
    automation::Lane::Volume,
    automation::Lane::Pan,
    automation::Lane::Filter,
    automation::Lane::Compression,
    automation::Lane::Pitch,
    automation::Lane::Repeat,
    automation::Lane::RepeatGate,
];

/// A toolbar switch's accessible state: on or off, as the status bar's
/// panel switches report it.
fn toggled(response: Response, label: &str, on: bool) -> Response {
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label)
    });
    response
}

/// "1 track", "2 tracks".
fn plural(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}
/// The lane menu's shown choice: a track lane, a bus's return lane, or Off.
fn automation_label(lane: MenuLane, show_label: bool) -> String {
    match lane {
        (_, Some(bus)) => return_lane_label(bus),
        (Some(lane), None) => lane.label(),
        // Without its label beside it, a bare "Off" would not say what is off.
        (None, None) if !show_label => "No lane".into(),
        (None, None) => "Off".into(),
    }
}

/// A lane of the lane menu: a track lane, a bus's return lane, or neither.
type MenuLane = (Option<automation::Lane>, Option<usize>);

/// The grouped rows of the lane menu: Off, the track lanes, the send lanes,
/// and the return lanes. A click on a row sets `choice`.
fn automation_menu(ui: &mut egui::Ui, project: &Project, choice: &mut MenuLane) {
    ui.spacing_mut().item_spacing.y = 0.0;
    lane_row(ui, choice, (None, None), "Off", (Some("edit clips"), None));
    let tracks = |lane: automation::Lane| {
        let count = project
            .tracks
            .iter()
            .filter(|track| !track.automation.lane(lane).is_empty())
            .count();
        (count > 0).then(|| plural(count, "track"))
    };
    lane_menu_heading(ui, "On every track");
    for lane in TRACK_LANES {
        lane_row(
            ui,
            choice,
            (Some(lane), None),
            &lane.label(),
            (None, tracks(lane)),
        );
    }
    lane_menu_heading(ui, "Send amount, on every track");
    for (bus, fx_bus) in project.fx.buses.iter().enumerate() {
        let lane = automation::Lane::Send(bus);
        let module = bus_module_name(&fx_bus.module);
        lane_row(
            ui,
            choice,
            (Some(lane), None),
            &lane.label(),
            (Some(module), tracks(lane)),
        );
    }
    return_rows(ui, project, choice);
}

/// The return lane of each FX bus, with the count of its points.
fn return_rows(ui: &mut egui::Ui, project: &Project, choice: &mut MenuLane) {
    lane_menu_heading(ui, "Return level, one line across the song");
    for bus in 0..mixjam_techno::application::fx::BUSES {
        let module = bus_module_name(&project.fx.buses[bus].module);
        let points = project.return_automation[bus].len();
        let count = (points > 0).then(|| plural(points, "point"));
        lane_row(
            ui,
            choice,
            (None, Some(bus)),
            &return_lane_label(bus),
            (Some(module), count),
        );
    }
}

/// One row of the lane menu, with its detail and count. A click chooses
/// `lane` and closes the menu.
fn lane_row(
    ui: &mut egui::Ui,
    choice: &mut MenuLane,
    lane: MenuLane,
    name: &str,
    (detail, count): (Option<&str>, Option<String>),
) {
    if lane_menu_row(ui, *choice == lane, name, detail, count).clicked() {
        *choice = lane;
        ui.close();
    }
}

/// What an FX bus holds, as the Automation menu names it beside its lanes.
fn bus_module_name(module: &mixjam_techno::application::fx::Module) -> &'static str {
    if module.is_empty() {
        "empty"
    } else {
        module.label()
    }
}

/// A group heading inside the Automation menu.
pub(in super::super) fn lane_menu_heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.add(
        egui::Label::new(
            egui::RichText::new(title_text(text))
                .size(10.0)
                .color(roles().muted),
        )
        .selectable(false),
    );
    ui.add_space(2.0);
}

/// One lane in the Automation menu: its name, what its bus holds, and how
/// much it holds, as "2 tracks" or "4 points". The current lane is filled
/// and marked at its left edge. Its accessible name is the lane's name alone.
fn lane_menu_row(
    ui: &mut egui::Ui,
    selected: bool,
    name: &str,
    detail: Option<&str>,
    count: Option<String>,
) -> egui::Response {
    let width = ui.available_width().max(300.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 2.0, roles().selection);
        painter.rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
            0.0,
            roles().accent,
        );
    } else if response.hovered() || response.has_focus() {
        painter.rect_filled(rect, 2.0, roles().hover);
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(0.5),
            2.0,
            Stroke::new(1.0_f32, roles().accent),
            egui::StrokeKind::Inside,
        );
    }
    let mut right = rect.right() - 8.0;
    if let Some(count) = count {
        let counted = painter.text(
            Pos2::new(right, rect.center().y),
            Align2::RIGHT_CENTER,
            count,
            FontId::proportional(11.0),
            roles().muted,
        );
        right = counted.left() - 10.0;
    }
    let left = rect.left() + 10.0;
    let mut job = egui::text::LayoutJob::default();
    job.append(
        name,
        0.0,
        egui::TextFormat::simple(FontId::proportional(13.0), roles().text),
    );
    if let Some(detail) = detail {
        job.append(
            &format!(" · {detail}"),
            0.0,
            egui::TextFormat::simple(FontId::proportional(12.0), roles().muted),
        );
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width((right - left).max(1.0));
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    ui.painter().galley(
        Pos2::new(left, rect.center().y - galley.size().y / 2.0),
        galley,
        roles().text,
    );
    response
}

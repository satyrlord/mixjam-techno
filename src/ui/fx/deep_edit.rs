//! Layer 2 floating FX Deep Edit window (`824x572 pt`) with full parameter bays,
//! live drive transfer scope, and telemetry meters.
use super::super::*;
use super::faces::{Slot, dial_slot};
use super::unit::{bus_switch, choice, left_division, module_and_preset_combos, note_division};
use super::{DEEP_EDIT_TITLE, fx_motif};
use mixjam_techno::application::FxBusMeterReading;
use mixjam_techno::application::fx::{
    Aetherform, BUSES, DelayCharacter, DelayParam, DistortionParam, DistortionShape, FxBus, Module,
    ReverbCharacter, ReverbParam, SHIMMER_INTERVALS, SpaceModel, TimeMode,
};

const WINDOW_W: f32 = 824.0;
const WINDOW_H: f32 = 572.0;
const WORKSPACE_W: f32 = 780.0;
const WORKSPACE_H: f32 = 456.0;
const LEFT_BAY_W: f32 = 500.0;
const BAY_GAP: f32 = 10.0;
const PREFIX: &str = "Deep edit";

#[derive(Clone, Copy)]
struct DialGrid {
    cols: usize,
    cell_w: f32,
    start_y: f32,
    well_offset: f32,
    label_offset: f32,
    row_h: f32,
}

impl DialGrid {
    const fn row(cols: usize, cell_w: f32, start_y: f32) -> Self {
        Self {
            cols,
            cell_w,
            start_y,
            well_offset: 38.0,
            label_offset: 58.0,
            row_h: 92.0,
        }
    }

    const fn two_row(cols: usize, cell_w: f32, start_y: f32, row_h: f32) -> Self {
        Self {
            cols,
            cell_w,
            start_y,
            well_offset: 36.0,
            label_offset: 55.0,
            row_h,
        }
    }
}

fn draw_dial_grid(ui: &mut egui::Ui, rect: Rect, grid: DialGrid, bus: &mut FxBus, slots: &[Slot]) {
    for (i, slot) in slots.iter().enumerate() {
        let (offset, row) = grid_cell(i, slots.len(), grid.cols);
        let x = rect.center().x + offset * grid.cell_w;
        let y = rect.top() + grid.start_y + row as f32 * grid.row_h;
        let positions = [
            Pos2::new(x, y),
            Pos2::new(x, y + grid.well_offset),
            Pos2::new(x, y + grid.label_offset),
        ];
        dial_slot(ui, positions, PREFIX, bus, slot, None);
    }
}

/// The column offset of dial `index` from the bay's center, in cells, and its
/// row. Each row centers on its own dial count, so a short last row stays
/// centered under the full rows.
fn grid_cell(index: usize, count: usize, cols: usize) -> (f32, usize) {
    let cols = cols.max(1);
    let row = index / cols;
    let in_row = (count - row * cols).min(cols);
    let col = index % cols;
    (col as f32 + 0.5 - in_row as f32 / 2.0, row)
}

fn bays(area: Rect, top_h: f32) -> [Rect; 4] {
    let right_w = (area.width() - LEFT_BAY_W - BAY_GAP).max(220.0);
    let bot_h = area.height() - top_h - BAY_GAP;
    let bay = |x: f32, y: f32, w: f32, h: f32| {
        Rect::from_min_size(area.min + Vec2::new(x, y), Vec2::new(w, h))
    };
    [
        bay(0.0, 0.0, LEFT_BAY_W, top_h),
        bay(LEFT_BAY_W + BAY_GAP, 0.0, right_w, top_h),
        bay(0.0, top_h + BAY_GAP, LEFT_BAY_W, bot_h),
        bay(LEFT_BAY_W + BAY_GAP, top_h + BAY_GAP, right_w, bot_h),
    ]
}

fn bus_tabs(
    ui: &mut egui::Ui,
    bus_index: &mut usize,
    meters: &[FxBusMeterReading; BUSES],
    ducks: [bool; BUSES],
) {
    ui.horizontal(|ui| {
        for i in 0..BUSES {
            let label = format!("FX {}", i + 1);
            let name = format!("{PREFIX} bus {}", i + 1);
            let role = if i == *bus_index {
                controls::ButtonRole::Primary
            } else {
                controls::ButtonRole::Secondary
            };
            if name_control(controls::button(ui, &label, role), &name).clicked() {
                *bus_index = i;
            }
        }
        ui.separator();
        let m = meters[*bus_index];
        let meter_rect = ui
            .allocate_exact_size(Vec2::new(148.0, 24.0), Sense::hover())
            .0;
        controls::instrument::meter_bar(
            ui,
            meter_rect,
            "Deep edit return meters",
            m,
            ducks[*bus_index],
        );
        ui.label(if m.tail_active {
            "Tail: Active"
        } else {
            "Tail: Idle"
        });
    });
}

fn header_bar(ui: &mut egui::Ui, bus: &mut FxBus, mut solo: bool) -> (bool, bool) {
    let mut clear_tail = false;
    let top_rect = ui
        .allocate_exact_size(Vec2::new(WORKSPACE_W, 28.0), Sense::hover())
        .0;
    module_and_preset_combos(
        ui,
        Rect::from_min_size(top_rect.min + Vec2::new(0.0, 2.0), Vec2::new(174.0, 24.0)),
        Rect::from_min_size(top_rect.min + Vec2::new(182.0, 2.0), Vec2::new(144.0, 24.0)),
        PREFIX,
        bus,
        false,
    );
    bus_switch(
        ui,
        top_rect.min + Vec2::new(340.0, 4.0),
        &format!("{PREFIX} power"),
        "Power",
        &mut bus.powered,
        "Enable send feed into this bus.",
        false,
    );
    let solo_toggled = ui
        .add_enabled_ui(bus.is_active(), |ui| {
            bus_switch(
                ui,
                top_rect.min + Vec2::new(416.0, 4.0),
                &format!("{PREFIX} solo"),
                "Solo",
                &mut solo,
                "Monitor only soloed FX returns during playback.",
                false,
            )
        })
        .inner
        .changed();
    bus_switch(
        ui,
        top_rect.min + Vec2::new(484.0, 4.0),
        &format!("{PREFIX} limiter"),
        "Limiter",
        &mut bus.limiter,
        super::super::mixer::LIMITER_HINT,
        false,
    );
    if let Some(bypass) = match &mut bus.module {
        Module::EchoformDelay(d) => Some(&mut d.bypass),
        Module::AetherformReverb(r) => Some(&mut r.bypass),
        Module::EmberformDrive(d) => Some(&mut d.bypass),
        Module::Empty => None,
    } {
        bus_switch(
            ui,
            top_rect.min + Vec2::new(564.0, 4.0),
            &format!("{PREFIX} bypass"),
            "Bypass",
            bypass,
            "Silence processed wet return while state keeps running.",
            false,
        );
        let clear_rect = Rect::from_min_size(
            Pos2::new(top_rect.right() - 84.0, top_rect.top() + 2.0),
            Vec2::new(84.0, 24.0),
        );
        let clear_name = format!("{PREFIX} clear tail");
        let resp = controls::button_at(
            ui,
            clear_rect,
            &clear_name,
            "Clear tail",
            controls::ButtonRole::Secondary,
            true,
        );
        if name_control(resp, &clear_name).clicked() {
            clear_tail = true;
        }
    }
    (clear_tail, solo_toggled)
}

fn drive_workspace(ui: &mut egui::Ui, area: Rect, bus: &mut FxBus, accent: Color32) {
    let [drive_bay, scope_bay, tone_bay, duck_bay] = bays(area, 228.0);

    controls::instrument::bay(ui, drive_bay, "Drive & Curve", accent, false);
    if let Module::EmberformDrive(drive) = &mut bus.module {
        let mut points = [0.0_f32; 65];
        Session::fx_drive_curve(drive, &mut points);
        controls::instrument::scope_frame(ui, scope_bay, "Transfer Curve", &points, accent);
        choice(
            ui,
            Rect::from_min_size(
                drive_bay.min + Vec2::new(14.0, 28.0),
                Vec2::new(136.0, 24.0),
            ),
            &format!("{PREFIX} drive type"),
            &mut drive.shape,
            &DistortionShape::ALL.map(|s| (s, s.label())),
            "Select wave-shaper or bitcrusher model.",
        );
        bus_switch(
            ui,
            drive_bay.min + Vec2::new(164.0, 29.0),
            &format!("{PREFIX} level match"),
            "Level match",
            &mut drive.level_match,
            "Equalize output loudness to match driven input.",
            false,
        );
    }
    draw_dial_grid(
        ui,
        drive_bay,
        DialGrid::row(4, 112.0, 108.0),
        bus,
        &[
            Slot::Drive(DistortionParam::Drive),
            Slot::Drive(DistortionParam::Asymmetry),
            Slot::Drive(DistortionParam::Output),
            Slot::Return,
        ],
    );

    controls::instrument::bay(ui, tone_bay, "Tone & Bitcrush", accent, false);
    draw_dial_grid(
        ui,
        tone_bay,
        DialGrid::row(5, 94.0, 92.0),
        bus,
        &[
            Slot::Drive(DistortionParam::LowCut),
            Slot::Drive(DistortionParam::HighCut),
            Slot::Drive(DistortionParam::Bits),
            Slot::Drive(DistortionParam::CrushRate),
            Slot::Drive(DistortionParam::Width),
        ],
    );

    controls::instrument::bay(ui, duck_bay, "Post-Limiter Ducker", accent, false);
    draw_dial_grid(
        ui,
        duck_bay,
        DialGrid::two_row(2, 116.0, 50.0, 90.0),
        bus,
        &[
            Slot::Drive(DistortionParam::DuckKey),
            Slot::Drive(DistortionParam::DuckThreshold),
            Slot::Drive(DistortionParam::DuckAmount),
            Slot::Drive(DistortionParam::DuckRelease),
        ],
    );
}

fn delay_workspace(ui: &mut egui::Ui, area: Rect, bus: &mut FxBus, bpm: f64, accent: Color32) {
    let [time_bay, scope_bay, tone_bay, duck_bay] = bays(area, 216.0);

    controls::instrument::bay(ui, time_bay, "Time & Feedback", accent, false);
    if let Module::EchoformDelay(delay) = &mut bus.module {
        let mut sync = delay.mode == TimeMode::Sync;
        bus_switch(
            ui,
            time_bay.min + Vec2::new(12.0, 28.0),
            &format!("{PREFIX} sync"),
            "Sync",
            &mut sync,
            "Lock delay times to project tempo divisions.",
            false,
        );
        delay.mode = if sync { TimeMode::Sync } else { TimeMode::Free };
        let link_before = delay.link_times;
        bus_switch(
            ui,
            time_bay.min + Vec2::new(80.0, 28.0),
            &format!("{PREFIX} link times"),
            "Link L/R",
            &mut delay.link_times,
            "Keep left and right delay times matched.",
            false,
        );
        if delay.link_times && !link_before {
            delay.time_ms_r = delay.time_ms_l;
            delay.division_r = delay.division_l;
        }
        bus_switch(
            ui,
            time_bay.min + Vec2::new(164.0, 28.0),
            &format!("{PREFIX} ping-pong"),
            "Ping-pong",
            &mut delay.ping_pong,
            "Alternate echoes across stereo channels.",
            false,
        );
        choice(
            ui,
            Rect::from_min_size(time_bay.min + Vec2::new(260.0, 28.0), Vec2::new(84.0, 22.0)),
            &format!("{PREFIX} delay character"),
            &mut delay.character,
            &[
                (DelayCharacter::Digital, "Digital"),
                (DelayCharacter::Analog, "Analog"),
                (DelayCharacter::Tape, "Tape"),
            ],
            "Feedback saturation and character.",
        );
        ui.add_enabled_ui(sync, |ui| {
            left_division(
                ui,
                Rect::from_min_size(time_bay.min + Vec2::new(354.0, 28.0), Vec2::new(64.0, 22.0)),
                PREFIX,
                delay,
            );
            ui.add_enabled_ui(!delay.link_times, |ui| {
                note_division(
                    ui,
                    Rect::from_min_size(
                        time_bay.min + Vec2::new(424.0, 28.0),
                        Vec2::new(64.0, 22.0),
                    ),
                    &format!("{PREFIX} right division"),
                    &mut delay.division_r,
                );
            });
        });
    }
    draw_dial_grid(
        ui,
        time_bay,
        DialGrid::row(4, 112.0, 108.0),
        bus,
        &[
            Slot::Delay(DelayParam::TimeL),
            Slot::Delay(DelayParam::TimeR),
            Slot::Delay(DelayParam::Feedback),
            Slot::Return,
        ],
    );

    controls::instrument::bay(ui, duck_bay, "Post-Limiter Ducker", accent, false);
    draw_dial_grid(
        ui,
        duck_bay,
        DialGrid::two_row(2, 116.0, 50.0, 92.0),
        bus,
        &[
            Slot::Delay(DelayParam::DuckKey),
            Slot::Delay(DelayParam::DuckThreshold),
            Slot::Delay(DelayParam::DuckAmount),
            Slot::Delay(DelayParam::DuckRelease),
        ],
    );

    controls::instrument::bay(ui, tone_bay, "Tone, Smear & Mod", accent, false);
    draw_dial_grid(
        ui,
        tone_bay,
        DialGrid::two_row(5, 94.0, 50.0, 92.0),
        bus,
        &[
            Slot::Delay(DelayParam::LowCut),
            Slot::Delay(DelayParam::HighCut),
            Slot::Delay(DelayParam::FilterResonance),
            Slot::Delay(DelayParam::Smear),
            Slot::Delay(DelayParam::Drive),
            Slot::Delay(DelayParam::ModRate),
            Slot::Delay(DelayParam::ModDepth),
            Slot::Delay(DelayParam::Width),
            Slot::Delay(DelayParam::Output),
        ],
    );

    if let Module::EchoformDelay(delay) = &bus.module {
        let mut taps = [(0.0_f32, 0.0_f32, 0.0_f32); 32];
        let count = Session::fx_delay_taps(delay, bpm, &mut taps);
        controls::instrument::delay_scope_frame(
            ui,
            scope_bay,
            "Tap & Decay",
            &taps[..count],
            delay.ping_pong,
            accent,
        );
    }
}

fn reverb_workspace(ui: &mut egui::Ui, area: Rect, bus: &mut FxBus, accent: Color32) {
    let [space_bay, scope_bay, tone_bay, duck_bay] = bays(area, 236.0);

    controls::instrument::bay(ui, space_bay, "Space, Damping & Shimmer", accent, false);
    reverb_switches(ui, space_bay, bus);
    draw_dial_grid(
        ui,
        space_bay,
        DialGrid::two_row(5, 94.0, 76.0, 86.0),
        bus,
        &[
            Slot::Reverb(ReverbParam::PreDelay),
            Slot::Reverb(ReverbParam::Decay),
            Slot::Reverb(ReverbParam::BassMult),
            Slot::Reverb(ReverbParam::TrebleMult),
            Slot::Reverb(ReverbParam::Size),
            Slot::Reverb(ReverbParam::LateBalance),
            Slot::Reverb(ReverbParam::Diffusion),
            Slot::Reverb(ReverbParam::Density),
            Slot::Reverb(ReverbParam::ShimmerAmount),
        ],
    );

    controls::instrument::bay(ui, tone_bay, "Tone, Modulation & Output", accent, false);
    draw_dial_grid(
        ui,
        tone_bay,
        DialGrid::two_row(4, 114.0, 48.0, 88.0),
        bus,
        &[
            Slot::Reverb(ReverbParam::LowCut),
            Slot::Reverb(ReverbParam::HighCut),
            Slot::Reverb(ReverbParam::ModRate),
            Slot::Reverb(ReverbParam::ModDepth),
            Slot::Reverb(ReverbParam::Drive),
            Slot::Reverb(ReverbParam::Width),
            Slot::Reverb(ReverbParam::Output),
            Slot::Return,
        ],
    );

    controls::instrument::bay(ui, duck_bay, "Post-Limiter Ducker", accent, false);
    draw_dial_grid(
        ui,
        duck_bay,
        DialGrid::two_row(2, 116.0, 48.0, 88.0),
        bus,
        &[
            Slot::Reverb(ReverbParam::DuckKey),
            Slot::Reverb(ReverbParam::DuckThreshold),
            Slot::Reverb(ReverbParam::DuckAmount),
            Slot::Reverb(ReverbParam::DuckRelease),
        ],
    );

    if let Module::AetherformReverb(reverb) = &bus.module {
        let tag = reverb_tag(reverb);
        let profile = Session::fx_reverb_profile(reverb);
        controls::instrument::reverb_scope_frame(
            ui,
            scope_bay,
            "Decay & Space",
            &tag,
            &profile,
            accent,
        );
    }
}

/// The scope's tag: the space, the character, and the shimmer interval.
fn reverb_tag(reverb: &Aetherform) -> String {
    let character = match reverb.character {
        ReverbCharacter::Natural => "NATURAL",
        ReverbCharacter::Vintage => "VINTAGE",
        ReverbCharacter::Bloom => "BLOOM",
    };
    let mut tag = format!(
        "{} · {character}",
        reverb.space_model.label().to_uppercase()
    );
    if reverb.shimmer_enabled {
        tag.push_str(&format!(" +{}", reverb.shimmer_interval_semitones));
    }
    tag
}

/// The selectors and switches along the top of the space bay.
fn reverb_switches(ui: &mut egui::Ui, space_bay: Rect, bus: &mut FxBus) {
    let Module::AetherformReverb(reverb) = &mut bus.module else {
        return;
    };
    let at = |x: f32, w: f32| {
        Rect::from_min_size(space_bay.min + Vec2::new(x, 26.0), Vec2::new(w, 22.0))
    };
    choice(
        ui,
        at(12.0, 96.0),
        &format!("{PREFIX} space"),
        &mut reverb.space_model,
        &SpaceModel::ALL.map(|m| (m, m.label())),
        "Acoustic space model.",
    );
    choice(
        ui,
        at(116.0, 88.0),
        &format!("{PREFIX} reverb character"),
        &mut reverb.character,
        &[
            (ReverbCharacter::Natural, "Natural"),
            (ReverbCharacter::Vintage, "Vintage"),
            (ReverbCharacter::Bloom, "Bloom"),
        ],
        "Tail character.",
    );
    bus_switch(
        ui,
        space_bay.min + Vec2::new(214.0, 26.0),
        &format!("{PREFIX} early reflections"),
        "Early",
        &mut reverb.early_reflections_enabled,
        "Initial room reflections.",
        false,
    );
    bus_switch(
        ui,
        space_bay.min + Vec2::new(294.0, 26.0),
        &format!("{PREFIX} shimmer"),
        "Shimmer",
        &mut reverb.shimmer_enabled,
        "Pitch-shifted tail feedback.",
        false,
    );
    let intervals: Vec<(u8, String)> = SHIMMER_INTERVALS
        .iter()
        .map(|s| (*s, format!("+{s}")))
        .collect();
    let labels: Vec<(u8, &str)> = intervals.iter().map(|(v, l)| (*v, l.as_str())).collect();
    ui.add_enabled_ui(reverb.shimmer_enabled, |ui| {
        choice(
            ui,
            at(394.0, 74.0),
            &format!("{PREFIX} shimmer interval"),
            &mut reverb.shimmer_interval_semitones,
            &labels,
            "Shimmer pitch interval in semitones.",
        );
    });
}
fn draw_module_workspace(ui: &mut egui::Ui, area: Rect, bus: &mut FxBus, bpm: f64) {
    let ui = &mut ui.new_child(egui::UiBuilder::new().id_salt("fx-bays").max_rect(area));
    let (motif, accent) = fx_motif(&bus.module);
    theme::with_knob_style(motif.knob, || match bus.module {
        Module::Empty => {
            controls::instrument::bay(ui, area, "Empty Bus", accent, false);
            ui.painter().text(
                area.center(),
                Align2::CENTER_CENTER,
                "Choose Echoform Delay, Aetherform Reverb,\n\
                 or Emberform Drive above.",
                FontId::proportional(13.0),
                roles().legend,
            );
        }
        Module::EmberformDrive(_) => drive_workspace(ui, area, bus, accent),
        Module::EchoformDelay(_) => delay_workspace(ui, area, bus, bpm, accent),
        Module::AetherformReverb(_) => reverb_workspace(ui, area, bus, accent),
    });
}

impl MixJamApp {
    /// Floating Layer 2 FX Deep Edit workspace showing all parameters, live curves, and meters.
    pub(in super::super) fn fx_deep_edit_window(&mut self, ctx: &egui::Context) {
        let Some(mut bus_index) = self.fx_deep_edit else {
            return;
        };
        let mut open = true;
        let buses = self.session.project().fx;
        let bpm = self.session.project().bpm;
        let mut draft = buses;
        let solos: [bool; BUSES] = std::array::from_fn(|index| self.session.fx_solo(index));
        let meters = self.boards.fx_meter_display.read();
        let generation = self.boards.master_view_generation;
        let mut clear_tail = false;
        let mut solo_toggled = false;

        let bounds = ctx.viewport_rect().shrink(12.0);
        dialog::window(DEEP_EDIT_TITLE)
            .open(&mut open)
            .palette(controls::Palette::Marine)
            .default_width(WINDOW_W)
            .default_height(WINDOW_H)
            .max_height((ctx.viewport_rect().height() - 40.0).max(320.0))
            .constrain_to(bounds)
            .show(ctx, |ui| {
                let ducks = draft.buses.map(|bus| bus.module.ducks());
                bus_tabs(ui, &mut bus_index, &meters, ducks);
                ui.push_id((generation, bus_index), |ui| {
                    let bus = &mut draft.buses[bus_index];
                    let (cleared, toggled) = header_bar(ui, bus, solos[bus_index]);
                    clear_tail = cleared;
                    solo_toggled = toggled;
                    let area = ui
                        .allocate_exact_size(Vec2::new(WORKSPACE_W, WORKSPACE_H), Sense::hover())
                        .0;
                    draw_module_workspace(ui, area, bus, bpm);
                });
            });

        self.fx_deep_edit = open.then_some(bus_index);
        if clear_tail {
            self.session.clear_fx_tail(bus_index);
        }
        if solo_toggled {
            self.session.toggle_fx_solo(bus_index);
        }
        if draft != buses {
            let result = self.session.edit(EditCommand::SetEffects(draft));
            self.apply(result);
        }
    }
}

#[cfg(test)]
mod grid_tests {
    use super::grid_cell;

    #[test]
    fn a_short_last_row_centers_under_the_full_row() {
        let offsets: Vec<_> = (0..9).map(|i| grid_cell(i, 9, 5)).collect();
        assert_eq!(
            offsets,
            [
                (-2.0, 0),
                (-1.0, 0),
                (0.0, 0),
                (1.0, 0),
                (2.0, 0),
                (-1.5, 1),
                (-0.5, 1),
                (0.5, 1),
                (1.5, 1),
            ]
        );
    }

    #[test]
    fn full_rows_and_single_rows_center_on_the_bay() {
        let offsets: Vec<_> = (0..4).map(|i| grid_cell(i, 4, 2)).collect();
        assert_eq!(offsets, [(-0.5, 0), (0.5, 0), (-0.5, 1), (0.5, 1)]);
        let offsets: Vec<_> = (0..3).map(|i| grid_cell(i, 3, 4)).collect();
        assert_eq!(offsets, [(-1.0, 0), (0.0, 0), (1.0, 0)]);
    }
}

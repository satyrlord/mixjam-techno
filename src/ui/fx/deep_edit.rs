//! Layer 2 floating FX Deep Edit window (`760x480 pt`) with full parameter bays,
//! live drive transfer scope, and telemetry meters.
use super::super::*;
use super::faces::{Slot, dial_slot};
use super::unit::{bus_switch, choice, left_division, module_and_preset_combos, note_division};
use super::{DEEP_EDIT_TITLE, enamel};
use mixjam_techno::application::FxBusMeterReading;
use mixjam_techno::application::fx::{
    BUSES, DelayCharacter, DelayParam, DistortionParam, DistortionShape, FxBus, Module,
    ReverbCharacter, ReverbParam, SHIMMER_INTERVALS, SpaceModel, TimeMode,
};

const WINDOW_W: f32 = 760.0;
const WINDOW_H: f32 = 480.0;
const PREFIX: &str = "Deep edit";
const CELL_W: f32 = 58.0;
const ROW_H: f32 = 88.0;

fn bay_accent(module: &Module) -> Color32 {
    let [r, g, b] = enamel(module);
    Color32::from_rgb(r, g, b)
}

fn draw_dial_grid_with(
    ui: &mut egui::Ui,
    rect: Rect,
    cols: usize,
    cell_w: f32,
    compact: bool,
    bus: &mut FxBus,
    slots: &[Slot],
) {
    let cols = cols.max(1);
    for (i, slot) in slots.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let x = rect.left() + 14.0 + (col as f32 + 0.5) * cell_w;
        let y = rect.top() + 44.0 + row as f32 * ROW_H;
        let positions = [
            Pos2::new(x, y),
            Pos2::new(x, y + 34.0),
            Pos2::new(x, y + 54.0),
        ];
        dial_slot(
            ui,
            positions,
            PREFIX,
            bus,
            slot,
            compact.then_some(super::master::COMPACT_WELL),
        );
    }
}

fn draw_dial_grid(ui: &mut egui::Ui, rect: Rect, cols: usize, bus: &mut FxBus, slots: &[Slot]) {
    draw_dial_grid_with(ui, rect, cols, CELL_W, false, bus, slots);
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
            .allocate_exact_size(Vec2::new(138.0, 24.0), Sense::hover())
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
    ui.add_space(4.0);
    let top_rect = ui
        .allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover())
        .0;
    module_and_preset_combos(
        ui,
        Rect::from_min_size(top_rect.min, Vec2::new(174.0, 24.0)),
        Rect::from_min_size(top_rect.min + Vec2::new(182.0, 0.0), Vec2::new(126.0, 24.0)),
        PREFIX,
        bus,
        false,
    );
    bus_switch(
        ui,
        top_rect.min + Vec2::new(320.0, 3.0),
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
                top_rect.min + Vec2::new(395.0, 3.0),
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
        top_rect.min + Vec2::new(462.0, 3.0),
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
            top_rect.min + Vec2::new(542.0, 3.0),
            &format!("{PREFIX} bypass"),
            "Bypass",
            bypass,
            "Silence processed wet return while state keeps running.",
            false,
        );
        let clear_rect =
            Rect::from_min_size(top_rect.min + Vec2::new(628.0, 2.0), Vec2::new(82.0, 24.0));
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
    let left_w = 468.0;
    let top_h = 186.0;
    let drive_bay = Rect::from_min_size(area.min, Vec2::new(left_w, top_h));
    let scope_bay = Rect::from_min_size(
        area.min + Vec2::new(left_w + 10.0, 0.0),
        Vec2::new((area.width() - left_w - 10.0).max(220.0), top_h),
    );
    let tone_bay = Rect::from_min_size(
        area.min + Vec2::new(0.0, top_h + 10.0),
        Vec2::new(left_w, 134.0),
    );
    let duck_bay = Rect::from_min_size(
        area.min + Vec2::new(left_w + 10.0, top_h + 10.0),
        Vec2::new(scope_bay.width(), 134.0),
    );

    controls::instrument::bay(ui, drive_bay, "Drive & Curve", accent, false);
    if let Module::EmberformDrive(drive) = &mut bus.module {
        let mut points = [0.0_f32; 65];
        Session::fx_drive_curve(drive, &mut points);
        controls::instrument::scope_frame(ui, scope_bay, "Transfer Curve", &points, accent);
        choice(
            ui,
            Rect::from_min_size(
                drive_bay.min + Vec2::new(12.0, 28.0),
                Vec2::new(126.0, 24.0),
            ),
            &format!("{PREFIX} drive type"),
            &mut drive.shape,
            &DistortionShape::ALL.map(|s| (s, s.label())),
            "Select wave-shaper or bitcrusher model.",
        );
        bus_switch(
            ui,
            drive_bay.min + Vec2::new(150.0, 28.0),
            &format!("{PREFIX} level match"),
            "Level match",
            &mut drive.level_match,
            "Equalize output loudness to match driven input.",
            false,
        );
    }
    let drive_top = Rect::from_min_max(drive_bay.min + Vec2::new(0.0, 32.0), drive_bay.max);
    draw_dial_grid(
        ui,
        drive_top,
        4,
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
        5,
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
        4,
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
    let left_w = 468.0;
    let top_h = 178.0;
    let bot_h = 146.0;
    let right_w = (area.width() - left_w - 10.0).max(220.0);
    let time_bay = Rect::from_min_size(area.min, Vec2::new(left_w, top_h));
    let scope_bay = Rect::from_min_size(
        area.min + Vec2::new(left_w + 10.0, 0.0),
        Vec2::new(right_w, top_h),
    );
    let tone_bay = Rect::from_min_size(
        area.min + Vec2::new(0.0, top_h + 10.0),
        Vec2::new(left_w, bot_h),
    );
    let duck_bay = Rect::from_min_size(
        area.min + Vec2::new(left_w + 10.0, top_h + 10.0),
        Vec2::new(right_w, bot_h),
    );

    controls::instrument::bay(ui, time_bay, "Time & Feedback", accent, false);
    if let Module::EchoformDelay(delay) = &mut bus.module {
        let mut sync = delay.mode == TimeMode::Sync;
        bus_switch(
            ui,
            time_bay.min + Vec2::new(12.0, 26.0),
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
            time_bay.min + Vec2::new(82.0, 26.0),
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
            time_bay.min + Vec2::new(168.0, 26.0),
            &format!("{PREFIX} ping-pong"),
            "Ping-pong",
            &mut delay.ping_pong,
            "Alternate echoes across stereo channels.",
            false,
        );
        choice(
            ui,
            Rect::from_min_size(time_bay.min + Vec2::new(266.0, 26.0), Vec2::new(82.0, 22.0)),
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
                Rect::from_min_size(time_bay.min + Vec2::new(12.0, 54.0), Vec2::new(66.0, 22.0)),
                PREFIX,
                delay,
            );
            ui.add_enabled_ui(!delay.link_times, |ui| {
                note_division(
                    ui,
                    Rect::from_min_size(
                        time_bay.min + Vec2::new(86.0, 54.0),
                        Vec2::new(66.0, 22.0),
                    ),
                    &format!("{PREFIX} right division"),
                    &mut delay.division_r,
                );
            });
        });
    }
    let time_grid = Rect::from_min_max(time_bay.min + Vec2::new(0.0, 62.0), time_bay.max);
    draw_dial_grid(
        ui,
        time_grid,
        4,
        bus,
        &[
            Slot::Delay(DelayParam::TimeL),
            Slot::Delay(DelayParam::TimeR),
            Slot::Delay(DelayParam::Feedback),
            Slot::Return,
        ],
    );

    controls::instrument::bay(ui, duck_bay, "Post-Limiter Ducker", accent, false);
    draw_dial_grid_with(
        ui,
        duck_bay,
        4,
        55.0,
        true,
        bus,
        &[
            Slot::Delay(DelayParam::DuckKey),
            Slot::Delay(DelayParam::DuckThreshold),
            Slot::Delay(DelayParam::DuckAmount),
            Slot::Delay(DelayParam::DuckRelease),
        ],
    );

    controls::instrument::bay(ui, tone_bay, "Tone, Smear & Mod", accent, false);
    draw_dial_grid_with(
        ui,
        tone_bay,
        9,
        49.0,
        true,
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
    let space_bay = Rect::from_min_size(area.min, Vec2::new(area.width(), 174.0));
    let tone_bay = Rect::from_min_size(area.min + Vec2::new(0.0, 182.0), Vec2::new(458.0, 148.0));
    let duck_bay = Rect::from_min_size(
        area.min + Vec2::new(468.0, 182.0),
        Vec2::new((area.width() - 468.0).max(250.0), 148.0),
    );

    controls::instrument::bay(ui, space_bay, "Space, Damping & Shimmer", accent, false);
    if let Module::AetherformReverb(reverb) = &mut bus.module {
        choice(
            ui,
            Rect::from_min_size(
                space_bay.min + Vec2::new(12.0, 26.0),
                Vec2::new(104.0, 22.0),
            ),
            &format!("{PREFIX} space"),
            &mut reverb.space_model,
            &SpaceModel::ALL.map(|m| (m, m.label())),
            "Acoustic space model.",
        );
        choice(
            ui,
            Rect::from_min_size(
                space_bay.min + Vec2::new(124.0, 26.0),
                Vec2::new(98.0, 22.0),
            ),
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
            space_bay.min + Vec2::new(234.0, 26.0),
            &format!("{PREFIX} early reflections"),
            "Early",
            &mut reverb.early_reflections_enabled,
            "Initial room reflections.",
            false,
        );
        bus_switch(
            ui,
            space_bay.min + Vec2::new(312.0, 26.0),
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
                Rect::from_min_size(
                    space_bay.min + Vec2::new(404.0, 26.0),
                    Vec2::new(70.0, 22.0),
                ),
                &format!("{PREFIX} shimmer interval"),
                &mut reverb.shimmer_interval_semitones,
                &labels,
                "Shimmer pitch interval in semitones.",
            );
        });
    }
    let space_grid = Rect::from_min_max(space_bay.min + Vec2::new(0.0, 32.0), space_bay.max);
    draw_dial_grid(
        ui,
        space_grid,
        10,
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
            Slot::Return,
        ],
    );

    controls::instrument::bay(ui, tone_bay, "Tone, Modulation & Output", accent, false);
    draw_dial_grid(
        ui,
        tone_bay,
        7,
        bus,
        &[
            Slot::Reverb(ReverbParam::LowCut),
            Slot::Reverb(ReverbParam::HighCut),
            Slot::Reverb(ReverbParam::ModRate),
            Slot::Reverb(ReverbParam::ModDepth),
            Slot::Reverb(ReverbParam::Drive),
            Slot::Reverb(ReverbParam::Width),
            Slot::Reverb(ReverbParam::Output),
        ],
    );

    controls::instrument::bay(ui, duck_bay, "Post-Limiter Ducker", accent, false);
    draw_dial_grid(
        ui,
        duck_bay,
        4,
        bus,
        &[
            Slot::Reverb(ReverbParam::DuckKey),
            Slot::Reverb(ReverbParam::DuckThreshold),
            Slot::Reverb(ReverbParam::DuckAmount),
            Slot::Reverb(ReverbParam::DuckRelease),
        ],
    );
}

fn draw_module_workspace(ui: &mut egui::Ui, area: Rect, bus: &mut FxBus, bpm: f64) {
    let ui = &mut ui.new_child(egui::UiBuilder::new().id_salt("fx-bays").max_rect(area));
    let accent = bay_accent(&bus.module);
    match bus.module {
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
    }
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
                    ui.add_space(6.0);
                    let area = ui
                        .allocate_exact_size(Vec2::new(724.0, 340.0), Sense::hover())
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

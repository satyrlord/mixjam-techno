//! FX returns board of the lower panel: Layer 1 compact four-card macro rack
//! paired with Layer 2 Deep Edit floating window (`deep_edit.rs`).
use super::*;
use mixjam_techno::application::FxBusMeterReading;
use mixjam_techno::application::fx::{BUSES, FxBuses, Module};
#[path = "fx/deep_edit.rs"]
mod deep_edit;
#[path = "fx/faces.rs"]
mod faces;
#[path = "fx/unit.rs"]
mod unit;
use unit::{CardAction, unit};

pub(super) const DEEP_EDIT_TITLE: &str = "FX deep edit";

/// One display reading for both FX views. Reduced mode holds the reading and
/// keeps peaks and gain reduction observed between updates.
#[derive(Default)]
pub(super) struct MeterDisplay {
    shown: [FxBusMeterReading; BUSES],
    pending: [FxBusMeterReading; BUSES],
    next: f64,
}

impl MeterDisplay {
    pub(super) fn read(&self) -> [FxBusMeterReading; BUSES] {
        self.shown
    }

    pub(super) fn sample(
        &mut self,
        readings: Option<[FxBusMeterReading; BUSES]>,
        reduced: bool,
        now: f64,
    ) {
        let Some(readings) = readings else {
            *self = Self::default();
            return;
        };
        if reduced {
            for (pending, reading) in self.pending.iter_mut().zip(readings) {
                *pending = FxBusMeterReading {
                    peak_l: pending.peak_l.max(reading.peak_l),
                    peak_r: pending.peak_r.max(reading.peak_r),
                    ducker_gr_db: pending.ducker_gr_db.max(reading.ducker_gr_db),
                    limiter_gr_db: pending.limiter_gr_db.max(reading.limiter_gr_db),
                    ..reading
                };
            }
            if now < self.next {
                return;
            }
            self.shown = self.pending;
        } else {
            self.shown = readings;
        }
        self.next = now + 1.0 / 15.0;
        self.pending = Default::default();
    }
}

#[cfg(test)]
#[test]
fn reduced_fx_meters_hold_their_readings_keep_peaks_and_reset_after_stop() {
    let mut display = MeterDisplay::default();
    let quiet = [FxBusMeterReading::default(); BUSES];
    let mut transient = quiet;
    transient[1] = FxBusMeterReading {
        peak_l: 0.7,
        peak_r: 0.8,
        rms_l: 0.2,
        rms_r: 0.2,
        ducker_gr_db: 4.0,
        limiter_gr_db: 2.0,
        tail_active: true,
    };
    display.sample(Some(quiet), true, 0.0);
    display.sample(Some(transient), true, 0.03);
    assert_eq!(display.read(), quiet);
    display.sample(Some(quiet), true, 0.06);
    assert_eq!(display.read(), quiet);
    display.sample(Some(quiet), true, 0.07);
    let held = display.read();
    assert_eq!(held[1].peak_l, 0.7);
    assert_eq!(held[1].peak_r, 0.8);
    assert_eq!(held[1].ducker_gr_db, 4.0);
    assert_eq!(held[1].limiter_gr_db, 2.0);
    assert_eq!(held[1].rms_l, 0.0);
    assert!(!held[1].tail_active);
    assert_eq!(held[0], quiet[0]);
    display.sample(Some(transient), false, 0.08);
    assert_eq!(display.read(), transient);
    display.sample(None, true, 0.09);
    assert_eq!(display.read(), quiet);
    display.sample(Some(transient), true, 0.10);
    assert_eq!(display.read(), transient);
}

pub(super) enum Action {
    None,
    Change(Box<FxBuses>),
    ClearTail(usize),
    ToggleSolo(usize),
    OpenDeepEdit(usize),
}

const MARGIN: f32 = 6.0;
const GAP: f32 = 8.0;
const UNIT_TOP: f32 = 8.0;
const CARD_MIN_WIDTH: f32 = 198.0;
const UNIT_HEIGHT: f32 = 228.0;
const HEADER: f32 = 36.0;
/// The widest a dial's column grows on a wide card, so the dials stay a
/// group and do not drift to the card's edges.
const MAX_CELL: f32 = 76.0;
const PADDING: f32 = 12.0;
const SELECT_Y: f32 = HEADER + 6.0;
const DIALS_Y: f32 = 82.0;
/// The meter row, clear of bottom corner screws and dial captions.
const FOOTER_Y: f32 = 186.0;
/// The meter row's height: room for three lettered lanes.
const FOOTER_H: f32 = 26.0;
const ROW_HEIGHT: f32 = UNIT_TOP + UNIT_HEIGHT + 16.0;
const BOARD_INSET: f32 = 2.0;
/// Lower panel preferred height showing the full 4-card macro rack cleanly.
pub(super) const MODULE_HEIGHT: f32 = ROW_HEIGHT + 2.0 * BOARD_INSET + 64.0;

pub(super) fn fx_motif(module: &Module) -> (super::master::MotifSpec, Color32) {
    use super::master::{MotifSpec, MotifStyle};
    match module {
        Module::EchoformDelay(_) => (
            MotifSpec {
                base: [16, 26, 44],
                secondary: [12, 56, 78],
                trim: [128, 226, 246],
                light_face: false,
                style: MotifStyle::DualSubBays,
                knob: theme::KnobStyle::Anodized([56, 172, 216]),
            },
            Color32::from_rgb(96, 214, 242),
        ),
        Module::AetherformReverb(_) => (
            MotifSpec {
                base: [20, 30, 82],
                secondary: [42, 28, 96],
                trim: [186, 176, 252],
                light_face: false,
                style: MotifStyle::CosmicNebula,
                knob: theme::KnobStyle::BrushedChrome([20, 44, 92]),
            },
            Color32::from_rgb(184, 168, 250),
        ),
        Module::EmberformDrive(_) => (
            MotifSpec {
                base: [96, 26, 20],
                secondary: [72, 20, 16],
                trim: [250, 184, 112],
                light_face: false,
                style: MotifStyle::DriveRail,
                knob: theme::KnobStyle::Anodized([216, 110, 44]),
            },
            Color32::from_rgb(248, 156, 86),
        ),
        Module::Empty => (
            MotifSpec {
                base: [15, 17, 19],
                secondary: [36, 40, 44],
                trim: [108, 116, 122],
                light_face: false,
                style: MotifStyle::EmptyBay,
                knob: theme::KnobStyle::Default,
            },
            Color32::from_rgb(136, 144, 148),
        ),
    }
}

/// The board's action for a press on bus `index`'s card, if any.
fn board_action(pressed: CardAction, index: usize) -> Option<Action> {
    match pressed {
        CardAction::ClearTail => Some(Action::ClearTail(index)),
        CardAction::ToggleSolo => Some(Action::ToggleSolo(index)),
        CardAction::OpenDeepEdit => Some(Action::OpenDeepEdit(index)),
        CardAction::None => None,
    }
}

/// Show the compact 4-card FX rack in `ui`. `reveal` scrolls bus `index` into view if narrowed.
pub(super) fn show(
    ui: &mut egui::Ui,
    buses: &FxBuses,
    reveal: Option<usize>,
    meters: &[FxBusMeterReading; BUSES],
    solos: [bool; BUSES],
) -> Action {
    let mut draft = *buses;
    let mut action = Action::None;
    let area = ui.available_rect_before_wrap();
    ui.advance_cursor_after_rect(area);
    let mut board = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("effects-board")
            .max_rect(area),
    );
    dialog::hardware_style(&mut board, controls::Palette::Marine);
    board.set_clip_rect(area.intersect(ui.clip_rect()));
    controls::panel(&board, area);
    named(
        &board.interact(area, board.id().with("surface"), Sense::hover()),
        egui::WidgetType::Other,
        "FX returns board",
    );
    let mut content = board.new_child(
        egui::UiBuilder::new()
            .id_salt("effects-content")
            .max_rect(area.shrink(BOARD_INSET)),
    );
    let card_w = ((content.available_width() - 2.0 * MARGIN - (BUSES - 1) as f32 * GAP)
        / BUSES as f32)
        .max(CARD_MIN_WIDTH);
    let natural = 2.0 * MARGIN + BUSES as f32 * card_w + (BUSES - 1) as f32 * GAP;
    let mut scroll = egui::ScrollArea::both()
        .id_salt("effects-scroll")
        .auto_shrink([false, false]);
    if let Some(index) = reveal {
        scroll = scroll.horizontal_scroll_offset(index as f32 * (card_w + GAP));
    }
    scroll.show(&mut content, |ui| {
        let size = Vec2::new(ui.available_width().max(natural), ROW_HEIGHT);
        let (bounds, _) = ui.allocate_exact_size(size, Sense::hover());
        for index in 0..BUSES {
            let left = MARGIN + index as f32 * (card_w + GAP);
            let rect = Rect::from_min_size(
                Pos2::new(bounds.left() + left, bounds.top() + UNIT_TOP),
                Vec2::new(card_w, UNIT_HEIGHT),
            );
            // A child Ui, unlike a scope, leaves the row's height alone, so
            // the scroll area holds exactly the row.
            let mut card = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("fx-unit", index))
                    .max_rect(rect),
            );
            let pressed = unit(
                &mut card,
                rect,
                index,
                &mut draft.buses[index],
                meters[index],
                solos[index],
            );
            if let Some(pressed) = board_action(pressed, index) {
                action = pressed;
            }
        }
    });
    view::name_scroll_area(&content, "effects-scroll", "Effects");
    match action {
        Action::None if draft != *buses => Action::Change(Box::new(draft)),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::super::master::tests::worst_zone_contrast;
    use super::*;

    #[test]
    fn ink_stays_legible_on_every_unit_lit_or_off() {
        let modules = [
            Module::Empty,
            Module::EchoformDelay(Default::default()),
            Module::AetherformReverb(Default::default()),
            Module::EmberformDrive(Default::default()),
        ];
        for theme_kind in theme::Theme::ALL {
            theme::paint_as(theme_kind);
            for module in &modules {
                let (motif, _) = fx_motif(module);
                for on in [true, false] {
                    let ([zone_a, zone_b], _) = motif.zones(on);
                    for (idx, zone) in [zone_a, zone_b]
                        .into_iter()
                        .chain(motif.extra_band_zones(on))
                        .enumerate()
                    {
                        let ratio = worst_zone_contrast(zone, roles().legend, motif.light_face, on);
                        assert!(
                            ratio >= 5.0,
                            "{theme_kind:?} FX zone {idx} (on={on}) gives {ratio:.2}:1"
                        );
                    }
                }
            }
        }
        theme::paint_as(theme::Theme::Studio);
    }

    #[test]
    fn deep_edit_ink_and_scopes_stay_legible_on_every_unit() {
        let modules = [
            Module::Empty,
            Module::EchoformDelay(Default::default()),
            Module::AetherformReverb(Default::default()),
            Module::EmberformDrive(Default::default()),
        ];
        for theme_kind in theme::Theme::ALL {
            theme::paint_as(theme_kind);
            let r = roles();
            let (scope_bg, tint_target, tint_mix) = match r.material {
                theme::Material::Metal => (
                    Color32::from_rgb(12, 18, 15),
                    Color32::from_rgb(236, 248, 232),
                    0.14_f32,
                ),
                theme::Material::Plastic => (
                    Color32::from_rgb(8, 22, 48),
                    Color32::from_rgb(230, 248, 255),
                    0.16,
                ),
                theme::Material::Flat => (r.strip_slot, Color32::WHITE, 0.0),
            };
            for module in &modules {
                let (motif, _) = fx_motif(module);
                let ink = motif.face_ink();
                for active in [true, false] {
                    let ([zone_a, zone_b], _) = motif.deep_zones(active);
                    for (idx, zone) in [zone_a, zone_b]
                        .into_iter()
                        .chain(motif.deep_extra_band_zones(active))
                        .enumerate()
                    {
                        let ratio = worst_zone_contrast(zone, ink, motif.light_face, active);
                        assert!(
                            ratio >= 5.0,
                            "{theme_kind:?} Deep Edit FX zone {idx} (active={active}) gives {ratio:.2}:1"
                        );
                    }
                }
                if !module.is_empty() {
                    let accent = motif.scope_accent();
                    let trace = if tint_mix > 0.0 {
                        accent.lerp_to_gamma(tint_target, tint_mix)
                    } else {
                        accent
                    };
                    let bg = match r.material {
                        theme::Material::Metal => scope_bg.lerp_to_gamma(accent, 0.05),
                        theme::Material::Plastic => scope_bg.lerp_to_gamma(accent, 0.07),
                        theme::Material::Flat => scope_bg,
                    };
                    let ratio = controls::contrast(bg, trace);
                    assert!(
                        ratio >= 5.0,
                        "{theme_kind:?} Deep Edit FX scope trace gives {ratio:.2}:1"
                    );
                }
            }
        }
        theme::paint_as(theme::Theme::Studio);
    }
}

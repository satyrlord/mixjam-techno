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

pub(super) const DEEP_EDIT_TITLE: &str = "FX Deep Edit";

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
const UNIT_HEIGHT: f32 = 236.0;
const HEADER: f32 = 34.0;
const CARD_CELL: f32 = 48.0;
const PADDING: f32 = 8.0;
const SELECT_Y: f32 = HEADER + 4.0;
const DIALS_Y: f32 = SELECT_Y + 32.0;
const FOOTER_Y: f32 = 196.0;
const ROW_HEIGHT: f32 = UNIT_TOP + UNIT_HEIGHT + 16.0;
const BOARD_INSET: f32 = 2.0;
/// Lower panel preferred height showing the full 4-card macro rack cleanly.
pub(super) const MODULE_HEIGHT: f32 = ROW_HEIGHT + 2.0 * BOARD_INSET + 64.0;

pub(super) const DELAY_ENAMEL: [u8; 3] = [112, 192, 214];
pub(super) const REVERB_ENAMEL: [u8; 3] = [184, 168, 234];
pub(super) const DRIVE_ENAMEL: [u8; 3] = [234, 164, 120];
pub(super) const EMPTY_ENAMEL: [u8; 3] = [196, 194, 186];

pub(super) fn enamel(module: &Module) -> [u8; 3] {
    match module {
        Module::Empty => EMPTY_ENAMEL,
        Module::EchoformDelay(_) => DELAY_ENAMEL,
        Module::AetherformReverb(_) => REVERB_ENAMEL,
        Module::EmberformDrive(_) => DRIVE_ENAMEL,
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
            ui.push_id(("fx-unit", index), |ui| {
                match unit(
                    ui,
                    rect,
                    index,
                    &mut draft.buses[index],
                    meters[index],
                    solos[index],
                ) {
                    CardAction::ClearTail => action = Action::ClearTail(index),
                    CardAction::ToggleSolo => action = Action::ToggleSolo(index),
                    CardAction::OpenDeepEdit => action = Action::OpenDeepEdit(index),
                    CardAction::None => {}
                }
            });
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
    use super::super::master::enamel_of;
    use super::*;

    #[test]
    fn ink_stays_legible_on_every_unit_lit_or_off() {
        for colour in [DELAY_ENAMEL, REVERB_ENAMEL, DRIVE_ENAMEL, EMPTY_ENAMEL] {
            for on in [true, false] {
                let ratio = controls::contrast(enamel_of(colour, on), roles().ink);
                assert!(ratio >= 5.0, "{colour:?} ({on}) gives {ratio:.2}:1");
            }
        }
    }
}

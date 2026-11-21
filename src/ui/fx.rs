//! FX returns board of the lower panel: the four FX buses that the mixer's
//! sends feed, side by side as units on a board. Each unit holds its effect,
//! the effect's preset and settings, the bus's input switch and return level,
//! its safety limiter, and Clear Tail. The caller turns changed buses into an
//! edit and a Clear Tail request into a session call.
use super::*;
use mixjam_techno::application::fx::{BUSES, FxBuses, Module};
#[path = "fx/faces.rs"]
mod faces;
#[path = "fx/unit.rs"]
mod unit;
use faces::unit_width;
use unit::unit;

pub(super) enum Action {
    None,
    Change(Box<FxBuses>),
    ClearTail(usize),
}

const MARGIN: f32 = 8.0;
const GAP: f32 = 14.0;
const UNIT_TOP: f32 = 10.0;
const UNIT_HEIGHT: f32 = 364.0;
const HEADER: f32 = 36.0;
/// One dial with its value well and label, and the pitch of dial rows.
const CELL: f32 = 62.0;
const ROW: f32 = 96.0;
const PADDING: f32 = 14.0;
/// The effect and preset selectors, then the effect's switches, then two
/// rows of dials, then the bus's switches.
const SELECT_Y: f32 = HEADER + 8.0;
const MODE_Y: f32 = SELECT_Y + 34.0;
const DIALS_Y: f32 = MODE_Y + 36.0;
const FOOTER_Y: f32 = DIALS_Y + 2.0 * ROW;
/// Room under the units for their shadows and the row's scroll bar.
const ROW_HEIGHT: f32 = UNIT_TOP + UNIT_HEIGHT + 20.0;
const BOARD_INSET: f32 = 2.0;
/// The lower panel's height that shows the whole board.
pub(super) const MODULE_HEIGHT: f32 = ROW_HEIGHT + 2.0 * BOARD_INSET + 64.0;

const DELAY_ENAMEL: [u8; 3] = [112, 192, 214];
const REVERB_ENAMEL: [u8; 3] = [184, 168, 234];
const DRIVE_ENAMEL: [u8; 3] = [234, 164, 120];
const EMPTY_ENAMEL: [u8; 3] = [196, 194, 186];

fn enamel(module: &Module) -> [u8; 3] {
    match module {
        Module::Empty => EMPTY_ENAMEL,
        Module::EchoformDelay(_) => DELAY_ENAMEL,
        Module::AetherformReverb(_) => REVERB_ENAMEL,
        Module::EmberformDrive(_) => DRIVE_ENAMEL,
    }
}
/// Show the board in the space left in `ui`. `reveal` brings a bus into view.
pub(super) fn show(ui: &mut egui::Ui, buses: &FxBuses, reveal: Option<usize>) -> Action {
    let mut draft = *buses;
    let mut clear = None;
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
    let lefts: Vec<f32> = buses
        .buses
        .iter()
        .scan(MARGIN, |left, bus| {
            let here = *left;
            *left += unit_width(&bus.module) + GAP;
            Some(here)
        })
        .collect();
    let natural = lefts[BUSES - 1] + unit_width(&buses.buses[BUSES - 1].module) + MARGIN;
    let mut scroll = egui::ScrollArea::both()
        .id_salt("effects-scroll")
        .auto_shrink([false, false]);
    if let Some(index) = reveal {
        scroll = scroll.horizontal_scroll_offset((lefts[index] - MARGIN).max(0.0));
    }
    scroll.show(&mut content, |ui| {
        let size = Vec2::new(ui.available_width().max(natural), ROW_HEIGHT);
        let (bounds, _) = ui.allocate_exact_size(size, Sense::hover());
        for (index, left) in lefts.iter().enumerate() {
            let rect = Rect::from_min_size(
                Pos2::new(bounds.left() + left, bounds.top() + UNIT_TOP),
                Vec2::new(unit_width(&buses.buses[index].module), UNIT_HEIGHT),
            );
            ui.push_id(("fx-unit", index), |ui| {
                if unit(ui, rect, index, &mut draft.buses[index]) {
                    clear = Some(index);
                }
            });
        }
    });
    view::name_scroll_area(&content, "effects-scroll", "Effects");
    if let Some(index) = clear {
        Action::ClearTail(index)
    } else if draft != *buses {
        Action::Change(Box::new(draft))
    } else {
        Action::None
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

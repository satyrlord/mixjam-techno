//! Section bookmarks on the bar ruler: each section's colored tag, and adding,
//! renaming, and deleting a section.
use super::*;
use mixjam_techno::application::{MAX_SECTION_NAME, Section};

/// Section bookmark colors, in section order; a song with more sections
/// repeats them.
pub(in super::super) fn section_color(index: usize) -> Color32 {
    let sections = &roles().sections;
    sections[index % sections.len()]
}

/// Paint each section's tag on the bar ruler: a colored line at its start and
/// its name on a chip that ends before the next section. Returns each chip
/// with its section's start.
pub(super) fn section_tags(
    ui: &egui::Ui,
    painter: &egui::Painter,
    ruler: Rect,
    bar_width: f32,
    sections: &[Section],
) -> Vec<(u32, Rect)> {
    let x_of = |tick: u32| ruler.left() + tick as f32 / TICKS_PER_BAR as f32 * bar_width;
    let mut chips = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let color = section_color(index);
        let x = x_of(section.start_tick);
        let limit = sections
            .get(index + 1)
            .map_or(f32::INFINITY, |next| x_of(next.start_tick) - 2.0);
        painter.line_segment(
            [Pos2::new(x, ruler.top()), Pos2::new(x, ruler.bottom())],
            Stroke::new(2.0_f32, color),
        );
        let galley = ui.painter().layout_no_wrap(
            section.name.clone(),
            FontId::proportional(11.0),
            roles().on_light,
        );
        let chip = Rect::from_min_size(
            Pos2::new(x, ruler.top() + 4.0),
            Vec2::new(galley.size().x + 10.0, ruler.height() - 8.0),
        );
        let visible =
            Rect::from_min_max(chip.min, Pos2::new(chip.right().min(limit), chip.bottom()));
        if visible.width() < 4.0 {
            continue;
        }
        let chip_painter = painter.with_clip_rect(visible.intersect(painter.clip_rect()));
        chip_painter.rect_filled(
            chip,
            egui::CornerRadius {
                nw: 0,
                sw: 0,
                ne: 4,
                se: 4,
            },
            color,
        );
        chip_painter.galley(
            Pos2::new(chip.left() + 5.0, chip.center().y - galley.size().y / 2.0),
            galley,
            roles().on_light,
        );
        chips.push((section.start_tick, visible));
    }
    chips
}

/// The section bookmarks on the bar ruler: whether they show, and their
/// rename field and context menu.
#[derive(Default)]
pub(in super::super) struct SectionState {
    /// Whether the arrangement shows the song's section bookmarks.
    pub(in super::super) bookmarks: bool,
    /// The section whose name is being edited on the bar ruler.
    pub(in super::super) rename: Option<SectionRename>,
    /// What the bar ruler's context menu acts on: the start of the section
    /// whose tag was right-clicked, or `None` away from every tag, and the bar
    /// that was right-clicked.
    pub(in super::super) menu: (Option<u32>, u32),
}

/// A section name being edited on the bar ruler.
pub(in super::super) struct SectionRename {
    /// The start of the section, which identifies it while the list changes.
    pub(in super::super) start_tick: u32,
    pub(in super::super) name: String,
    /// The editor takes the keyboard focus on its first frame.
    pub(in super::super) focused: bool,
}

/// The first "Section N" name that no section of `sections` has.
fn new_section_name(sections: &[Section]) -> String {
    (1..)
        .map(|n| format!("Section {n}"))
        .find(|name| sections.iter().all(|section| section.name != *name))
        .unwrap_or_default()
}

impl MixJamApp {
    /// Double-click the bar ruler to add a section at that bar, or a tag to
    /// rename it; right-click for the same choices and Delete.
    pub(super) fn section_gestures(
        &mut self,
        ui: &mut egui::Ui,
        ruler_response: &Response,
        ruler: Rect,
        tags: &[(u32, Rect)],
    ) {
        let tag_at = |pos: Pos2| {
            tags.iter()
                .find(|(_, rect)| rect.contains(pos))
                .map(|(tick, _)| *tick)
        };
        if ruler_response.double_clicked()
            && let Some(pos) = ruler_response.interact_pointer_pos()
        {
            // The first click of a double-click starts no loop.
            self.score.loop_start = None;
            match tag_at(pos) {
                Some(tick) => self.rename_section(tick),
                None => self.add_section(self.position_tick(pos.x, ruler, false)),
            }
        }
        if ruler_response.secondary_clicked() {
            let pos = ruler_response.interact_pointer_pos();
            self.sections.menu = (
                pos.and_then(tag_at),
                pos.map_or(self.session.cursor_tick(), |pos| {
                    self.position_tick(pos.x, ruler, false)
                }),
            );
        }
        let (tag, bar) = self.sections.menu;
        ruler_response.context_menu(|ui| match tag {
            Some(tick) => {
                if ui.button("Rename section").clicked() {
                    self.rename_section(tick);
                    ui.close();
                }
                if ui.button("Delete section").clicked() {
                    let mut sections = self.session.project().sections.clone();
                    sections.retain(|section| section.start_tick != tick);
                    let result = self.session.edit(EditCommand::SetSections(sections));
                    self.apply(result);
                    ui.close();
                }
            }
            None => {
                if ui.button("Add section here").clicked() {
                    self.add_section(bar);
                    ui.close();
                }
            }
        });
        self.section_rename_editor(ui, ruler);
    }

    /// Add a section named "Section N" at `tick` and edit its name. A section
    /// that already starts there is renamed instead.
    pub(in super::super) fn add_section(&mut self, tick: u32) {
        let sections = &self.session.project().sections;
        if !sections.iter().any(|section| section.start_tick == tick) {
            let mut sections = sections.clone();
            sections.push(Section {
                start_tick: tick,
                name: new_section_name(&sections),
            });
            sections.sort_by_key(|section| section.start_tick);
            let result = self.session.edit(EditCommand::SetSections(sections));
            if result.is_err() {
                self.apply(result);
                return;
            }
        }
        self.rename_section(tick);
    }

    fn rename_section(&mut self, tick: u32) {
        if let Some(section) = self
            .session
            .project()
            .sections
            .iter()
            .find(|section| section.start_tick == tick)
        {
            self.sections.rename = Some(SectionRename {
                start_tick: tick,
                name: section.name.clone(),
                focused: false,
            });
        }
    }

    /// The name editor over a section's tag. Enter or a click elsewhere keeps
    /// the name; Escape or an empty name keeps the old one.
    fn section_rename_editor(&mut self, ui: &mut egui::Ui, ruler: Rect) {
        let Some(rename) = &mut self.sections.rename else {
            return;
        };
        let x =
            ruler.left() + rename.start_tick as f32 / TICKS_PER_BAR as f32 * self.score.bar_width;
        let mut done = None;
        egui::Area::new(ui.id().with("section_rename"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(x, ruler.top() + 1.0))
            .show(ui.ctx(), |ui| {
                let response = name_control(
                    ui.add(
                        egui::TextEdit::singleline(&mut rename.name)
                            .char_limit(MAX_SECTION_NAME)
                            .desired_width(180.0)
                            .font(FontId::proportional(12.0)),
                    ),
                    "Section name",
                );
                if !rename.focused {
                    response.request_focus();
                    rename.focused = true;
                }
                if response.lost_focus() {
                    done = Some(!ui.input(|input| input.key_pressed(egui::Key::Escape)));
                }
            });
        let Some(keep) = done else {
            return;
        };
        let Some(rename) = self.sections.rename.take() else {
            return;
        };
        let name = rename.name.trim();
        if !keep || name.is_empty() {
            return;
        }
        let mut sections = self.session.project().sections.clone();
        if let Some(section) = sections
            .iter_mut()
            .find(|section| section.start_tick == rename.start_tick)
            && section.name != name
        {
            section.name = name.into();
            let result = self.session.edit(EditCommand::SetSections(sections));
            self.apply(result);
        }
    }
}

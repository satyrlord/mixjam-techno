//! The tool rail at the left edge: its artwork, its tools, their groups, and the
//! switch that shows their names.
use super::*;

impl MixJamApp {
    /// Load the bundled artwork once. A failure is reported once; the header
    /// then falls back to text actions and the tool rail stays hidden.
    pub(super) fn icons(&mut self, ctx: &egui::Context) -> Option<Arc<RailIcons>> {
        if self.rail_icons.is_none() {
            let loaded = RailIcons::load(ctx).map(Arc::new);
            if let Err(error) = &loaded {
                self.error(error.clone());
            }
            self.rail_icons = Some(loaded);
        }
        self.rail_icons.as_ref()?.as_ref().ok().cloned()
    }

    pub(in super::super) fn side_rail(&mut self, ctx: &egui::Context) {
        let Some(icons) = self.icons(ctx) else {
            return;
        };
        let layout = self.session.layout();
        if layout.maximised {
            return;
        }
        let names = layout.tool_names;
        egui::SidePanel::left("tools")
            .exact_width(rail_width(names))
            .resizable(false)
            .frame(egui::Frame::new().fill(roles().canvas).inner_margin(6))
            .show(ctx, |ui| {
                controls::apply_palette(ui, controls::Palette::Utility);
                controls::rail_panel(ui, ui.max_rect().expand(4.0));
                // Tools touch: their faces and hover states carry the rhythm.
                ui.spacing_mut().item_spacing.y = 0.0;
                // Options sits at the foot. Its place is kept first, but it is
                // drawn last, so Tab still reaches the tools from the top.
                let area = ui.available_rect_before_wrap();
                let foot_height = RAIL_FOOT_GAP + 2.0 * rail_button_size(names).y;
                let (body, foot) = area.split_top_bottom_at_y(area.bottom() - foot_height);
                ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                    let output = egui::ScrollArea::vertical()
                        .id_salt("tool-rail")
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| self.rail_tools(ui, ctx, &icons, names));
                        });
                    rail_scroll_edges(ui, &output);
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(foot), |ui| {
                    ui.vertical_centered(|ui| {
                        // With names, the foot stands apart by space alone, as
                        // the groups do; icons only, a separator divides it.
                        if names {
                            ui.add_space(RAIL_FOOT_GAP);
                        } else {
                            ui.add_space((RAIL_FOOT_GAP - RAIL_SEPARATOR) / 2.0);
                            controls::rail_separator(ui);
                            ui.add_space((RAIL_FOOT_GAP - RAIL_SEPARATOR) / 2.0);
                        }
                        if rail_tool(ui, &icons, RailIcon::Options, self.options.is_some(), names)
                            .clicked()
                        {
                            self.run_command(commands::Command::Options, ctx);
                        }
                        if commands::rail_commands_button(ui, names, self.commands.is_some())
                            .clicked()
                        {
                            self.commands = Some(commands::CommandList::new());
                        }
                    });
                });
            });
    }

    /// The rail's tools in their groups: File, Clips and Tools. With names
    /// off, each group heading is a separator.
    fn rail_tools(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        icons: &RailIcons,
        names: bool,
    ) {
        let tool = |ui: &mut egui::Ui, icon: RailIcon, active: bool| {
            rail_tool(ui, icons, icon, active, names)
        };
        let mut switch = None;
        if !names {
            // Icons only, there is no heading row: the switch has its own.
            let size = Vec2::new(rail_button_size(false).x.min(ui.available_width()), 16.0);
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            switch = Some(rail_names_switch(ui, rect, names));
        }
        // The rail draws its groups and tools from the command registry.
        for (index, (group, tools)) in commands::RAIL.iter().enumerate() {
            if let Some(response) = rail_group(ui, group, names, index == 0) {
                switch = Some(response);
            }
            for command in tools.iter().copied() {
                let Some(icon) = command.icon() else {
                    continue;
                };
                if tool(ui, icon, self.command_active(command)).clicked() {
                    self.run_command(command, ctx);
                }
            }
        }
        if switch.is_some_and(|response| response.clicked()) {
            self.set_layout(|layout| layout.tool_names = !layout.tool_names);
        }
    }
}

/// A muted icon that names the control beside it, as the microphone does
/// for the recording track.
pub(in super::super) fn icon_heading(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    width: f32,
) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::hover());
    ui.painter().image(
        icons.texture_id(icon),
        icons.icon_rect(ui.ctx(), rect.center()),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        roles().muted,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Image, ui.is_enabled(), icon.label())
    });
    response.hover_hint(icon.label());
}

/// The rail's width: icons with names, or icons only.
pub(in super::super) fn rail_width(names: bool) -> f32 {
    if names { 84.0 } else { 64.0 }
}

/// One rail tool's target: an icon over its name, or the icon alone. Both
/// leave a 6-point gutter to the chassis edge.
pub(in super::super) fn rail_button_size(names: bool) -> Vec2 {
    if names {
        Vec2::new(68.0, 40.0)
    } else {
        Vec2::new(48.0, 30.0)
    }
}

/// The space above Options at the rail's foot, its separator included.
const RAIL_FOOT_GAP: f32 = 12.0;
/// The height `controls::rail_separator` takes.
const RAIL_SEPARATOR: f32 = 8.0;
/// Every tool name's size. A long name is shortened, never shrunk.
const RAIL_NAME_SIZE: f32 = 10.5;
/// With names, the icon's centre below the target's top, and the name's
/// bottom above the target's bottom.
const RAIL_ICON_CENTRE: f32 = 13.0;
const RAIL_NAME_BOTTOM: f32 = 4.0;
/// How far the engraved scroll edge stops short of each side of the view.
const RAIL_EDGE_INSET: f32 = 6.0;

pub(super) fn rail_icon_color(ui: &egui::Ui, active: bool) -> Color32 {
    if !ui.is_enabled() {
        roles().muted.gamma_multiply(0.5)
    } else if active {
        roles().tools[0].1
    } else {
        roles().legend
    }
}

/// The rail marks its active tool in the workspace accent, so it reads as
/// part of the workspace in every theme.
fn rail_mark_color(ui: &egui::Ui, active: bool) -> Color32 {
    if ui.is_enabled() && active {
        roles().accent
    } else {
        rail_icon_color(ui, false)
    }
}

/// One rail target: its surface, the icon that `glyph` paints at the given
/// centre and colour, and with names on, `name` under the icon. Callers add
/// the accessible name and the tooltip.
pub(in super::super) fn rail_item(
    ui: &mut egui::Ui,
    names: bool,
    active: bool,
    name: &str,
    glyph: impl FnOnce(&egui::Ui, Pos2, Color32),
) -> Response {
    let size = rail_button_size(names);
    let width = size.x.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, size.y), Sense::click());
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    controls::rail_surface(ui, &response, active);
    let color = rail_mark_color(ui, active);
    let centre = if names {
        Pos2::new(rect.center().x, rect.top() + RAIL_ICON_CENTRE)
    } else {
        rect.center()
    };
    glyph(ui, centre, color);
    if names {
        let text_color = if !ui.is_enabled() {
            color
        } else if active || response.hovered() {
            roles().text
        } else {
            roles().legend
        };
        let galley = ui.painter().layout_no_wrap(
            name.to_owned(),
            egui::FontId::proportional(RAIL_NAME_SIZE),
            text_color,
        );
        let origin = Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.bottom() - RAIL_NAME_BOTTOM - galley.size().y,
        );
        ui.painter()
            .with_clip_rect(rect.intersect(ui.clip_rect()))
            .galley(origin, galley, text_color);
    }
    response
}

/// A rail tool. With names on, its rail name is printed under its icon in
/// the text face; the accessible name, toggle state and hint stay the same
/// in both states.
pub(in super::super) fn rail_tool(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    active: bool,
    names: bool,
) -> Response {
    let response = rail_item(ui, names, active, icon.rail_name(), |ui, centre, color| {
        ui.painter().image(
            icons.texture_id(icon),
            icons.icon_rect(ui.ctx(), centre),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            color,
        );
    });
    response.widget_info(|| {
        if icon.is_toggle() {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                active,
                icon.label(),
            )
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), icon.label())
        }
    });
    response.hover_hint(icon.hint())
}

/// An engraved edge wherever more tools lie past the scrolling view, so a
/// short window never hides tools without a cue. A line, not a fade: a fade
/// to one colour shows as a band on the plastic gradient and the metal grain.
fn rail_scroll_edges<R>(ui: &egui::Ui, output: &egui::scroll_area::ScrollAreaOutput<R>) {
    // The scroll area draws its content this far past the view, so the edge
    // sits where the tools end, not across them.
    let reach = Vec2::new(0.0, ui.visuals().clip_rect_margin);
    let view = output.inner_rect.expand2(reach);
    let offset = output.state.offset.y;
    let painter = ui.painter();
    let edge = |y: f32| {
        let left = Pos2::new(view.left() + RAIL_EDGE_INSET, y);
        let right = Pos2::new(view.right() - RAIL_EDGE_INSET, y);
        painter.line_segment([left, right], Stroke::new(1.0_f32, roles().ink));
        painter.line_segment(
            [left + Vec2::Y, right + Vec2::Y],
            Stroke::new(1.0_f32, Color32::from_white_alpha(45)),
        );
    };
    if offset > 0.5 {
        edge(view.top() + 0.5);
    }
    if output.content_size.y - offset - output.inner_rect.height() > 0.5 {
        edge(view.bottom() - 1.5);
    }
}

/// The double chevron that shows or hides the tool names, in `rect`.
pub(in super::super) fn rail_names_switch(ui: &mut egui::Ui, rect: Rect, names: bool) -> Response {
    let label = if names {
        "Shrink the tool rail to icons"
    } else {
        "Show the tool names"
    };
    let response = ui.interact(rect, ui.id().with("rail-names-switch"), Sense::click());
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    focus_outline(ui, &response, roles().accent);
    // A chevron that points where the rail goes: left to shrink, right to widen.
    let direction = if names { -1.0 } else { 1.0 };
    let color = if response.hovered() {
        roles().text
    } else {
        roles().muted
    };
    let stroke = Stroke::new(1.5_f32, color);
    for offset in [-3.0, 3.0] {
        let tip = rect.center() + Vec2::new(offset + direction * 2.5, 0.0);
        let back = tip - Vec2::new(direction * 4.0, 0.0);
        ui.painter()
            .line_segment([back + Vec2::new(0.0, -4.0), tip], stroke);
        ui.painter()
            .line_segment([back + Vec2::new(0.0, 4.0), tip], stroke);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.hover_hint(label)
}

/// A group heading over the rail's tools: a short, tracked uppercase title
/// with names on, a separator with icons only (none above the first group).
/// The first heading also holds the switch that shows or hides the names, at
/// its right end, and returns that switch's response.
pub(in super::super) fn rail_group(
    ui: &mut egui::Ui,
    title: &str,
    names: bool,
    first: bool,
) -> Option<Response> {
    if !names {
        if !first {
            controls::rail_separator(ui);
        }
        return None;
    }
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &title.to_uppercase(),
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(10.0),
            color: roles().muted,
            extra_letter_spacing: 0.6,
            ..Default::default()
        },
    );
    let galley = ui.painter().layout_job(job);
    // More space above a heading than below it: it belongs to its group.
    let above = if first { 2.0 } else { 8.0 };
    let height = (above + galley.size().y + 4.0).round();
    let width = rail_button_size(true).x.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    // The tracking trails the last letter; half of it is not part of the word.
    let text = Rect::from_min_size(
        Pos2::new(
            rect.center().x - (galley.size().x - 0.6) / 2.0,
            rect.top() + above,
        ),
        galley.size(),
    );
    ui.painter().galley(text.min, galley, roles().muted);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title));
    first.then(|| {
        let switch = Rect::from_center_size(
            Pos2::new(rect.right() - 9.0, text.center().y),
            Vec2::splat(18.0),
        );
        rail_names_switch(ui, switch, names)
    })
}

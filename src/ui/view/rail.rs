//! The tool rail at the left edge: its artwork, its tools, their groups, and the
//! switch that shows their names.
use super::*;

impl MixJamApp {
    /// Load the bundled artwork, again when the display density changes. A
    /// failure is reported once; the header then falls back to text actions
    /// and the tool rail stays hidden.
    pub(super) fn icons(&mut self, ctx: &egui::Context) -> Option<Arc<RailIcons>> {
        if self.rail_icons.as_ref().is_none_or(|loaded| {
            loaded
                .as_ref()
                .is_ok_and(|icons| !icons.matches_density(ctx))
        }) {
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
                ui.spacing_mut().item_spacing.y = 2.0;
                // Options sits at the foot. Its place is kept first, but it is
                // drawn last, so Tab still reaches the tools from the top.
                let area = ui.available_rect_before_wrap();
                let foot_height = RAIL_FOOT_GAP + 2.0 * rail_button_size(names).y + 2.0;
                let (body, foot) = area.split_top_bottom_at_y(area.bottom() - foot_height);
                ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("tool-rail")
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| self.rail_tools(ui, ctx, &icons, names));
                        });
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(foot), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(RAIL_FOOT_GAP - 8.0);
                        controls::rail_separator(ui);
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
        if rail_names_switch(ui, names).clicked() {
            self.set_layout(|layout| layout.tool_names = !layout.tool_names);
        }
        // The rail draws its groups and tools from the command registry.
        for (index, (group, tools)) in commands::RAIL.iter().enumerate() {
            rail_group(ui, group, names, index == 0);
            for command in tools.iter().copied() {
                let Some(icon) = command.icon() else {
                    continue;
                };
                if tool(ui, icon, self.command_active(command)).clicked() {
                    self.run_command(command, ctx);
                }
            }
        }
    }
}

pub(in super::super) fn rail_button(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    active: bool,
) -> Response {
    let width = 48.0_f32.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    controls::icon_button_surface(ui, &response, active);
    let color = rail_icon_color(ui, active);
    ui.painter().image(
        icons.texture_id(icon),
        icons.icon_rect(ui.ctx(), rect.center()),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        color,
    );
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

/// One rail tool's target: an icon over its name, or the icon alone.
pub(in super::super) fn rail_button_size(names: bool) -> Vec2 {
    if names {
        Vec2::new(72.0, 42.0)
    } else {
        Vec2::new(48.0, 30.0)
    }
}

/// The space above Options at the rail's foot, its separator included.
const RAIL_FOOT_GAP: f32 = 12.0;

pub(super) fn rail_icon_color(ui: &egui::Ui, active: bool) -> Color32 {
    if !ui.is_enabled() {
        roles().muted.gamma_multiply(0.5)
    } else if active {
        roles().tools[0].1
    } else {
        roles().legend
    }
}

/// A rail tool. With names on, the tool's accessible name is printed under
/// its icon in the text face; the name, toggle state and hint stay the same
/// in both states.
pub(in super::super) fn rail_tool(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    active: bool,
    names: bool,
) -> Response {
    if !names {
        return rail_button(ui, icons, icon, active);
    }
    let size = rail_button_size(true);
    let width = size.x.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, size.y), Sense::click());
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    controls::icon_button_surface(ui, &response, active);
    let color = rail_icon_color(ui, active);
    ui.painter().image(
        icons.texture_id(icon),
        icons.icon_rect(ui.ctx(), Pos2::new(rect.center().x, rect.top() + 13.0)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        color,
    );
    let text_color = if !ui.is_enabled() {
        color
    } else if active || response.hovered() {
        roles().text
    } else {
        roles().legend
    };
    // A long name steps down half a point at a time rather than clipping.
    let mut font = 11.0;
    let galley = loop {
        let galley = ui.painter().layout_no_wrap(
            icon.label().to_owned(),
            egui::FontId::proportional(font),
            text_color,
        );
        if galley.size().x <= rect.width() - 4.0 || font <= 9.0 {
            break galley;
        }
        font -= 0.5;
    };
    let origin = Pos2::new(
        rect.center().x - galley.size().x / 2.0,
        rect.bottom() - 4.0 - galley.size().y,
    );
    ui.painter().galley(origin, galley, text_color);
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

/// The button at the rail's top that shows or hides the tool names.
pub(in super::super) fn rail_names_switch(ui: &mut egui::Ui, names: bool) -> Response {
    let label = if names {
        "Shrink the tool rail to icons"
    } else {
        "Show the tool names"
    };
    let width = rail_button_size(names).x.min(ui.available_width().max(1.0));
    let height = if names { 20.0 } else { 16.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
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

/// A group heading over the rail's tools: a short uppercase title with
/// names on, a separator with icons only (none above the first group).
pub(in super::super) fn rail_group(ui: &mut egui::Ui, title: &str, names: bool, first: bool) {
    if !names {
        if !first {
            controls::rail_separator(ui);
        }
        return;
    }
    let height = if first { 14.0 } else { 20.0 };
    let width = rail_button_size(true).x.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let galley = ui.painter().layout_no_wrap(
        title.to_uppercase(),
        egui::FontId::proportional(9.5),
        roles().muted,
    );
    ui.painter().galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.bottom() - 1.0 - galley.size().y,
        ),
        galley,
        roles().muted,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title));
}

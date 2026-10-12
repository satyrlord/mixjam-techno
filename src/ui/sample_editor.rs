//! Sample Editor hardware presentation. Session owns sample processing and saving.
use super::*;

const CREAM: Color32 = Color32::from_rgb(245, 232, 205);
const AMBER: Color32 = Color32::from_rgb(245, 193, 86);
const GOLD: Color32 = Color32::from_rgb(229, 172, 60);
const INK: Color32 = Color32::from_rgb(24, 25, 23);
const BODY: Color32 = Color32::from_rgb(42, 46, 48);
const FACE: Color32 = Color32::from_rgb(18, 21, 21);
const EDGE: Color32 = Color32::from_rgb(102, 110, 110);
const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

macro_rules! pngs {
    ($name:literal) => {
        [
            include_bytes!(concat!("../../assets/sample-editor/100/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/sample-editor/150/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/sample-editor/200/", $name, ".png")).as_slice(),
        ]
    };
}

struct Artwork {
    density: usize,
    textures: Vec<egui::TextureHandle>,
}
impl Artwork {
    fn load(ctx: &egui::Context) -> Result<Arc<Self>, String> {
        let density = [1.0_f32, 1.5, 2.0]
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (**a - ctx.pixels_per_point())
                    .abs()
                    .total_cmp(&(**b - ctx.pixels_per_point()).abs())
            })
            .map_or(0, |(i, _)| i);
        let id = egui::Id::new(("sample-editor-artwork", density));
        if let Some(art) = ctx.data(|data| data.get_temp::<Arc<Self>>(id)) {
            return Ok(art);
        }
        let textures = [pngs!("panel"), pngs!("collapsed"), pngs!("thumb")]
            .iter()
            .enumerate()
            .map(|(index, variants)| {
                let image = image::load_from_memory(variants[density])
                    .map_err(|error| format!("Cannot load Sample Editor artwork: {error}"))?
                    .into_rgba8();
                Ok(ctx.load_texture(
                    format!("sample-editor-{density}-{index}"),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    ),
                    egui::TextureOptions::NEAREST,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let art = Arc::new(Self { density, textures });
        ctx.data_mut(|data| data.insert_temp(id, art.clone()));
        Ok(art)
    }
    fn paint(&self, ui: &egui::Ui, index: usize, center: Pos2) {
        let ppp = ui.ctx().pixels_per_point();
        let size = self.textures[index].size_vec2() / ppp;
        let min = ((center - size / 2.0).to_vec2() * ppp).round() / ppp;
        ui.painter().image(
            self.textures[index].id(),
            Rect::from_min_size(min.to_pos2(), size),
            UV,
            Color32::WHITE,
        );
    }
}

pub(super) struct Source<'a> {
    pub name: &'a str,
    pub frames: u64,
    pub sample_rate: u32,
    pub available: bool,
    pub missing: bool,
}

#[derive(Debug, PartialEq)]
pub(super) enum Action {
    None,
    Listen,
    Save,
    Details,
}

pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    start: &mut u64,
    end: &mut u64,
    pitch: &mut f64,
) -> Result<Action, String> {
    let art = Artwork::load(ctx)?;
    let scale = [1.0, 1.5, 2.0][art.density] / ctx.pixels_per_point();
    let collapse_id = egui::Id::new("sample-editor-collapsed");
    let collapsed = ctx
        .data(|data| data.get_temp::<bool>(collapse_id))
        .unwrap_or(false);
    let guard_id = egui::Id::new("sample-editor-escape-guard");
    let guarded = ctx
        .data(|data| data.get_temp::<bool>(guard_id))
        .unwrap_or(false);
    let mut action = Action::None;
    egui::Window::new("SAMPLE EDITOR").id(egui::Id::new("Sample editor"))
        .title_bar(false).resizable(false).collapsible(false).frame(egui::Frame::NONE)
        .default_pos(ctx.content_rect().center()-Vec2::new(320.0, 184.0)*scale)
        .show(ctx, |ui| {
            let (bounds, _) = ui.allocate_exact_size(Vec2::new(640.0, if collapsed {48.0} else {368.0})*scale, Sense::hover());
            let at = |x, y| bounds.min+Vec2::new(x,y)*scale;
            let rect = |x,y,w,h| Rect::from_min_size(at(x,y),Vec2::new(w,h)*scale);
            let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
            panel.set_clip_rect(bounds);
            if !collapsed { panel.painter().rect_filled(rect(0.0,304.0,640.0,64.0),10.0,BODY); }
            art.paint(&panel, usize::from(collapsed), at(320.0, if collapsed {24.0} else {160.0}));
            let header_y=if collapsed {24.0} else {32.0};
            text(&panel, at(320.0,header_y), "SAMPLE EDITOR",21.0*scale,CREAM,true);
            let mut editing = false;
            if !collapsed {
                let name = panel.put(rect(32.0,65.0,576.0,26.0),egui::Label::new(egui::RichText::new(source.name).size(13.0*scale).color(CREAM)).truncate());
                name_control(name.clone(),"Source sound");
                ctx.accesskit_node_builder(name.id, |node| node.set_value(source.name));
                name.on_hover_text(source.name);
                label(&panel,at(34.0,126.0),"Start frame",13.0*scale);
                label(&panel,at(326.0,126.0),"End frame",13.0*scale);
                panel.add_enabled_ui(source.available && source.frames>0, |ui| {
                    editing |= frame_value(ui,rect(128.0,104.0,148.0,44.0),scale,"Start frame",start,0..=source.frames.saturating_sub(1));
                    editing |= frame_value(ui,rect(416.0,104.0,188.0,44.0),scale,"End frame",end,1..=source.frames.max(1));
                    pitch_rail(ui,&art,rect(34.0,188.0,242.0,36.0),scale,pitch);
                    editing |= pitch_value(ui,rect(310.0,184.0,112.0,44.0),scale,pitch);
                });
                label(&panel,at(440.0,206.0),"Pitch (semitones)",13.0*scale);
                let reason = if source.missing { Some("This sound is missing and plays silence. Open Import details to inspect or resolve it.") }
                    else if !source.available || source.frames==0 { Some("Select a nonempty sound or clip before editing.") }
                    else if *start>=*end || *end>source.frames { Some("End frame must be greater than Start frame and within the source.") }
                    else if !pitch.is_finite() || !(-24.0..=24.0).contains(pitch) { Some("Pitch must be from -24 to 24 semitones.") } else { None };
                let ready = reason.is_none();
                let listen = action_button(&mut panel,rect(32.0,266.0,224.0,36.0),"Listen edit",scale,false,ready);
                if listen.on_hover_text("Preview the selected frames at this pitch. The source stays unchanged.").on_disabled_hover_text(reason.unwrap_or("Select a nonempty trim range.")).clicked() { action=Action::Listen; }
                let save = action_button(&mut panel,rect(284.0,266.0,324.0,36.0),"Save edited sound",scale,true,ready);
                if save.on_hover_text("Save the edit as a new WAV in Wave. The original source stays unchanged.").on_disabled_hover_text(reason.unwrap_or("Select a nonempty trim range.")).clicked() { action=Action::Save; }
                let summary = reason.map(str::to_owned).unwrap_or_else(|| {
                    let selected=*end-*start;
                    if source.sample_rate>0 { format!("{selected} source frames selected · {:.3} s after pitch",(selected as f64/2.0_f64.powf(*pitch/12.0)).round().max(1.0)/f64::from(source.sample_rate)) }
                    else { format!("{selected} source frames selected") }
                });
                panel.painter().rect_filled(rect(8.0,326.0,624.0,36.0),4.0*scale,FACE);
                panel.painter().rect_stroke(rect(8.0,326.0,624.0,36.0),4.0*scale,Stroke::new(1.0*scale,if reason.is_some(){CORAL}else{EDGE}),egui::StrokeKind::Inside);
                let width=if source.missing {484.0} else {592.0};
                let status=panel.put(rect(24.0,326.0,width,36.0),egui::Label::new(egui::RichText::new(&summary).size(11.0*scale).color(if reason.is_some(){CORAL}else{CREAM})).wrap());
                name_control(status.clone(),"Sample editor status");
                ctx.accesskit_node_builder(status.id, |node| {node.set_value(summary.as_str());node.set_live(egui::accesskit::Live::Polite);});
                status.on_hover_text(&summary);
                if source.missing && action_button(&mut panel,rect(516.0,328.0,100.0,28.0),"Import details",scale,false,true).clicked() { action=Action::Details; }
            }
            let collapse_name=if collapsed {"Expand Sample editor"} else {"Collapse Sample editor"};
            let collapse=header_button(&mut panel,rect(10.0,header_y-16.0,32.0,32.0),collapse_name,scale);
            let direction=if collapsed {-1.0} else {1.0};
            panel.painter().add(egui::Shape::convex_polygon(vec![at(26.0,header_y)+Vec2::new(-5.0,-3.0*direction)*scale,at(26.0,header_y)+Vec2::new(5.0,-3.0*direction)*scale,at(26.0,header_y)+Vec2::new(0.0,4.5*direction)*scale],AMBER,Stroke::NONE));
            if collapse.on_hover_text(collapse_name).clicked(){ctx.data_mut(|data|data.insert_temp(collapse_id,!collapsed));}
            let close=header_button(&mut panel,rect(598.0,header_y-16.0,32.0,32.0),"Close Sample editor",scale);
            for diagonal in [-1.0,1.0] { let offset=Vec2::new(6.0,6.0*diagonal)*scale; panel.painter().line_segment([at(614.0,header_y)-offset,at(614.0,header_y)+offset],Stroke::new(1.6*scale,CREAM)); }
            let layer=panel.layer_id();
            let on_top=ctx.memory(|memory|memory.top_modal_layer().is_none() && memory.layer_ids().filter(|other|other.order==egui::Order::Middle && memory.areas().is_visible(other)).last()==Some(layer));
            if close.on_hover_text("Close Sample editor (Esc)").clicked() || (on_top && !guarded && ctx.input(|input|input.key_pressed(egui::Key::Escape))) { *open=false; }
            let guard=editing || egui::Popup::is_any_open(ctx);
            ctx.data_mut(|data|data.insert_temp(guard_id,guard));
        });
    Ok(action)
}

fn text(ui: &egui::Ui, at: Pos2, value: &str, size: f32, color: Color32, heading: bool) {
    ui.painter().text(
        at,
        Align2::CENTER_CENTER,
        value,
        if heading {
            FontId::new(size, egui::FontFamily::Name("Syncopate Bold".into()))
        } else {
            FontId::proportional(size)
        },
        color,
    );
}
fn label(ui: &egui::Ui, at: Pos2, value: &str, size: f32) {
    ui.painter().text(
        at,
        Align2::LEFT_CENTER,
        value,
        FontId::proportional(size),
        CREAM,
    );
}
fn focus(ui: &egui::Ui, response: &Response, scale: f32) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(scale),
            3.0,
            Stroke::new(1.5 * scale, AMBER),
            egui::StrokeKind::Outside,
        );
    }
}
fn header_button(ui: &mut egui::Ui, rect: Rect, name: &str, scale: f32) -> Response {
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    ui.painter()
        .circle_filled(rect.center(), 14.0 * scale, FACE);
    ui.painter().circle_stroke(
        rect.center(),
        14.0 * scale,
        Stroke::new(1.0 * scale, if response.hovered() { CREAM } else { EDGE }),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), name)
    });
    focus(ui, &response, scale);
    response
}
fn action_button(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    scale: f32,
    primary: bool,
    enabled: bool,
) -> Response {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.interact(rect, ui.id().with(name), Sense::click())
        })
        .inner;
    let fill = if primary {
        if response.hovered() { AMBER } else { GOLD }
    } else if response.hovered() {
        BODY
    } else {
        FACE
    };
    ui.painter().rect_filled(rect, 5.0 * scale, fill);
    ui.painter().rect_stroke(
        rect,
        5.0 * scale,
        Stroke::new(1.0 * scale, if primary { AMBER } else { EDGE }),
        egui::StrokeKind::Inside,
    );
    text(
        ui,
        rect.center(),
        name,
        13.0 * scale,
        if primary { INK } else { CREAM },
        false,
    );
    if enabled && response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, 5.0 * scale, Color32::from_black_alpha(55));
    }
    if !enabled {
        ui.painter()
            .rect_filled(rect, 5.0 * scale, Color32::from_black_alpha(130));
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), name)
    });
    focus(ui, &response, scale);
    response
}
fn well(ui: &egui::Ui, rect: Rect, scale: f32) {
    ui.painter().rect_filled(rect, 4.0 * scale, FACE);
    ui.painter().rect_stroke(
        rect,
        4.0 * scale,
        Stroke::new(1.0 * scale, EDGE),
        egui::StrokeKind::Inside,
    );
}
fn value_style(ui: &mut egui::Ui, scale: f32) {
    ui.spacing_mut().interact_size = Vec2::new(40.0, 28.0) * scale;
    ui.spacing_mut().button_padding = Vec2::ZERO;
    ui.style_mut().drag_value_text_style = egui::TextStyle::Monospace;
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(16.0 * scale));
    ui.visuals_mut().override_text_color = Some(AMBER);
    let widgets = &mut ui.visuals_mut().widgets;
    for widget in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
    ] {
        widget.bg_fill = Color32::TRANSPARENT;
        widget.weak_bg_fill = Color32::TRANSPARENT;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke.color = AMBER;
    }
}
fn stepper(ui: &mut egui::Ui, rect: Rect, name: &str, up: bool, scale: f32) -> Response {
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    ui.painter().rect_filled(
        rect,
        2.0,
        if response.hovered() {
            BODY
        } else {
            Color32::from_rgb(38, 42, 43)
        },
    );
    let direction = if up { -1.0 } else { 1.0 };
    let center = rect.center();
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            center + Vec2::new(-4.0, -2.5 * direction) * scale,
            center + Vec2::new(4.0, -2.5 * direction) * scale,
            center + Vec2::new(0.0, 3.5 * direction) * scale,
        ],
        CREAM,
        Stroke::NONE,
    ));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), name)
    });
    focus(ui, &response, scale);
    response
}
fn frame_value(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    value: &mut u64,
    range: std::ops::RangeInclusive<u64>,
) -> bool {
    well(ui, rect, scale);
    let mut editing = false;
    ui.scope_builder(egui::UiBuilder::new().id(name).max_rect(Rect::from_min_max(rect.min+Vec2::new(6.0,8.0)*scale,rect.max-Vec2::new(28.0,8.0)*scale)),|ui|{
        value_style(ui,scale);let response=ui.add(egui::DragValue::new(value).range(range.clone()).clamp_existing_to_range(false).speed(100).fixed_decimals(0).update_while_editing(false));if response.changed(){*value=(*value).clamp(*range.start(),*range.end());}editing=response.has_focus();
        name_control(response,name).on_hover_text(if name=="Start frame"{"First included source frame (zero-based). Click for exact entry; Enter applies. Use the adjacent buttons for one-frame steps (Shift: 100)."} else {"First excluded source frame. Set this to the source frame count to include the last frame. Click for exact entry; Enter applies. Use the adjacent buttons for one-frame steps (Shift: 100)."});
    });
    for (up, y) in [(true, 2.0), (false, 24.0)] {
        let label = format!("{} {name}", if up { "Increase" } else { "Decrease" });
        let button = stepper(
            ui,
            Rect::from_min_size(
                Pos2::new(rect.right() - 22.0 * scale, rect.top() + y * scale),
                Vec2::new(18.0, 18.0) * scale,
            ),
            &label,
            up,
            scale,
        );
        if button
            .on_hover_text("Change one frame. Hold Shift to change 100 frames.")
            .clicked()
        {
            let amount = if ui.input(|input| input.modifiers.shift) {
                100
            } else {
                1
            };
            *value = if up {
                value.saturating_add(amount)
            } else {
                value.saturating_sub(amount)
            };
            *value = (*value).clamp(*range.start(), *range.end());
        }
    }
    editing
}
fn pitch_value(ui: &mut egui::Ui, rect: Rect, scale: f32, pitch: &mut f64) -> bool {
    well(ui, rect, scale);
    let mut editing = false;
    ui.scope_builder(egui::UiBuilder::new().id("pitch-value").max_rect(Rect::from_min_max(rect.min+Vec2::new(6.0,8.0)*scale,rect.max-Vec2::new(28.0,8.0)*scale)),|ui|{
        value_style(ui,scale);let id=ui.next_auto_id();let context=ui.ctx().clone();let response=ui.add(egui::DragValue::new(pitch).range(-24.0..=24.0).clamp_existing_to_range(false).speed(0.1).custom_formatter(move|value,_|if context.memory(|memory|memory.has_focus(id)){format!("{value}")}else{format!("{value:.1}")}).update_while_editing(false));if response.changed(){*pitch=pitch.clamp(-24.0,24.0);}editing=response.has_focus();name_control(response,"Pitch value").on_hover_text("Enter semitones from -24 to +24. Pitch changes playback speed and output duration together. Zero keeps the source pitch.");
    });
    for (up, y) in [(true, 2.0), (false, 24.0)] {
        let name = if up {
            "Increase Pitch"
        } else {
            "Decrease Pitch"
        };
        let button = stepper(
            ui,
            Rect::from_min_size(
                Pos2::new(rect.right() - 22.0 * scale, rect.top() + y * scale),
                Vec2::new(18.0, 18.0) * scale,
            ),
            name,
            up,
            scale,
        );
        if button
            .on_hover_text("Change 0.1 semitone. Hold Shift for one semitone.")
            .clicked()
        {
            let amount = if ui.input(|input| input.modifiers.shift) {
                1.0
            } else {
                0.1
            };
            *pitch += if up { amount } else { -amount };
            *pitch = pitch.clamp(-24.0, 24.0);
        }
    }
    editing
}
fn pitch_rail(ui: &mut egui::Ui, art: &Artwork, rect: Rect, scale: f32, pitch: &mut f64) {
    ui.scope_builder(egui::UiBuilder::new().id("pitch-rail").max_rect(rect),|ui|{
        ui.spacing_mut().slider_width=rect.width();ui.spacing_mut().interact_size.y=rect.height();
        let widgets=&mut ui.visuals_mut().widgets;for widget in [&mut widgets.inactive,&mut widgets.hovered,&mut widgets.active]{widget.bg_fill=Color32::TRANSPARENT;widget.bg_stroke=Stroke::NONE;widget.fg_stroke=Stroke::NONE;}
        let response=ui.add(egui::Slider::new(pitch,-24.0..=24.0).show_value(false).step_by(0.1).clamping(egui::SliderClamping::Edits));
        let travel=response.rect.x_range().shrink(response.rect.height()/2.5);
        let rail=Rect::from_center_size(response.rect.center(),Vec2::new(travel.span(),8.0*scale));ui.painter().rect_filled(rail,4.0,FACE);ui.painter().rect_stroke(rail,4.0,Stroke::new(1.0*scale,EDGE),egui::StrokeKind::Inside);
        art.paint(ui,2,Pos2::new(egui::lerp(travel.min..=travel.max,((*pitch+24.0)/48.0)as f32),response.rect.center().y));
        response.widget_info(||egui::WidgetInfo::slider(response.enabled(),*pitch,"Pitch (semitones)"));focus(ui,&response,scale);response.on_hover_text("Pitch changes playback speed and sample length. +12 is one octave up and half the duration; -12 is one octave down and twice the duration. Use the numeric value for exact entry.");
    });
}

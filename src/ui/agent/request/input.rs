//! Agent requests that become input steps: a screenshot, a click, a value, a
//! key, text, a pointer gesture, a scroll, a wait, and Quit.
use super::*;

impl MixJamApp {
    /// Turn an input request into the steps a person at the window takes.
    pub(super) fn agent_input(
        &mut self,
        ctx: &egui::Context,
        request: Request,
    ) -> Result<Start, String> {
        Ok(match request {
            Request::Screenshot { path } => {
                if !path.is_absolute() {
                    return Err("Give the full path of the PNG file.".into());
                }
                // A minimized window has no framebuffer; the GL backend
                // panics when it reads one.
                if ctx.input(|input| input.viewport().minimized == Some(true)) {
                    return Err("The window is minimized. Restore it before a screenshot.".into());
                }
                Start::run(vec![Step::Shoot(path)])
            }
            Request::Click {
                target,
                window,
                role,
                index,
                double,
                right,
                pointer,
                modifiers: held,
            } => {
                let node = self.agent_target(ctx, &target, window, role, index)?;
                let steps = if double || right || pointer || !held.is_empty() {
                    let [x, y, width, height] = node.rect;
                    let pos = visible(ctx, Pos2::new(x + width / 2.0, y + height / 2.0))?;
                    click_steps(pos, right, double, modifiers(&held))
                } else if node.clickable {
                    vec![action(&node, accesskit::Action::Click, None)]
                } else {
                    return Err(format!(
                        "\"{}\" does not take a click. Add pointer=true to click at its position.",
                        node.label
                    ));
                };
                Start::Run {
                    steps: settled(steps),
                    clips: Vec::new(),
                    control: Some(node.id),
                }
            }
            Request::Set {
                target,
                window,
                role,
                index,
                value,
            } => {
                let node = self.agent_target(ctx, &target, window, role, index)?;
                let steps = self.set_steps(ctx, &node, &value)?;
                Start::Run {
                    steps: settled(steps),
                    clips: Vec::new(),
                    control: Some(node.id),
                }
            }
            Request::Key {
                key,
                modifiers: held,
            } => {
                let key = egui::Key::from_name(&key).ok_or_else(|| {
                    format!(
                        "\"{key}\" is not a key. Use a name such as Enter, Escape, Space, Delete, \
                         Tab, ArrowLeft, A, or F5."
                    )
                })?;
                Start::run(settled(key_steps(key, modifiers(&held)).into()))
            }
            Request::Type { text } => Start::run(settled(vec![Step::Input(
                vec![egui::Event::Text(text)],
                egui::Modifiers::NONE,
            )])),
            Request::Pointer {
                action,
                x,
                y,
                to_x,
                to_y,
                right,
                modifiers: held,
            } => {
                let held = modifiers(&held);
                let pos = visible(ctx, Pos2::new(x, y))?;
                let steps = pointer_steps(ctx, action, pos, (to_x, to_y), right, held)?;
                Start::run(settled(steps))
            }
            Request::Scroll { x, y, dx, dy } => {
                let pos = visible(ctx, Pos2::new(x, y))?;
                Start::run(settled(vec![
                    Step::Input(vec![egui::Event::PointerMoved(pos)], egui::Modifiers::NONE),
                    Step::Input(
                        vec![egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(dx, dy),
                            modifiers: egui::Modifiers::NONE,
                        }],
                        egui::Modifiers::NONE,
                    ),
                ]))
            }
            Request::Wait {
                ms,
                idle,
                timeout_ms,
            } => {
                let wait = if idle {
                    timeout_ms.unwrap_or(120_000)
                } else {
                    ms.unwrap_or(0)
                };
                Start::run(vec![Step::Until {
                    at: Instant::now() + Duration::from_millis(wait),
                    idle,
                }])
            }
            Request::Quit => {
                self.request_action(PendingAction::Close, ctx);
                // Unsaved work opens a modal instead. The loop replies at
                // once when the window closes, and after the modal shows.
                Start::run(settled(Vec::new()))
            }
            _ => return Err("That is not an input request.".into()),
        })
    }

    /// The steps that give `node` the value of a `set` request.
    fn set_steps(
        &self,
        ctx: &egui::Context,
        node: &UiNode,
        value: &Value,
    ) -> Result<Vec<Step>, String> {
        Ok(match node.role {
            "slider" | "number" => {
                let number = match value {
                    Value::Number(number) => Some(*number),
                    Value::Text(text) => text.parse().ok(),
                    Value::Bool(_) => None,
                }
                .filter(|number: &f64| number.is_finite())
                .ok_or("This control needs a number.")?;
                vec![action(
                    node,
                    accesskit::Action::SetValue,
                    Some(accesskit::ActionData::NumericValue(number)),
                )]
            }
            "text_input" => {
                let text = value.text();
                let mut steps = vec![action(node, accesskit::Action::Focus, None)];
                steps.extend(key_steps(egui::Key::A, modifiers(&[Modifier::Ctrl])));
                if text.is_empty() {
                    steps.extend(key_steps(egui::Key::Backspace, egui::Modifiers::NONE));
                } else {
                    steps.push(Step::Input(
                        vec![egui::Event::Text(text)],
                        egui::Modifiers::NONE,
                    ));
                }
                steps
            }
            "checkbox" | "radio" | "button" if node.checked.is_some() => {
                let checked = truth(value)?;
                if node.checked == Some(checked) {
                    Vec::new()
                } else if node.role == "radio" && !checked {
                    return Err("Choose another radio button instead.".into());
                } else {
                    vec![action(node, accesskit::Action::Click, None)]
                }
            }
            "combo_box" if node.text.as_deref() == Some(value.text().as_str()) => Vec::new(),
            "combo_box" => vec![
                action(node, accesskit::Action::Click, None),
                Step::Idle,
                Step::Idle,
                Step::Pick {
                    value: value.text(),
                    known: self
                        .agent_nodes(ctx)
                        .into_iter()
                        .map(|node| node.id)
                        .collect(),
                },
                Step::Idle,
                Step::ClosePopups,
            ],
            _ => {
                return Err(format!(
                    "\"{}\" is a {} and has no value to set. Use click.",
                    node.label, node.role
                ));
            }
        })
    }
}

/// `pos` when it is inside the window.
fn visible(ctx: &egui::Context, pos: Pos2) -> Result<Pos2, String> {
    if ctx.viewport_rect().contains(pos) {
        Ok(pos)
    } else {
        Err(format!(
            "The position ({:.0}, {:.0}) is outside the window.",
            pos.x, pos.y
        ))
    }
}

/// The steps of a `pointer` request at `pos`, which is inside the window.
fn pointer_steps(
    ctx: &egui::Context,
    action: PointerAction,
    pos: Pos2,
    (to_x, to_y): (Option<f32>, Option<f32>),
    right: bool,
    held: egui::Modifiers,
) -> Result<Vec<Step>, String> {
    Ok(match action {
        PointerAction::Move => {
            vec![Step::Input(vec![egui::Event::PointerMoved(pos)], held)]
        }
        PointerAction::Down => vec![press(pos, right, held)],
        PointerAction::Up => vec![
            Step::Input(vec![egui::Event::PointerMoved(pos)], held),
            release(pos, right, held),
        ],
        PointerAction::Click => click_steps(pos, right, false, held),
        PointerAction::DoubleClick => click_steps(pos, right, true, held),
        PointerAction::Drag => {
            let (Some(to_x), Some(to_y)) = (to_x, to_y) else {
                return Err("A drag needs to_x and to_y.".into());
            };
            let end = visible(ctx, Pos2::new(to_x, to_y))?;
            // The first short move starts the drag. The others
            // let the target under the pointer follow it.
            let moves = [
                pos + (end - pos).normalized() * 8.0,
                pos + (end - pos) / 2.0,
                end,
                end,
            ];
            // Establish the hovered edge/window before pressing.
            // Native hit testing needs the same approach frame
            // that a physical pointer supplies before a drag.
            [
                Step::Input(vec![egui::Event::PointerMoved(pos)], held),
                Step::Idle,
                press(pos, right, held),
            ]
            .into_iter()
            .chain(moves.map(|pos| Step::Input(vec![egui::Event::PointerMoved(pos)], held)))
            .chain([release(end, right, held)])
            .collect()
        }
    })
}

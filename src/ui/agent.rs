//! Agent driver. It turns requests from the control channel into the input
//! and commands that a person at the window produces, and reports what the
//! window then shows. Song changes still go through the session.
use super::*;
use eframe::egui::accesskit;
use mixjam_techno::application::agent::{
    self as wire, ArrangementView, Body, Modifier, Outcome, PathPromptView, PointerAction, Request,
    Responder, Selection, Server, StateView, Status, Tools, UiNode, Value,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

/// The controls of the last finished frame, shared with the output tap.
type Tree = Arc<Mutex<Vec<UiNode>>>;

pub(super) struct Driver {
    server: Server,
    tree: Tree,
    tapped: bool,
    job: Option<Job>,
}

/// A request that needs frames to run. It replies when its steps are done.
struct Job {
    respond: Responder,
    steps: VecDeque<Step>,
    clips: Vec<u64>,
    /// The id of the control to report.
    control: Option<String>,
}

enum Step {
    /// Events for one frame, with the modifier keys held during it.
    Input(Vec<egui::Event>, egui::Modifiers),
    /// A frame without agent input, so that the window shows the result.
    Idle,
    /// Click the item that an opened combo box added to the controls.
    Pick {
        value: String,
        known: BTreeSet<String>,
    },
    ClosePopups,
    Shoot(PathBuf),
    Shot {
        path: PathBuf,
        deadline: Instant,
    },
    Until {
        at: Instant,
        idle: bool,
    },
}

enum Start {
    Reply(Body),
    Run {
        steps: Vec<Step>,
        clips: Vec<u64>,
        control: Option<String>,
    },
}

impl Start {
    fn run(steps: Vec<Step>) -> Self {
        Self::Run {
            steps,
            clips: Vec::new(),
            control: None,
        }
    }
}

/// Copies each frame's accessibility tree for the driver.
struct TreeTap(Tree);

impl egui::Plugin for TreeTap {
    fn debug_name(&self) -> &'static str {
        "mixjam-agent-tree"
    }

    fn output_hook(&mut self, output: &mut egui::FullOutput) {
        if let Some(update) = &output.platform_output.accesskit_update
            && let Ok(mut tree) = self.0.lock()
        {
            *tree = nodes(update);
        }
    }
}

impl Driver {
    pub(super) fn new(server: Server) -> Self {
        Self {
            server,
            tree: Tree::default(),
            tapped: false,
            job: None,
        }
    }

    fn nodes(&self) -> Vec<UiNode> {
        self.tree
            .lock()
            .map(|tree| tree.clone())
            .unwrap_or_default()
    }
}

fn role_name(role: accesskit::Role) -> &'static str {
    use accesskit::Role;
    match role {
        Role::Button => "button",
        Role::CheckBox => "checkbox",
        Role::RadioButton => "radio",
        Role::ComboBox => "combo_box",
        Role::Slider => "slider",
        Role::SpinButton => "number",
        Role::TextInput => "text_input",
        Role::Link => "link",
        Role::Label => "label",
        Role::Image => "image",
        Role::Window => "window",
        Role::ProgressIndicator => "progress",
        _ => "other",
    }
}

fn nodes(update: &accesskit::TreeUpdate) -> Vec<UiNode> {
    let root = update.tree.as_ref().map(|tree| tree.root);
    let mut parents = HashMap::new();
    let mut windows = HashMap::new();
    for (id, node) in &update.nodes {
        for child in node.children() {
            parents.insert(*child, *id);
        }
        if matches!(
            node.role(),
            accesskit::Role::Window | accesskit::Role::Dialog
        ) && Some(*id) != root
            && let Some(title) = node.label()
        {
            windows.insert(*id, title.to_string());
        }
    }
    let window_of = |id: &accesskit::NodeId| {
        let mut id = id;
        // The depth limit guards against a malformed tree.
        for _ in 0..64 {
            id = parents.get(id)?;
            if let Some(title) = windows.get(id) {
                return Some(title.clone());
            }
        }
        None
    };
    let mut nodes: Vec<UiNode> = update
        .nodes
        .iter()
        .filter(|(id, _)| Some(*id) != root)
        .filter_map(|(id, node)| {
            let bounds = node.bounds()?;
            // egui stores the text of a label as its value.
            let (label, text) = if node.role() == accesskit::Role::Label {
                (node.value(), None)
            } else {
                (node.label(), node.value())
            };
            Some(UiNode {
                id: format!("#{}", id.0),
                role: role_name(node.role()),
                label: label.unwrap_or_default().to_string(),
                text: text.map(str::to_string),
                number: node.numeric_value(),
                checked: node
                    .toggled()
                    .map(|toggled| toggled == accesskit::Toggled::True),
                enabled: !node.is_disabled(),
                clickable: node.supports_action(accesskit::Action::Click),
                window: window_of(id),
                rect: [
                    bounds.x0 as f32,
                    bounds.y0 as f32,
                    (bounds.x1 - bounds.x0) as f32,
                    (bounds.y1 - bounds.y0) as f32,
                ],
            })
        })
        .collect();
    // egui adds a second node for the text of a label, and nodes without
    // content for window edges. Neither is a control.
    let labels: BTreeSet<String> = nodes
        .iter()
        .filter(|node| node.role == "label")
        .map(|node| node.label.clone())
        .collect();
    nodes.retain(|node| {
        node.role != "other"
            || node.clickable
            || !node.label.is_empty()
            || node
                .text
                .as_ref()
                .is_some_and(|text| !labels.contains(text))
    });
    nodes.sort_by(|a, b| {
        a.window
            .cmp(&b.window)
            .then(a.rect[1].total_cmp(&b.rect[1]))
            .then(a.rect[0].total_cmp(&b.rect[0]))
    });
    nodes
}

fn in_window(node: &UiNode, window: Option<&str>) -> bool {
    window.is_none_or(|window| {
        node.window
            .as_deref()
            .is_some_and(|title| title.eq_ignore_ascii_case(window))
    })
}

/// The control that a request names by id or by label.
fn find(
    nodes: &[UiNode],
    target: &str,
    window: Option<&str>,
    role: Option<&str>,
    index: Option<usize>,
) -> Result<UiNode, String> {
    if target.starts_with('#') {
        return nodes
            .iter()
            .find(|node| node.id == target)
            .cloned()
            .ok_or_else(|| format!("No control has the id {target}. Read `ui` again."));
    }
    let scoped: Vec<&UiNode> = nodes
        .iter()
        .filter(|node| {
            node.role != "window"
                && in_window(node, window)
                && role.is_none_or(|role| node.role == role)
        })
        .collect();
    let lower = target.to_lowercase();
    let tests: [&dyn Fn(&str) -> bool; 3] = [
        &|label| label == target,
        &|label| label.to_lowercase() == lower,
        &|label| label.to_lowercase().contains(&lower),
    ];
    let mut found = Vec::new();
    for test in tests {
        found = scoped
            .iter()
            .copied()
            .filter(|node| test(&node.label))
            .collect();
        if !found.is_empty() {
            break;
        }
    }
    if found.is_empty() {
        return Err(format!(
            "No control is named \"{target}\". Use `ui` to list the controls on screen."
        ));
    }
    // Prefer what a person can use now: an enabled control over its text.
    if found.iter().any(|node| node.enabled) {
        found.retain(|node| node.enabled);
    }
    if found.iter().any(|node| node.role != "label") {
        found.retain(|node| node.role != "label");
    }
    match index {
        Some(index) => found.get(index).copied().cloned().ok_or_else(|| {
            format!(
                "Only {} controls are named \"{target}\"; index counts from 0.",
                found.len()
            )
        }),
        None if found.len() == 1 => Ok(found[0].clone()),
        None => Err(format!(
            "{} controls are named \"{target}\": {}. Add window=, role=, or index=.",
            found.len(),
            found
                .iter()
                .enumerate()
                .map(|(index, node)| format!(
                    "index {index} is {} {}{} at ({:.0}, {:.0})",
                    node.role,
                    node.id,
                    node.window
                        .as_ref()
                        .map(|window| format!(" in {window}"))
                        .unwrap_or_default(),
                    node.rect[0],
                    node.rect[1]
                ))
                .collect::<Vec<_>>()
                .join("; ")
        )),
    }
}

fn modifiers(held: &[Modifier]) -> egui::Modifiers {
    let mut modifiers = egui::Modifiers::NONE;
    for modifier in held {
        match modifier {
            Modifier::Ctrl => {
                modifiers.ctrl = true;
                modifiers.command = true;
            }
            Modifier::Shift => modifiers.shift = true,
            Modifier::Alt => modifiers.alt = true,
        }
    }
    modifiers
}

fn action(node: &UiNode, action: accesskit::Action, data: Option<accesskit::ActionData>) -> Step {
    // `find` returns only ids that `nodes` wrote in this form.
    let target = accesskit::NodeId(node.id[1..].parse().unwrap_or_default());
    Step::Input(
        vec![egui::Event::AccessKitActionRequest(
            accesskit::ActionRequest {
                action,
                target,
                data,
            },
        )],
        egui::Modifiers::NONE,
    )
}

fn key_steps(key: egui::Key, modifiers: egui::Modifiers) -> [Step; 2] {
    [true, false].map(|pressed| {
        Step::Input(
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            }],
            modifiers,
        )
    })
}

fn button(pos: Pos2, right: bool, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: if right {
            egui::PointerButton::Secondary
        } else {
            egui::PointerButton::Primary
        },
        pressed,
        modifiers,
    }
}

fn press(pos: Pos2, right: bool, held: egui::Modifiers) -> Step {
    Step::Input(
        vec![
            egui::Event::PointerMoved(pos),
            button(pos, right, true, held),
        ],
        held,
    )
}

fn release(pos: Pos2, right: bool, held: egui::Modifiers) -> Step {
    Step::Input(vec![button(pos, right, false, held)], held)
}

fn click_steps(pos: Pos2, right: bool, double: bool, held: egui::Modifiers) -> Vec<Step> {
    (0..if double { 2 } else { 1 })
        .flat_map(|_| [press(pos, right, held), release(pos, right, held)])
        .collect()
}

fn settled(mut steps: Vec<Step>) -> Vec<Step> {
    steps.extend([Step::Idle, Step::Idle]);
    steps
}

fn truth(value: &Value) -> Result<bool, String> {
    match value {
        Value::Bool(value) => Ok(*value),
        Value::Text(text) if text.eq_ignore_ascii_case("true") => Ok(true),
        Value::Text(text) if text.eq_ignore_ascii_case("false") => Ok(false),
        _ => Err("This control needs value=true or value=false.".into()),
    }
}

fn rect_view(rect: Rect) -> [f32; 4] {
    [rect.left(), rect.top(), rect.width(), rect.height()]
}

impl MixJamApp {
    /// Let agents drive this window through `server`.
    pub fn attach_agent(&mut self, server: Server, ctx: &egui::Context) {
        let ctx = ctx.clone();
        server.set_waker(move || ctx.request_repaint());
        self.agent = Some(Driver::new(server));
    }

    /// Run before each frame: take requests, and add the input of the request
    /// in progress to the frame.
    pub(super) fn agent_frame(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        let Some(driver) = self.agent.as_mut() else {
            return;
        };
        if !driver.tapped {
            ctx.add_plugin(TreeTap(driver.tree.clone()));
            driver.tapped = true;
        }
        // The platform adapter turns the tree off when no screen reader runs.
        ctx.enable_accesskit();
        let mut job = driver.job.take();
        loop {
            let Some(current) = job.as_mut() else {
                let Some((request, respond)) =
                    self.agent.as_ref().and_then(|driver| driver.server.next())
                else {
                    break;
                };
                match self.agent_start(ctx, request) {
                    Ok(Start::Reply(body)) => respond.send(Ok(body)),
                    Ok(Start::Run {
                        steps,
                        clips,
                        control,
                    }) => {
                        job = Some(Job {
                            respond,
                            steps: steps.into(),
                            clips,
                            control,
                        });
                    }
                    Err(error) => respond.send(Err(error)),
                }
                continue;
            };
            // The window closes after this frame. Reply while one still runs.
            if self.allow_close {
                current.steps.clear();
            }
            let Some(step) = current.steps.pop_front() else {
                if let Some(job) = job.take() {
                    let outcome = self.agent_outcome(job.clips, job.control);
                    job.respond.send(Ok(Body::Done(Box::new(outcome))));
                }
                continue;
            };
            // Text fields ignore keys while the window is in the background.
            input.focused = true;
            let failure = match step {
                Step::Input(events, held) => {
                    input.events.extend(events);
                    if held != egui::Modifiers::NONE {
                        input.modifiers = held;
                    }
                    ctx.request_repaint();
                    break;
                }
                Step::Idle => {
                    ctx.request_repaint();
                    break;
                }
                Step::Pick { value, known } => {
                    let options: Vec<UiNode> = self
                        .agent_nodes()
                        .into_iter()
                        .filter(|node| {
                            node.clickable && !node.label.is_empty() && !known.contains(&node.id)
                        })
                        .collect();
                    // "160" also selects a choice shown as "160 BPM", when
                    // only one choice has that word.
                    let worded = || {
                        let mut found = options.iter().filter(|node| {
                            node.label
                                .split_whitespace()
                                .any(|word| word.eq_ignore_ascii_case(&value))
                        });
                        found.next().filter(|_| found.next().is_none())
                    };
                    match options
                        .iter()
                        .find(|node| node.label == value)
                        .or_else(|| {
                            options
                                .iter()
                                .find(|node| node.label.eq_ignore_ascii_case(&value))
                        })
                        .or_else(worded)
                    {
                        Some(option) => {
                            current.steps.push_front(action(
                                option,
                                accesskit::Action::Click,
                                None,
                            ));
                            continue;
                        }
                        None => {
                            egui::Popup::close_all(ctx);
                            ctx.request_repaint();
                            format!(
                                "This control has no choice \"{value}\". Its choices are: {}.",
                                options
                                    .iter()
                                    .map(|node| node.label.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                    }
                }
                Step::ClosePopups => {
                    egui::Popup::close_all(ctx);
                    continue;
                }
                Step::Shoot(path) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                    current.steps.push_front(Step::Shot {
                        path,
                        deadline: Instant::now() + Duration::from_secs(10),
                    });
                    ctx.request_repaint();
                    break;
                }
                Step::Shot { path, deadline } => {
                    let image = input.events.iter().find_map(|event| match event {
                        egui::Event::Screenshot { image, .. } => Some(image.clone()),
                        _ => None,
                    });
                    let result = match image {
                        Some(image) => {
                            capture::write_png(&path, &image).map(|()| Body::Screenshot {
                                width: image.width(),
                                height: image.height(),
                                pixels_per_point: ctx.pixels_per_point(),
                                path,
                            })
                        }
                        None if Instant::now() > deadline => {
                            Err("The window did not return a screenshot.".into())
                        }
                        None => {
                            current.steps.push_front(Step::Shot { path, deadline });
                            ctx.request_repaint();
                            break;
                        }
                    };
                    if let Some(job) = job.take() {
                        job.respond.send(result);
                    }
                    continue;
                }
                Step::Until { at, idle } => {
                    let busy = self.session.wave_scan().is_some() || self.session.is_exporting();
                    let now = Instant::now();
                    if idle && !busy || !idle && now >= at {
                        ctx.request_repaint();
                        continue;
                    }
                    if idle && now >= at {
                        "The scan or export did not finish in time. Send wait again.".to_string()
                    } else {
                        current.steps.push_front(Step::Until { at, idle });
                        ctx.request_repaint_after(if idle {
                            Duration::from_millis(50)
                        } else {
                            at - now
                        });
                        break;
                    }
                }
            };
            if let Some(job) = job.take() {
                job.respond.send(Err(failure));
            }
        }
        if let Some(driver) = self.agent.as_mut() {
            driver.job = job;
        }
    }

    /// The controls on screen. A modal takes all input, but the toolkit
    /// still reports the controls behind it as enabled.
    fn agent_nodes(&self) -> Vec<UiNode> {
        let mut nodes = self.agent.as_ref().map(Driver::nodes).unwrap_or_default();
        if let Some(modal) = self.modal_name() {
            for node in &mut nodes {
                node.enabled &= node.window.as_deref() == Some(modal);
            }
        }
        nodes
    }

    fn agent_start(&mut self, ctx: &egui::Context, request: Request) -> Result<Start, String> {
        let visible = |pos: Pos2| {
            if ctx.viewport_rect().contains(pos) {
                Ok(pos)
            } else {
                Err(format!(
                    "The position ({:.0}, {:.0}) is outside the window.",
                    pos.x, pos.y
                ))
            }
        };
        Ok(match request {
            Request::Help => Start::Reply(Body::Help { help: wire::HELP }),
            Request::State => Start::Reply(Body::State(Box::new(self.agent_state()))),
            Request::Samples {
                search,
                group,
                library,
                offset,
                limit,
            } => {
                let library = library
                    .map(|library| Product::parse(&library))
                    .transpose()?
                    .map(Product::label);
                let search = search.to_lowercase();
                let found: Vec<_> = self
                    .session
                    .library()
                    .samples
                    .iter()
                    .filter(|sample| {
                        sample.name.to_lowercase().contains(&search)
                            && group
                                .as_deref()
                                .is_none_or(|group| sample.group_name.eq_ignore_ascii_case(group))
                    })
                    .map(|sample| wire::sample_view(&self.session, sample))
                    .filter(|sample| library.is_none() || sample.library == library)
                    .collect();
                Start::Reply(Body::Samples {
                    total: found.len(),
                    samples: found
                        .into_iter()
                        .skip(offset)
                        .take(limit.unwrap_or(50))
                        .collect(),
                })
            }
            Request::Ui {
                all,
                window,
                contains,
            } => {
                let contains = contains.map(|text| text.to_lowercase());
                Start::Reply(Body::Ui {
                    modal: self.modal_name(),
                    nodes: self
                        .agent_nodes()
                        .into_iter()
                        .filter(|node| {
                            // Without `all`: what a person can operate now.
                            let usable = node.enabled
                                && !matches!(node.role, "label" | "image")
                                && (!node.label.is_empty() || node.text.is_some())
                                && (node.clickable
                                    || matches!(
                                        node.role,
                                        "slider" | "number" | "text_input" | "combo_box" | "window"
                                    ));
                            (all || usable)
                                && match window.as_deref() {
                                    Some(title) if node.role == "window" => {
                                        node.label.eq_ignore_ascii_case(title)
                                    }
                                    window => in_window(node, window),
                                }
                                && contains.as_ref().is_none_or(|text| {
                                    node.label.to_lowercase().contains(text)
                                        || node.text.as_ref().is_some_and(|value| {
                                            value.to_lowercase().contains(text)
                                        })
                                })
                        })
                        .collect(),
                })
            }
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
                let node = self.agent_target(&target, window, role, index)?;
                let steps = if double || right || pointer || !held.is_empty() {
                    let [x, y, width, height] = node.rect;
                    let pos = visible(Pos2::new(x + width / 2.0, y + height / 2.0))?;
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
                let node = self.agent_target(&target, window, role, index)?;
                let steps = match node.role {
                    "slider" | "number" => {
                        let number = match &value {
                            Value::Number(number) => Some(*number),
                            Value::Text(text) => text.parse().ok(),
                            Value::Bool(_) => None,
                        }
                        .filter(|number: &f64| number.is_finite())
                        .ok_or("This control needs a number.")?;
                        vec![action(
                            &node,
                            accesskit::Action::SetValue,
                            Some(accesskit::ActionData::NumericValue(number)),
                        )]
                    }
                    "text_input" => {
                        let text = value.text();
                        let mut steps = vec![action(&node, accesskit::Action::Focus, None)];
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
                        if node.checked == Some(truth(&value)?) {
                            Vec::new()
                        } else if node.role == "radio" {
                            return Err("Choose another radio button instead.".into());
                        } else {
                            vec![action(&node, accesskit::Action::Click, None)]
                        }
                    }
                    "combo_box" if node.text.as_deref() == Some(value.text().as_str()) => {
                        Vec::new()
                    }
                    "combo_box" => vec![
                        action(&node, accesskit::Action::Click, None),
                        Step::Idle,
                        Step::Idle,
                        Step::Pick {
                            value: value.text(),
                            known: self.agent_nodes().into_iter().map(|node| node.id).collect(),
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
                };
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
                        "\"{key}\" is not a key. Use a name such as Enter, Escape, Space, Delete, Tab, ArrowLeft, A, or F5."
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
                let pos = visible(Pos2::new(x, y))?;
                let steps =
                    match action {
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
                            let end = visible(Pos2::new(to_x, to_y))?;
                            // The first short move starts the drag. The others
                            // let the target under the pointer follow it.
                            let moves = [
                                pos + (end - pos).normalized() * 8.0,
                                pos + (end - pos) / 2.0,
                                end,
                                end,
                            ];
                            std::iter::once(press(pos, right, held))
                                .chain(moves.map(|pos| {
                                    Step::Input(vec![egui::Event::PointerMoved(pos)], held)
                                }))
                                .chain([release(end, right, held)])
                                .collect()
                        }
                    };
                Start::run(settled(steps))
            }
            Request::Scroll { x, y, dx, dy } => {
                let pos = visible(Pos2::new(x, y))?;
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
            Request::Place {
                sample,
                track,
                bar,
                tick,
            } => {
                let sample = self.agent_sample(&sample)?;
                if !(1..=TRACK_COUNT).contains(&track) {
                    return Err(format!("Tracks are 1 to {TRACK_COUNT}."));
                }
                let id = self.try_place(sample, track - 1, wire::position_tick(bar, tick)?)?;
                Start::Run {
                    steps: vec![Step::Idle],
                    clips: vec![id],
                    control: None,
                }
            }
            Request::MoveClips {
                clips,
                tracks,
                bars,
                ticks,
                copy,
            } => {
                let bars = (bars * f64::from(TICKS_PER_BAR)).round();
                if !bars.is_finite() || bars.abs() > f64::from(u32::MAX) {
                    return Err("The bar distance is out of range.".into());
                }
                let clips = self.try_move_clips(&clips, tracks, ticks + bars as i64, copy)?;
                Start::Run {
                    steps: vec![Step::Idle],
                    clips,
                    control: None,
                }
            }
            Request::DeleteClips { clips } => {
                self.session.edit(EditCommand::DeleteClips(clips))?;
                self.reconcile_selection();
                self.sync_notice();
                Start::run(vec![Step::Idle])
            }
            Request::Select { clips, sample } => {
                let sample = sample
                    .map(|sample| self.agent_sample(&sample))
                    .transpose()?;
                let project = self.session.project();
                if let Some(id) = clips
                    .iter()
                    .find(|id| !project.clips.iter().any(|clip| clip.id == **id))
                {
                    return Err(format!("Clip {id} does not exist."));
                }
                let first = clips
                    .first()
                    .and_then(|id| project.clips.iter().find(|clip| clip.id == *id));
                self.selected_sample = sample.or(first.map(|clip| clip.sample_id));
                self.selected_clip = clips.first().copied();
                self.selected_clips = clips.into_iter().collect();
                Start::run(vec![Step::Idle])
            }
            Request::Seek { bar, tick } => {
                self.seek(wire::position_tick(bar, tick)?);
                Start::run(vec![Step::Idle])
            }
            Request::SetLoop {
                first_bar,
                last_bar,
            } => {
                let region = match (first_bar, last_bar) {
                    (None, None) => None,
                    (Some(first), Some(last)) if (1..=last).contains(&first) => Some([
                        (first - 1).saturating_mul(TICKS_PER_BAR),
                        last.saturating_mul(TICKS_PER_BAR),
                    ]),
                    _ => {
                        return Err(
                            "Give first_bar and last_bar from 1, or neither to turn the loop off."
                                .into(),
                        );
                    }
                };
                self.session.edit(EditCommand::SetLoop(region))?;
                self.sync_notice();
                Start::run(vec![Step::Idle])
            }
            Request::Quit => {
                self.request_action(PendingAction::Close, ctx);
                // Unsaved work opens a modal instead. The loop replies at
                // once when the window closes, and after the modal shows.
                Start::run(settled(Vec::new()))
            }
        })
    }

    fn agent_target(
        &self,
        target: &str,
        window: Option<String>,
        role: Option<String>,
        index: Option<usize>,
    ) -> Result<UiNode, String> {
        let node = find(
            &self.agent_nodes(),
            target,
            window.as_deref(),
            role.as_deref(),
            index,
        )?;
        if node.enabled {
            Ok(node)
        } else {
            Err(match self.modal_name() {
                Some(modal) if node.window.as_deref() != Some(modal) => format!(
                    "\"{}\" is disabled while the {modal} modal is open.",
                    node.label
                ),
                _ => format!("\"{}\" is disabled.", node.label),
            })
        }
    }

    /// A sound by id, or by a name that only one sound has.
    fn agent_sample(&self, sample: &Value) -> Result<u32, String> {
        let library = self.session.library();
        match sample {
            Value::Number(id) if id.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(id) => {
                let id = *id as u32;
                if library.sample_by_id(id).is_some() || self.session.missing_sample(id).is_some() {
                    Ok(id)
                } else {
                    Err(format!("No sound has the id {id}."))
                }
            }
            Value::Text(name) => {
                let named: Vec<&Sample> = library
                    .samples
                    .iter()
                    .filter(|sample| sample.name.eq_ignore_ascii_case(name))
                    .collect();
                match named[..] {
                    [sample] => Ok(sample.id),
                    [] => Err(format!(
                        "No sound is named \"{name}\". Use `samples search=` to find one."
                    )),
                    _ => Err(format!(
                        "{} sounds are named \"{name}\". Use one of these ids: {}.",
                        named.len(),
                        named
                            .iter()
                            .map(|sample| format!("{} ({})", sample.id, sample.group_name))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                }
            }
            _ => Err("Give the sound as an id or a name.".into()),
        }
    }

    /// The modal that takes all input, if one is open.
    fn modal_name(&self) -> Option<&'static str> {
        if self.path_prompt.is_some() {
            Some("Choose path")
        } else if self.pending_action.is_some() {
            Some("Save your changes?")
        } else if self.sample_conflicts_open() {
            Some("Sample conflicts")
        } else if self.import_path.is_some() {
            Some("Import sound")
        } else {
            None
        }
    }

    fn open_windows(&self) -> Vec<&'static str> {
        [
            (self.show_help, "Help"),
            (self.show_libraries, "Sample libraries"),
            (self.show_import_details, "Import details"),
            (self.show_effects, "Effect Studio"),
            (self.show_step_synth, "Step synth"),
            (self.show_sample_edit, "Sample editor"),
            (self.show_stretch, "Time Stretcher"),
            (
                self.session.has_recorded_take() && !self.session.is_recording(),
                "Recorded take",
            ),
        ]
        .into_iter()
        .filter_map(|(open, title)| open.then_some(title))
        .collect()
    }

    fn agent_status(&self) -> Status {
        Status {
            text: self.status.clone(),
            error: self.status_error,
        }
    }

    fn agent_outcome(&self, clips: Vec<u64>, control: Option<String>) -> Outcome {
        Outcome {
            status: self.agent_status(),
            modal: self.modal_name(),
            windows: self.open_windows(),
            closing: self.allow_close,
            clips,
            control: control
                .and_then(|id| self.agent_nodes().into_iter().find(|node| node.id == id)),
        }
    }

    fn agent_state(&self) -> StateView {
        let mut song = wire::song_view(&self.session);
        if let Some(lanes) = self.timeline_rect {
            for (view, clip) in song.clips.iter_mut().zip(&self.session.project().clips) {
                let rect = self.clip_rect(clip, lanes);
                view.rect = (rect != Rect::NOTHING).then(|| rect_view(rect));
            }
        }
        StateView {
            version: mixjam_techno::application::APP_VERSION,
            song,
            transport: wire::transport(&self.session),
            status: self.agent_status(),
            modal: self.modal_name(),
            path_prompt: self.path_prompt.as_ref().map(|prompt| {
                let spec = self.path_spec(&prompt.request);
                PathPromptView {
                    title: spec.title,
                    kind: match spec.kind {
                        PathKind::OpenFile => "open_file",
                        PathKind::SaveFile => "save_file",
                        PathKind::Folder => "folder",
                    },
                    extensions: spec
                        .filter
                        .map(|(_, extensions)| extensions.to_vec())
                        .unwrap_or_default(),
                    suggested_name: spec.file_name,
                }
            }),
            windows: self.open_windows(),
            selection: Selection {
                sample: self.selected_sample,
                clips: self.selected_clips.iter().copied().collect(),
            },
            tools: Tools {
                panel: if self.show_mixer { "mixer" } else { "sounds" },
                erase: self.erase,
                follow: self.follow_playhead,
                zoom: self.zoom,
                record_track: self.armed_track + 1,
            },
            busy: wire::busy(&self.session),
            arrangement: self
                .timeline_rect
                .zip(self.timeline_drop_rect)
                .map(|(lanes, visible)| ArrangementView {
                    visible: rect_view(visible),
                    left: lanes.left(),
                    top: lanes.top(),
                    bar_width: self.timeline_bar_width,
                    row_height: self.timeline_row_height,
                    tracks: self.timeline_tracks.iter().map(|track| track + 1).collect(),
                }),
        }
    }
}

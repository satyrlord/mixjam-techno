//! Agent driver. It turns requests from the control channel into the input
//! and commands that a person at the window produces, and reports what the
//! window then shows. Song changes still go through the session.
use super::*;
use eframe::egui::accesskit;
use mixjam_techno::application::agent::{
    self as wire, ArrangementView, Body, Modifier, Outcome, PathPromptView, PointerAction, Request,
    Responder, Selection, Server, StateView, Status, Tools, UiNode, Value,
};
use mixjam_techno::application::automation;
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};
#[path = "agent/request.rs"]
mod request;
#[path = "agent/state.rs"]
mod state;
#[path = "agent/tree.rs"]
mod tree;
pub(super) use tree::nodes;
use tree::*;

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
            if self.files.allow_close {
                current.steps.clear();
            }
            let Some(step) = current.steps.pop_front() else {
                if let Some(job) = job.take() {
                    let outcome = self.agent_outcome(ctx, job.clips, job.control);
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
                        .agent_nodes(ctx)
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
}

#[cfg(test)]
#[path = "agent/tests.rs"]
mod tests;

//! The controls on screen as an agent sees them: the accessibility tree read
//! into nodes, and the search for the node a request names.
use super::*;

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
        // A board and its named toolbar scope controls as a tool window did.
        Role::Window | Role::TabPanel | Role::Toolbar => "window",
        Role::ProgressIndicator => "progress",
        _ => "other",
    }
}

pub(crate) fn nodes(update: &accesskit::TreeUpdate) -> Vec<UiNode> {
    let root = update.tree.as_ref().map(|tree| tree.root);
    let mut parents = HashMap::new();
    let mut windows = HashMap::new();
    for (id, node) in &update.nodes {
        for child in node.children() {
            parents.insert(*child, *id);
        }
        if matches!(
            node.role(),
            accesskit::Role::Window
                | accesskit::Role::Dialog
                | accesskit::Role::TabPanel
                | accesskit::Role::Toolbar
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
                description: node.description().map(str::to_owned),
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

pub(super) fn in_window(node: &UiNode, window: Option<&str>) -> bool {
    window.is_none_or(|window| {
        node.window
            .as_deref()
            .is_some_and(|title| title.eq_ignore_ascii_case(window))
    })
}

/// The control that a request names by id or by label.
pub(super) fn find(
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

impl MixJamApp {
    /// The controls on screen. A modal takes all input, but the toolkit
    /// still reports the controls behind it as enabled.
    pub(super) fn agent_nodes(&self, ctx: &egui::Context) -> Vec<UiNode> {
        let mut nodes = self.agent.as_ref().map(Driver::nodes).unwrap_or_default();
        if let Some(modal) = self.blocking_modal(ctx) {
            for node in &mut nodes {
                node.enabled &= node.window.as_deref() == Some(modal);
            }
        }
        nodes
    }
}

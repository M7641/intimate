//! DOM + accessibility capture.
//!
//! The serialized HTML is kept as raw evidence; the accessibility tree is the
//! useful spec source — roles + labels + structure, which map onto semantic
//! React components far better than raw DOM does.

use anyhow::Result;
use chromiumoxide::cdp::browser_protocol::accessibility::{
    AxNode, AxValue, EnableParams, GetFullAxTreeParams,
};
use chromiumoxide::Page;

use crate::models::{Component, InputControl};

/// Roles that carry no structural meaning — dropped from the inventory.
const NOISE_ROLES: &[&str] = &[
    "generic",
    "none",
    "InlineTextBox",
    "LineBreak",
    "StaticText",
];

/// Roles that represent something a user can interact with.
const INPUT_ROLES: &[&str] = &[
    "button",
    "textbox",
    "searchbox",
    "combobox",
    "listbox",
    "menuitem",
    "checkbox",
    "radio",
    "slider",
    "spinbutton",
    "switch",
    "tab",
    "link",
];

pub async fn capture_html(page: &Page) -> Result<String> {
    Ok(page.content().await?)
}

/// Fetch the full accessibility tree and fold it into a component inventory
/// plus a list of interactive controls.
pub async fn capture_a11y(page: &Page) -> Result<(Vec<Component>, Vec<InputControl>)> {
    page.execute(EnableParams::default()).await?;
    let response = page.execute(GetFullAxTreeParams::default()).await?;

    let mut components = Vec::new();
    let mut inputs = Vec::new();

    for node in &response.result.nodes {
        if node.ignored {
            continue;
        }
        let Some(role) = role_of(node) else { continue };
        if NOISE_ROLES.contains(&role.as_str()) {
            continue;
        }
        let name = name_of(node);

        if INPUT_ROLES.contains(&role.as_str()) {
            inputs.push(InputControl {
                role: role.clone(),
                name: name.clone(),
            });
        }
        components.push(Component { role, name });
    }

    Ok((components, inputs))
}

fn role_of(node: &AxNode) -> Option<String> {
    string_value(node.role.as_ref())
}

fn name_of(node: &AxNode) -> Option<String> {
    string_value(node.name.as_ref()).filter(|s| !s.is_empty())
}

/// AX values box their payload as a JSON value; pull the string out.
fn string_value(value: Option<&AxValue>) -> Option<String> {
    value
        .and_then(|v| v.value.as_ref())
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

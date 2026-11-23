//! Declarative appearance and click-scoped actions for the GPUI workspace.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::package::{Manifest, Output};
use super::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Contribution {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar: Option<SidebarStyle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tab_menu: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SidebarStyle {
    pub tab_height: Option<u8>,
    pub group_height: Option<u8>,
    pub section_gap: Option<u8>,
    pub group_radius: Option<u8>,
    pub group_inset: Option<u8>,
    pub show_collapsed_count: Option<bool>,
    pub show_group_new_tab: Option<bool>,
}

impl Contribution {
    pub(super) fn validate(&self, manifest: &Manifest) -> Result<()> {
        if self.sidebar.is_none() && self.tab_menu.is_empty() {
            return Err(Error::new("invalid_workspace", "declare a sidebar style or tab menu"));
        }
        if let Some(style) = self.sidebar {
            for (name, value, minimum, maximum) in [
                ("tab_height", style.tab_height, 32, 64),
                ("group_height", style.group_height, 32, 64),
                ("section_gap", style.section_gap, 0, 24),
                ("group_radius", style.group_radius, 0, 24),
                ("group_inset", style.group_inset, 4, 24),
            ] {
                if value.is_some_and(|value| !(minimum..=maximum).contains(&value)) {
                    return Err(Error::new(
                        "invalid_workspace_style",
                        format!("{name} must be within {minimum}..={maximum}"),
                    ));
                }
            }
        }
        if self.tab_menu.len() > 12
            || self.tab_menu.iter().enumerate().any(|(index, id)| {
                self.tab_menu[..index].contains(id)
                    || !manifest.commands.iter().any(|command| {
                        command.id == *id
                            && command.handler.is_some()
                            && command.output == Output::Json
                    })
            })
        {
            return Err(Error::new(
                "invalid_workspace_menu",
                "declare up to 12 unique Luau commands with JSON output",
            ));
        }
        Ok(())
    }
}

pub(super) fn approval(manifest: &Manifest, source: &str) -> String {
    let data = serde_json::to_vec(manifest).expect("validated manifest is JSON");
    let mut digest = Sha256::new();
    digest.update((data.len() as u64).to_le_bytes());
    digest.update(data);
    digest.update(source.as_bytes());
    digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(feature = "gpui-shell")]
pub(super) fn run(
    package: &super::Package,
    command: &super::package::Command,
    source: &str,
    context: serde_json::Map<String, serde_json::Value>,
) -> Result<Option<TabAction>> {
    let mut params = command.params.as_object().cloned().expect("validated command params");
    params.extend(context);
    let params = serde_json::Value::Object(params);
    super::json::validate(&params)?;
    let result = super::lua::execute(
        package,
        command.handler.as_deref().expect("validated workspace handler"),
        Some(source),
        params,
        std::time::Duration::from_millis(250),
        &mut |_, _, _| Err(Error::new("runtime_unavailable", "tab menu handlers are data-only")),
    )?;
    let action: Option<TabAction> = serde_json::from_value(result.result)
        .map_err(|error| Error::new("invalid_tab_action", error))?;
    if let Some(TabAction::Rename { name }) = &action
        && (name.len() > 256 || name.chars().any(char::is_control))
    {
        return Err(Error::new("invalid_tab_action", "tab names must fit within 256 UTF-8 bytes"));
    }
    Ok(action)
}

/// A menu handler may ask for one operation on the addressed tab. The UI
/// resolves the tab identity again when the handler finishes.
#[cfg(feature = "gpui-shell")]
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum TabAction {
    Focus,
    Rename { name: String },
    Move { direction: Direction },
    AssignGroup { group_id: Option<String> },
    ShellPicker,
}

#[cfg(feature = "gpui-shell")]
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Direction {
    Previous,
    Next,
}

//! GPUI owns contribution snapshots, menu invocation and application of tab actions.

use super::*;
use crate::gpui_shell::toast::{ToastKind, toast};
use crate::i18n::Message;
use crate::plugins::install::{CommandSummary, Library};
use crate::plugins::workspace::{Direction, SidebarStyle, TabAction};
use gpui_component::menu::PopupMenuItem;
use serde_json::json;

pub(in crate::gpui_shell) struct PluginLibraryChanged;
impl gpui::Global for PluginLibraryChanged {}

#[derive(Clone)]
pub(super) struct MenuEntry {
    plugin_id: String,
    approval: String,
    command: CommandSummary,
}

pub(super) struct State {
    pub(super) sidebar: SidebarStyle,
    #[cfg(windows)]
    pub(super) toolbar: super::plugin_toolbar::State,
    menu: Vec<MenuEntry>,
    load: Option<gpui::Task<()>>,
    invoke: Option<gpui::Task<()>>,
    _library_subscription: Subscription,
}

impl State {
    #[cfg(all(windows, test, feature = "gpui-test-support"))]
    pub(super) fn clear_test_load(&mut self) {
        self.load = None;
    }

    pub(super) fn new(cx: &mut Context<NebulaWorkspace>) -> Self {
        Self {
            sidebar: SidebarStyle::default(),
            #[cfg(windows)]
            toolbar: super::plugin_toolbar::State::default(),
            menu: Vec::new(),
            load: None,
            invoke: None,
            _library_subscription: cx.observe_global::<PluginLibraryChanged>(|workspace, cx| {
                workspace.plugin_workspace.invoke = None;
                workspace.refresh_workspace_plugins(cx);
            }),
        }
    }
}

impl NebulaWorkspace {
    pub(super) fn refresh_workspace_plugins(&mut self, cx: &mut Context<Self>) {
        let task = cx.background_executor().spawn(async { Library::default().list() });
        self.plugin_workspace.load = Some(cx.spawn(async move |this, cx| {
            let listing = task.await;
            let _ = this.update(cx, |this, cx| {
                this.plugin_workspace.load = None;
                let plugins = match listing {
                    Ok(listing) => listing.plugins,
                    Err(error) => {
                        log::warn!("could not load workspace plugins: {error}");
                        Vec::new()
                    },
                };
                #[cfg(windows)]
                this.plugin_workspace.toolbar.load_entries(&plugins);
                this.plugin_workspace.sidebar = plugins
                    .iter()
                    .filter(|plugin| plugin.enabled)
                    .find_map(|plugin| plugin.workspace.as_ref()?.sidebar)
                    .unwrap_or_default();
                this.plugin_workspace.menu.clear();
                for plugin in plugins.into_iter().filter(|plugin| plugin.enabled) {
                    let Some(approval) = plugin.workspace_approval else { continue };
                    for command in plugin.workspace_commands {
                        this.plugin_workspace.menu.push(MenuEntry {
                            plugin_id: plugin.id.clone(),
                            approval: approval.clone(),
                            command,
                        });
                    }
                }
                this.clamp_tabs_scroll();
                cx.notify();
            });
        }));
    }

    pub(super) fn workspace_plugin_tab_menu(
        &self,
        menu: PopupMenu,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
        workspace: gpui::WeakEntity<Self>,
    ) -> PopupMenu {
        if self.plugin_workspace.menu.is_empty() || !self.tabs[ix].is_terminal() {
            return menu;
        }
        let language = crate::gpui_shell::config::ui_language(cx);
        let tab_id = self.tab_meta[ix].runtime_id.clone();
        let entries = self.plugin_workspace.menu.clone();
        let busy = self.plugin_workspace.invoke.is_some();
        menu.submenu(language.text(Message::SettingsPluginsTitle), window, cx, move |menu, _, _| {
            entries.iter().fold(menu, |menu, entry| {
                let workspace = workspace.clone();
                let tab_id = tab_id.clone();
                let entry = entry.clone();
                menu.item(PopupMenuItem::new(entry.command.title.clone()).disabled(busy).on_click(
                    move |_, window, cx| {
                        let _ = workspace.update(cx, |this, cx| {
                            this.run_workspace_plugin(&entry, &tab_id, window, cx);
                        });
                    },
                ))
            })
        })
        .separator()
    }

    fn run_workspace_plugin(
        &mut self,
        entry: &MenuEntry,
        tab_id: &crate::runtime_api::tabs::TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.plugin_workspace.invoke.is_some() {
            return;
        }
        let Some(ix) = self.plugin_target_index(tab_id) else { return };
        let groups = self.tab_groups.plugin_group_snapshot();
        let args = serde_json::Map::from_iter([
            (
                "tab".into(),
                json!({
                    "id": tab_id.0,
                    "title": self.tab_title(ix, cx).to_string(),
                    "active": self.active == ix,
                    "group_id": self.tab_group_at(ix).map(|id| id.to_string()),
                }),
            ),
            ("groups".into(), json!(groups)),
        ]);
        let entry = entry.clone();
        let tab_id = tab_id.clone();
        let task = cx.background_executor().spawn(async move {
            Library::default().workspace_tab_action(
                &entry.plugin_id,
                &entry.approval,
                &entry.command.id,
                args,
            )
        });
        self.plugin_workspace.invoke = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.plugin_workspace.invoke = None;
                let result = result.and_then(|action| match action {
                    Some(action) => this.apply_workspace_plugin_action(&tab_id, action, window, cx),
                    None => Ok(()),
                });
                if let Err(error) = result {
                    this.refresh_workspace_plugins(cx);
                    toast(
                        window,
                        cx,
                        ToastKind::Warning,
                        workspace_ui_language().format(
                            Message::SettingsPluginsFailed,
                            &[("error", &error.to_string())],
                        ),
                    );
                }
                cx.notify();
            });
        }));
    }

    fn plugin_target_index(&self, id: &crate::runtime_api::tabs::TabId) -> Option<usize> {
        self.tab_meta
            .iter()
            .position(|meta| &meta.runtime_id == id)
            .filter(|&ix| self.tabs[ix].is_terminal())
    }

    fn apply_workspace_plugin_action(
        &mut self,
        tab_id: &crate::runtime_api::tabs::TabId,
        action: TabAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::plugins::Result<()> {
        use crate::plugins::Error;
        let ix = self.plugin_target_index(tab_id).ok_or_else(|| {
            Error::new("target_not_found", "the tab closed or moved to another window")
        })?;
        match action {
            TabAction::Focus => self.activate_tab(ix, window, cx),
            TabAction::Rename { name } => {
                super::rename::apply_commit_rename(&mut self.tab_meta[ix], &name);
            },
            TabAction::Move { direction } => {
                self.move_tab_by_step(ix, direction == Direction::Next, window, cx);
            },
            TabAction::AssignGroup { group_id } => {
                let group_id = group_id
                    .map(|id| id.parse::<u64>())
                    .transpose()
                    .map_err(|_| Error::new("invalid_tab_action", "invalid group ID"))?;
                self.apply_plugin_group(ix, group_id, cx)?;
            },
            TabAction::ShellPicker => {
                self.open_shell_palette_for_group(self.tab_group_at(ix), window, cx);
            },
        }
        Ok(())
    }
}

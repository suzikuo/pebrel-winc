//! Windows title-bar buttons for explicitly authorized directory launchers.

use super::*;
use crate::i18n::Message;
use crate::plugins::Grant;
use crate::plugins::directory::{Launcher, Target};
use crate::plugins::install::Library;

const BUTTON_WIDTH: f32 = 88.0;

pub(in crate::gpui_shell) struct PluginLibraryChanged;
impl gpui::Global for PluginLibraryChanged {}

#[derive(Clone)]
struct Entry {
    plugin_id: String,
    label: SharedString,
    launcher: Launcher,
}

pub(super) struct State {
    entries: Vec<Entry>,
    load: Option<gpui::Task<()>>,
    launch: Option<gpui::Task<()>>,
    _library_subscription: Subscription,
}

impl State {
    pub(super) fn new(cx: &mut Context<NebulaWorkspace>) -> Self {
        Self {
            entries: Vec::new(),
            load: None,
            launch: None,
            _library_subscription: cx
                .observe_global::<PluginLibraryChanged>(NebulaWorkspace::refresh_plugin_toolbar),
        }
    }
}

impl NebulaWorkspace {
    fn plugin_directory_target(&self, cx: &App) -> Option<Target> {
        if self.settings_open {
            return None;
        }
        let view = self.tabs.get(self.active)?.focused_view()?.read(cx);
        if view.ssh_destination.is_some() {
            return None;
        }
        let cwd = view.working_directory()?;
        if let Some(wsl) = self.active_wsl_cwd(cx) {
            return Some(Target::Wsl { distribution: wsl.distro, directory: wsl.guest });
        }
        let directory = std::path::PathBuf::from(cwd);
        (view.inherits_windows_host_token() && directory.is_absolute())
            .then_some(Target::Windows(directory))
    }

    pub(super) fn refresh_plugin_toolbar(&mut self, cx: &mut Context<Self>) {
        let task = cx.background_executor().spawn(async { Library::default().list() });
        // Replacing the task cancels the previous UI continuation.
        self.plugin_toolbar.load = Some(cx.spawn(async move |this, cx| {
            let listing = task.await;
            let _ = this.update(cx, |this, cx| {
                this.plugin_toolbar.load = None;
                this.plugin_toolbar.entries.clear();
                match listing {
                    Ok(listing) => {
                        for plugin in listing.plugins.into_iter().filter(|plugin| {
                            plugin.enabled && plugin.grants.contains(&Grant::DirectoryLauncher)
                        }) {
                            let single = plugin.directory_launchers.len() == 1;
                            for launcher in plugin.directory_launchers {
                                let label = if single { &plugin.name } else { &launcher.title };
                                this.plugin_toolbar.entries.push(Entry {
                                    plugin_id: plugin.id.clone(),
                                    label: label.clone().into(),
                                    launcher,
                                });
                            }
                        }
                    },
                    Err(error) => log::warn!("could not load plugin launchers: {error}"),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn plugin_toolbar_width(&self) -> f32 {
        self.plugin_toolbar.entries.len() as f32
            * (BUTTON_WIDTH + window_titlebar::TITLE_BAR_CONTROL_GAP)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn set_test_plugin_toolbar(&mut self, enabled: bool) {
        self.plugin_toolbar.load = None;
        self.plugin_toolbar.entries.clear();
        if enabled {
            self.plugin_toolbar.entries.push(Entry {
                plugin_id: "example.directory-launcher".into(),
                label: "Directory Viewer".into(),
                launcher: Launcher {
                    id: "open-directory".into(),
                    title: "Open Directory".into(),
                    executable: r"C:\Tools\viewer.exe".into(),
                    windows_args: Vec::new(),
                    wsl_args: None,
                },
            });
        }
    }

    pub(super) fn render_plugin_toolbar(&self, cx: &mut Context<Self>) -> Vec<Button> {
        if self.plugin_toolbar.entries.is_empty() {
            return Vec::new();
        }
        let target = self.plugin_directory_target(cx);
        let language = crate::gpui_shell::config::ui_language(cx);
        self.plugin_toolbar
            .entries
            .iter()
            .map(|entry| {
                let supported =
                    target.as_ref().is_some_and(|target| target.supports(&entry.launcher));
                let disabled = !supported || self.plugin_toolbar.launch.is_some();
                let id: SharedString =
                    format!("directory-launcher/{}/{}", entry.plugin_id, entry.launcher.id).into();
                let title: SharedString = entry.launcher.title.clone().into();
                let tooltip = if supported {
                    title.clone()
                } else {
                    language.text(Message::SettingsPluginsLauncherNoTarget).into()
                };
                let entry = entry.clone();
                Button::new(id.clone())
                    .debug_selector(move || id.to_string())
                    .ghost()
                    .size(px(32.0))
                    .w(px(BUTTON_WIDTH))
                    .h(px(32.0))
                    .disabled(disabled)
                    .when(!disabled, |button| button.cursor_pointer())
                    .aria_label(title)
                    .tooltip(tooltip)
                    .child(div().truncate().child(entry.label.clone()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.run_plugin_launcher(&entry, window, cx);
                    }))
            })
            .collect()
    }

    fn run_plugin_launcher(&mut self, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) {
        use crate::gpui_shell::toast::{ToastKind, toast};

        if self.plugin_toolbar.launch.is_some() {
            return;
        }
        let Some(target) =
            self.plugin_directory_target(cx).filter(|target| target.supports(&entry.launcher))
        else {
            toast(
                window,
                cx,
                ToastKind::Warning,
                workspace_ui_language().text(Message::SettingsPluginsLauncherNoTarget),
            );
            return;
        };
        let entry = entry.clone();
        let task = cx.background_executor().spawn(async move {
            Library::default().launch_directory(&entry.plugin_id, &entry.launcher, target)
        });
        self.plugin_toolbar.launch = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.plugin_toolbar.launch = None;
                if let Err(error) = result {
                    this.refresh_plugin_toolbar(cx);
                    toast(
                        window,
                        cx,
                        ToastKind::Warning,
                        workspace_ui_language().format(
                            Message::SettingsPluginsLauncherFailed,
                            &[("error", &error.to_string())],
                        ),
                    );
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

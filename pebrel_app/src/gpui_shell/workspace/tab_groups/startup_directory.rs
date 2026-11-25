//! Store and edit the starting location shared by a tab group.

use super::{GroupEdit, NebulaWorkspace};
use crate::gpui_shell::prelude::*;
use crate::i18n::{Message, UiLanguage};
use crate::platform::launch_directory;
use crate::session::LaunchSession;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::select::SelectItem;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(in crate::gpui_shell::workspace) enum GroupStartupDirectory {
    Local { path: String },
    Wsl { distro: String, path: String },
}

impl GroupStartupDirectory {
    fn from_input(distro: Option<&str>, path: &str) -> Option<Self> {
        if path.chars().any(char::is_control) {
            return None;
        }
        match distro {
            Some(distro)
                if !distro.is_empty()
                    && launch_directory::guest_path(Path::new(path)).is_some()
                    && crate::shell_detect::wsl_accepts_startup_arg(path) =>
            {
                Some(Self::Wsl { distro: distro.to_owned(), path: path.to_owned() })
            },
            None if Path::new(path).is_absolute() => Some(Self::Local { path: path.to_owned() }),
            _ => None,
        }
    }

    fn distro(&self) -> Option<&str> {
        match self {
            Self::Local { .. } => None,
            Self::Wsl { distro, .. } => Some(distro),
        }
    }

    pub(in crate::gpui_shell::workspace) fn path(&self) -> &str {
        match self {
            Self::Local { path } | Self::Wsl { path, .. } => path,
        }
    }

    pub(in crate::gpui_shell::workspace) fn matches_program(
        &self,
        program: &str,
        args: &[String],
    ) -> bool {
        match self {
            Self::Local { .. } => !launch_directory::is_wsl_program(program),
            Self::Wsl { distro, .. } => {
                launch_directory::is_wsl_program(program)
                    && crate::shell_detect::wsl_launch_distro(program, args)
                        == Some(distro.as_str())
            },
        }
    }

    pub(in crate::gpui_shell::workspace) fn for_launch(
        &self,
        launch: &LaunchSession,
    ) -> Option<&Path> {
        let (program, args) = match launch {
            LaunchSession::Shell { program, args, .. } => (program, args),
            LaunchSession::Profile { command, args, .. } => (command, args),
            _ => return None,
        };
        self.matches_program(program, args).then(|| Path::new(self.path()))
    }
}

#[derive(Clone)]
struct DirectoryEnvironment {
    title: SharedString,
    distro: Option<String>,
}

impl DirectoryEnvironment {
    fn new(distro: Option<String>, language: UiLanguage) -> Self {
        let title = match &distro {
            Some(distro) => format!("WSL · {distro}").into(),
            None => language.text(Message::TabGroupsStartupLocal).into(),
        };
        Self { title, distro }
    }
}

impl SelectItem for DirectoryEnvironment {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.distro
    }
}

struct TabDirectory {
    title: SharedString,
    directory: GroupStartupDirectory,
}

impl NebulaWorkspace {
    fn group_tab_directories(&self, id: u64, cx: &App) -> Vec<TabDirectory> {
        self.tabs
            .iter()
            .enumerate()
            .filter_map(|(ix, tab)| {
                if self.tab_group_at(ix) != Some(id) {
                    return None;
                }
                let view = tab.focused_view()?.read(cx);
                // A nested Option distinguishes a host pane from WSL with unknown distribution.
                let distro = match view.exec_context.as_ref()?.wsl_distribution() {
                    Some(distro) => Some(distro?),
                    None => None,
                };
                let directory =
                    GroupStartupDirectory::from_input(distro, view.working_directory()?)?;
                let title = self.tab_title(ix, cx);
                let title = match distro {
                    Some(distro) => format!("{title} · {distro}").into(),
                    None => title,
                };
                Some(TabDirectory { title, directory })
            })
            .collect()
    }

    pub(super) fn edit_tab_group_startup_directory(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = crate::gpui_shell::config::ui_language(cx);
        let current = self.tab_groups.startup_directory(id).cloned();
        let sources = self.group_tab_directories(id, cx);
        let source_count = sources.len();
        let mut distros = Vec::new();
        #[cfg(windows)]
        distros.extend(crate::shell_detect::wsl_distro_names());
        distros.extend(
            current
                .iter()
                .chain(sources.iter().map(|source| &source.directory))
                .filter_map(GroupStartupDirectory::distro)
                .map(str::to_owned),
        );
        distros.sort();
        distros.dedup();
        let selected_distro = current.as_ref().and_then(GroupStartupDirectory::distro);
        let selected_row = selected_distro
            .and_then(|name| distros.iter().position(|distro| distro == name))
            .map_or(0, |index| index + 1);
        let environments = [None]
            .into_iter()
            .chain(distros.into_iter().map(Some))
            .map(|distro| DirectoryEnvironment::new(distro, language))
            .collect::<Vec<_>>();
        let environment = cx.new(|cx| {
            SelectState::new(environments, Some(IndexPath::default().row(selected_row)), window, cx)
        });
        let path = cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .placeholder(language.text(Message::TabGroupsStartupPath));
            if let Some(current) = &current {
                input.set_value(current.path(), window, cx);
            }
            input
        });
        let workspace = cx.entity().downgrade();
        let edit_path = path.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let selected_environment = environment.clone();
            let selected_path = path.clone();
            let workspace = workspace.clone();
            let muted = cx.theme().muted_foreground;
            let border = cx.theme().border;
            let rows = sources.iter().enumerate().map(|(index, source)| {
                let path = path.clone();
                let environment = environment.clone();
                let directory = source.directory.clone();
                let detail = directory.path().to_owned();
                Button::new(("tab-group-directory-source", index))
                    .ghost()
                    .w_full()
                    .h(px(38.0))
                    .px_2()
                    .aria_label(source.title.clone())
                    .tooltip(detail.clone())
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .items_start()
                            .child(
                                div().max_w_full().truncate().text_xs().child(source.title.clone()),
                            )
                            .child(
                                div()
                                    .max_w_full()
                                    .truncate()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(detail),
                            ),
                    )
                    .on_click(move |_, window, cx| {
                        let distro = directory.distro().map(str::to_owned);
                        environment.update(cx, |state, cx| {
                            state.set_selected_value(&distro, window, cx);
                            cx.notify();
                        });
                        path.update(cx, |state, cx| state.set_value(directory.path(), window, cx));
                    })
            });
            center_modal_dialog(dialog, window, 264.0 + source_count.min(3) as f32 * 38.0)
                .title(
                    div()
                        .text_sm()
                        .font_semibold()
                        .child(language.text(Message::TabGroupsStartupDirectory)),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(language.text(Message::TabGroupsStartupFromTab)),
                                )
                                .when(source_count == 0, |group| {
                                    group.child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .child(language.text(Message::TabGroupsStartupNoTabs)),
                                    )
                                })
                                .when(source_count > 0, |group| {
                                    group.child(
                                        v_flex()
                                            .id("tab-group-directory-sources")
                                            .w_full()
                                            .max_h(px(116.0))
                                            .overflow_y_scroll()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(border)
                                            .children(rows),
                                    )
                                }),
                        )
                        .child(Select::new(&environment).w_full())
                        .child(
                            Input::new(&path).w_full().accessibility_id("tab-group-startup-path"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(language.text(Message::TabGroupsStartupClearHint)),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .child(div().flex_1())
                        .child(
                            DialogClose::new().child(
                                Button::new("tab-group-directory-cancel")
                                    .label(language.text(Message::CommonCancel)),
                            ),
                        )
                        .child(
                            DialogAction::new().child(
                                Button::new("tab-group-directory-save")
                                    .primary()
                                    .label(language.text(Message::CommonSave)),
                            ),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    let path = selected_path.read(cx).value();
                    let distro = selected_environment
                        .read(cx)
                        .selected_value()
                        .and_then(|distro| distro.as_deref());
                    let directory = GroupStartupDirectory::from_input(distro, &path);
                    if directory.is_none() && !path.trim().is_empty() {
                        crate::gpui_shell::toast::toast(
                            window,
                            cx,
                            crate::display::ToastKind::Warning,
                            language.text(Message::TabGroupsStartupInvalidPath),
                        );
                        return false;
                    }
                    workspace
                        .update(cx, |this, cx| {
                            this.apply_tab_group_edit(
                                GroupEdit::StartupDirectory(id, directory),
                                "",
                                window,
                                cx,
                            )
                        })
                        .unwrap_or(true)
                })
        });
        if source_count == 0 {
            edit_path.update(cx, |state, cx| state.focus(window, cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_startup_directory_requires_an_absolute_path_in_its_environment() {
        let native = std::env::temp_dir().join("local project");
        assert!(GroupStartupDirectory::from_input(None, native.to_str().unwrap()).is_some());
        assert!(GroupStartupDirectory::from_input(None, "project").is_none());
        assert!(GroupStartupDirectory::from_input(None, "").is_none());
        for invalid in ["project", "//server/share", "/root/\"project\"", "/root/\nproject"] {
            assert!(GroupStartupDirectory::from_input(Some("Ubuntu"), invalid).is_none());
        }
        assert!(GroupStartupDirectory::from_input(Some("Ubuntu"), "/root/项目 space").is_some());
    }

    #[test]
    fn group_startup_directory_only_matches_named_launchers_in_its_environment() {
        let local = GroupStartupDirectory::Local { path: "unused".into() };
        let guest =
            GroupStartupDirectory::Wsl { distro: "Ubuntu".into(), path: "/root/pro2".into() };
        assert!(local.matches_program("local-shell", &[]));
        assert!(!guest.matches_program("local-shell", &[]));
        assert!(!guest.matches_program("wsl.exe", &[]));
        assert!(!guest.matches_program("wsl.exe", &["-d".into(), "Debian".into()]));
        assert!(!guest.matches_program(
            "wsl.exe",
            &["--exec".into(), "program".into(), "-d".into(), "Ubuntu".into()]
        ));
        if cfg!(windows) {
            let args = ["--distribution=Ubuntu".into(), "--user=root".into()];
            assert!(guest.matches_program("wsl.exe", &args));
            assert!(!local.matches_program("wsl.exe", &args));
        }
        assert_eq!(guest.for_launch(&LaunchSession::Ssh { host: "Ubuntu".into() }), None);
    }

    #[cfg(windows)]
    #[test]
    fn group_startup_directory_overrides_wsl_profile_without_changing_its_command() {
        let group =
            GroupStartupDirectory::from_input(Some("Ubuntu"), "/root/project space").unwrap();
        let profile = LaunchSession::Profile {
            name: "Project".into(),
            command: "wsl.exe".into(),
            args: ["-d", "Ubuntu", "--user", "root", "--cd", "/old", "--exec", "zsh", "--cd"]
                .map(str::to_owned)
                .to_vec(),
            cwd: Some("/old".into()),
            shell_id: None,
            profile_id: None,
            env_snapshot: None,
        };
        let path = group.for_launch(&profile).expect("same distribution");
        let launch = super::super::super::shell_launch::at_explicit_directory(profile, Some(path));
        let LaunchSession::Profile { args, cwd, .. } = launch else { panic!("expected a profile") };
        assert_eq!(cwd.as_deref(), Some("/root/project space"));
        assert_eq!(
            args,
            [
                "--cd",
                "\"/root/project space\"",
                "-d",
                "Ubuntu",
                "--user",
                "root",
                "--exec",
                "zsh",
                "--cd"
            ]
        );
    }
}

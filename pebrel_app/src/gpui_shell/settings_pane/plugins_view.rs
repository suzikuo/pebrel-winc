//! Plugin Center layout, installation review, and installed plugin cards.

use super::plugins::{Busy, Pending};
use super::*;
use crate::i18n::Message;
use crate::plugins::Grant;
use crate::plugins::install::{Broken, Installed};
use gpui_component::button::ButtonCustomVariant;

fn grant_label(grant: Grant) -> Message {
    match grant {
        Grant::ThemeLibrary => Message::SettingsPluginsGrantThemeLibrary,
        Grant::DirectoryLauncher => Message::SettingsPluginsGrantDirectoryLauncher,
        Grant::WorkspaceUi => Message::SettingsPluginsGrantWorkspaceUi,
    }
}

impl SettingsPane {
    pub(super) fn section_plugins(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        if self.plugins.listing.is_none() && self.plugins.idle() {
            self.refresh_plugins(cx);
        }
        let language = crate::gpui_shell::config::ui_language(cx);
        let theme = cx.theme();
        let base = self.font_size_px(cx);
        let idle = self.plugins.idle();
        let community = self.plugins.community_open;
        let listing = self.plugins.listing.as_ref().and_then(|listing| listing.as_ref().ok());
        let count = listing.map(|listing| listing.plugins.len() + listing.broken.len());
        let heading = h_flex()
            .flex_1()
            .min_w_0()
            .flex_basis(px(280.0))
            .items_start()
            .gap_2()
            .child(plugin_icon(puzzle_icon(), theme.link))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(base * 1.12))
                            .font_semibold()
                            .child(language.text(Message::SettingsPluginsCenterTitle)),
                    )
                    .child(
                        div()
                            .max_w(px(520.0))
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .whitespace_normal()
                            .child(language.text(Message::SettingsPluginsDescription)),
                    ),
            );
        let install = plugin_button("plugins-install")
            .debug_selector(|| "plugins-install".into())
            .primary()
            .icon(IconName::FolderOpen)
            .label(language.text(match self.plugins.busy {
                Some(Busy::Installing) => Message::SettingsPluginsInstalling,
                _ => Message::SettingsPluginsInstall,
            }))
            .loading(matches!(self.plugins.busy, Some(Busy::Picking | Busy::Installing)))
            .disabled(!idle)
            .on_click(cx.listener(|this, _, window, cx| this.pick_plugin_folder(window, cx)));
        let hero = design::settings_card(cx)
            .bg(crate::gpui_shell::theme::settings_card_fill(cx, Some(theme.link.opacity(0.07))))
            .p_3()
            .child(h_flex().w_full().flex_wrap().gap_2().child(heading).child(install));
        let tabs = h_flex()
            .flex_wrap()
            .gap_1()
            .child(
                plugin_tab("plugins-installed", !community, cx)
                    .icon(IconName::Inbox)
                    .label(language.text(Message::SettingsPluginsInstalledTab))
                    .when_some(count, |button, count| {
                        button.child(
                            div()
                                .min_w(px(22.0))
                                .px_1()
                                .py(px(2.0))
                                .text_center()
                                .text_xs()
                                .rounded_full()
                                .bg(theme.foreground.opacity(0.08))
                                .child(count.to_string()),
                        )
                    })
                    .disabled(!idle)
                    .on_click(cx.listener(|this, _, _, cx| this.show_plugin_community(false, cx))),
            )
            .child(
                plugin_tab("plugins-community", community, cx)
                    .icon(IconName::Globe)
                    .label(language.text(Message::SettingsPluginsCommunityTitle))
                    .disabled(!idle)
                    .on_click(cx.listener(|this, _, _, cx| this.show_plugin_community(true, cx))),
            );
        let page = v_flex()
            .w_full()
            .min_w_0()
            .max_w(px(820.0))
            .gap_2()
            .child(hero)
            .when_some(self.plugins.pending.as_ref(), |page, pending| {
                page.child(self.plugin_grant_card(pending, cx))
            })
            .when_some(self.plugins.feedback.as_ref(), |page, (id, result)| {
                if id.is_none() || community {
                    page.child(feedback_line(None, result, cx))
                } else {
                    page
                }
            })
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .pb_1()
                    .border_b_1()
                    .border_color(crate::gpui_shell::theme::settings_hairline(cx))
                    .child(tabs)
                    .map(|bar| {
                        if community {
                            bar.child(self.plugin_community_actions(cx))
                        } else {
                            bar.child(
                                plugin_button("plugins-refresh")
                                    .debug_selector(|| "plugins-refresh".into())
                                    .ghost()
                                    .icon(IconName::Redo2)
                                    .label(language.text(Message::SettingsPluginsRefresh))
                                    .loading(self.plugins.loading)
                                    .disabled(!idle)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.refresh_plugins(cx)),
                                    ),
                            )
                        }
                    }),
            );
        if community {
            page.child(self.plugin_community_section(cx))
        } else {
            page.child(self.installed_plugins(cx))
        }
    }

    fn installed_plugins(&self, cx: &mut Context<Self>) -> gpui::Div {
        let language = crate::gpui_shell::config::ui_language(cx);
        let theme = cx.theme();
        match &self.plugins.listing {
            None => plugin_notice(
                IconName::Loader,
                theme.muted_foreground,
                language.text(Message::SettingsPluginsLoading),
                cx,
            ),
            Some(Err(error)) => plugin_notice(
                IconName::TriangleAlert,
                theme.danger,
                language.format(Message::SettingsPluginsFailed, &[("error", error)]),
                cx,
            ),
            Some(Ok(listing)) if listing.plugins.is_empty() && listing.broken.is_empty() => {
                plugin_notice(
                    IconName::Inbox,
                    theme.muted_foreground,
                    language.text(Message::SettingsPluginsEmpty),
                    cx,
                )
                .debug_selector(|| "plugins-empty".into())
            },
            Some(Ok(listing)) => {
                let mut cards = v_flex().w_full().gap_2();
                for (index, plugin) in listing.plugins.iter().enumerate() {
                    cards = cards.child(self.plugin_row(index, plugin, cx));
                }
                for (offset, broken) in listing.broken.iter().enumerate() {
                    cards = cards.child(self.broken_plugin_row(
                        listing.plugins.len() + offset,
                        broken,
                        cx,
                    ));
                }
                cards
                    .when(
                        listing.plugins.iter().any(|plugin| {
                            plugin.workspace.as_ref().is_some_and(|ui| ui.scratch_terminal)
                        }),
                        |cards| cards.child(self.scratch_terminal_settings(cx)),
                    )
                    .child(
                        h_flex()
                            .items_start()
                            .gap_2()
                            .px_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(Icon::new(IconName::Info).size(px(14.0)).flex_shrink_0())
                            .child(
                                div()
                                    .min_w_0()
                                    .whitespace_normal()
                                    .child(language.text(Message::SettingsPluginsThemesKept)),
                            ),
                    )
            },
        }
    }

    fn plugin_grant_card(
        &self,
        pending: &Pending,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let language = crate::gpui_shell::config::ui_language(cx);
        let theme = cx.theme();
        let mut grants = v_flex().gap_2();
        for grant in &pending.request.requested {
            if *grant == Grant::WorkspaceUi
                && let Some(ui) = &pending.request.workspace
                && ui.scratch_terminal
            {
                grants = grants.child(permission_line(
                    language.text(Message::SettingsPluginsGrantScratchTerminal),
                    theme.link,
                ));
                if ui.sidebar.is_none() && ui.tab_menu.is_empty() {
                    continue;
                }
            }
            grants = grants.child(permission_line(language.text(grant_label(*grant)), theme.link));
        }
        for launcher in &pending.request.directory_launchers {
            let details = if launcher.opens_explorer() {
                language
                    .format(Message::SettingsPluginsLauncherExplorer, &[("title", &launcher.title)])
            } else {
                language.format(
                    Message::SettingsPluginsLauncherWindows,
                    &[
                        ("title", &launcher.title),
                        ("path", &launcher.executable),
                        ("args", &format!("{:?}", launcher.windows_args)),
                    ],
                )
            };
            grants = grants.child(
                v_flex()
                    .gap_1()
                    .pl_5()
                    .child(div().text_xs().whitespace_normal().child(details))
                    .when_some(
                        launcher.wsl_args.as_ref().filter(|_| !launcher.opens_explorer()),
                        |details, args| {
                            details.child(div().text_xs().whitespace_normal().child(
                                language.format(
                                    Message::SettingsPluginsLauncherWsl,
                                    &[("args", &format!("{args:?}"))],
                                ),
                            ))
                        },
                    ),
            );
        }
        design::settings_card(cx)
            .id("plugins-grant")
            .debug_selector(|| "plugins-grant".into())
            .border_1()
            .border_color(theme.link.opacity(0.45))
            .p_3()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(plugin_icon(
                        Icon::empty().path(crate::gpui_shell::assets::nav::SHIELD),
                        theme.link,
                    ))
                    .child(div().min_w_0().text_sm().font_semibold().whitespace_normal().child(
                        language.format(
                            Message::SettingsPluginsGrantTitle,
                            &[("name", &pending.request.name)],
                        ),
                    )),
            )
            .child(grants)
            .when(pending.community.is_none(), |card| {
                card.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .whitespace_normal()
                        .child(pending.path.display().to_string()),
                )
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .pt_1()
                    .border_t_1()
                    .border_color(crate::gpui_shell::theme::settings_hairline(cx))
                    .child(
                        plugin_button("plugins-grant-allow")
                            .debug_selector(|| "plugins-grant-allow".into())
                            .primary()
                            .label(language.text(Message::SettingsPluginsGrantAllow))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.confirm_plugin_install(cx)),
                            ),
                    )
                    .child(
                        plugin_button("plugins-grant-cancel")
                            .debug_selector(|| "plugins-grant-cancel".into())
                            .ghost()
                            .label(language.text(Message::SettingsPluginsGrantCancel))
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_plugin_install(cx))),
                    )
                    .when_some(pending.community.as_ref(), |actions, (url, _)| {
                        let url = url.clone();
                        actions.child(
                            plugin_button("plugins-community-source")
                                .ghost()
                                .icon(IconName::ExternalLink)
                                .label(language.text(Message::SettingsPluginsCommunitySource))
                                .on_click(move |_, _, cx| cx.open_url(&url)),
                        )
                    }),
            )
    }

    fn plugin_row(
        &self,
        index: usize,
        plugin: &Installed,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let language = crate::gpui_shell::config::ui_language(cx);
        let theme = cx.theme();
        let idle = self.plugins.idle();
        let busy = matches!(&self.plugins.busy, Some(Busy::Plugin(id)) if id == &plugin.id);
        let status = language.text(if plugin.enabled {
            Message::SettingsPluginsEnabled
        } else {
            Message::SettingsPluginsDisabled
        });
        let icon_color = if plugin.enabled { theme.link } else { theme.muted_foreground };
        let id = plugin.id.clone();
        let mut card = row_frame(index, cx).child(
            h_flex()
                .flex_wrap()
                .justify_between()
                .gap_2()
                .child(
                    h_flex()
                        .flex_1()
                        .flex_basis(px(180.0))
                        .min_w_0()
                        .gap_2()
                        .items_center()
                        .child(plugin_icon(puzzle_icon(), icon_color))
                        .child(
                            v_flex()
                                .min_w_0()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .whitespace_normal()
                                        .child(plugin.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .whitespace_normal()
                                        .child(format!("{} · {}", plugin.version, plugin.id)),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .gap_1()
                        .child(plugin_status(
                            status,
                            if plugin.enabled { theme.success } else { theme.muted_foreground },
                        ))
                        .child(
                            div()
                                .id(("plugin-enabled", index))
                                .debug_selector(move || format!("plugin-enabled-{index}"))
                                .child(
                                    crate::gpui_shell::widgets::NebulaSwitch::new(format!(
                                        "plugin-enabled-{index}"
                                    ))
                                    .aria_label(plugin.name.clone())
                                    .small()
                                    .checked(plugin.enabled)
                                    .disabled(!idle)
                                    .on_click(cx.listener(
                                        move |this, checked, _, cx| {
                                            this.set_plugin_enabled(id.clone(), *checked, cx)
                                        },
                                    )),
                                ),
                        )
                        .child(self.uninstall_plugin_button(index, &plugin.id, cx)),
                ),
        );
        if !plugin.grants.is_empty() {
            let granted: Vec<_> = plugin
                .grants
                .iter()
                .map(|grant| {
                    let label = if *grant == Grant::WorkspaceUi
                        && plugin.workspace.as_ref().is_some_and(|ui| {
                            ui.scratch_terminal && ui.sidebar.is_none() && ui.tab_menu.is_empty()
                        }) {
                        Message::SettingsPluginsGrantScratchTerminal
                    } else {
                        grant_label(*grant)
                    };
                    language.text(label)
                })
                .collect();
            card = card.child(
                div().text_xs().text_color(theme.muted_foreground).whitespace_normal().child(
                    language.format(
                        Message::SettingsPluginsGranted,
                        &[("grants", &granted.join(", "))],
                    ),
                ),
            );
        }
        let contributions = installed_contributions(plugin, language);
        if !contributions.is_empty() {
            card = card.child(h_flex().flex_wrap().gap_1().children(
                contributions.into_iter().map(|text| plugin_status(text, theme.muted_foreground)),
            ));
        }
        if let Some((Some(row), result)) = &self.plugins.feedback
            && *row == plugin.id
        {
            card = card.child(feedback_line(Some(index), result, cx));
        }
        let mut commands = h_flex().min_w_0().flex_wrap().gap_1();
        for (command_index, command) in plugin.theme_commands.iter().enumerate() {
            let (id, command_id) = (plugin.id.clone(), command.id.clone());
            commands = commands.child(
                plugin_button(("plugin-generate", index * 64 + command_index))
                    .debug_selector(move || format!("plugin-generate-{index}-{command_index}"))
                    .outline()
                    .icon(IconName::Palette)
                    .tooltip(format!(
                        "{}\n{}",
                        command.title,
                        language.text(Message::SettingsPluginsGenerateTooltip)
                    ))
                    .aria_label(command.title.clone())
                    .max_w_full()
                    .child(div().min_w_0().truncate().child(command.title.clone()))
                    .loading(busy)
                    .disabled(!idle || !plugin.enabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.generate_plugin_theme(id.clone(), command_id.clone(), cx)
                    })),
            );
        }
        card.when(!plugin.theme_commands.is_empty(), |card| card.child(commands))
    }

    fn broken_plugin_row(
        &self,
        index: usize,
        broken: &Broken,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let language = crate::gpui_shell::config::ui_language(cx);
        row_frame(index, cx).child(
            h_flex()
                .items_start()
                .gap_2()
                .child(plugin_icon(IconName::TriangleAlert, cx.theme().danger))
                .child(
                    v_flex()
                        .min_w_0()
                        .flex_1()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .whitespace_normal()
                                .child(broken.id.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .whitespace_normal()
                                .child(language.format(
                                    Message::SettingsPluginsBroken,
                                    &[("error", &broken.error)],
                                )),
                        ),
                )
                .child(self.uninstall_plugin_button(index, &broken.id, cx)),
        )
    }

    fn uninstall_plugin_button(&self, index: usize, id: &str, cx: &Context<Self>) -> Button {
        let language = crate::gpui_shell::config::ui_language(cx);
        let id = id.to_owned();
        let action = language.text(Message::SettingsPluginsUninstall);
        plugin_button(("plugin-uninstall", index))
            .debug_selector(move || format!("plugin-uninstall-{index}"))
            .ghost()
            .w(px(32.0))
            .icon(Icon::empty().path(crate::gpui_shell::assets::nav::TRASH))
            .aria_label(action)
            .tooltip(action)
            .disabled(!self.plugins.idle())
            .on_click(cx.listener(move |this, _, _, cx| this.uninstall_plugin(id.clone(), cx)))
    }
}

fn installed_contributions(plugin: &Installed, language: crate::i18n::UiLanguage) -> Vec<String> {
    let mut contributions = Vec::new();
    if plugin.bundled_themes > 0 {
        contributions.push(language.format(
            Message::SettingsPluginsBundledThemes,
            &[("count", &plugin.bundled_themes.to_string())],
        ));
    }
    if !plugin.directory_launchers.is_empty() {
        let titles: Vec<_> =
            plugin.directory_launchers.iter().map(|action| action.title.as_str()).collect();
        contributions.push(
            language.format(
                Message::SettingsPluginsDirectoryActions,
                &[("actions", &titles.join(", "))],
            ),
        );
    }
    if !plugin.workspace_commands.is_empty() {
        let titles: Vec<_> =
            plugin.workspace_commands.iter().map(|action| action.title.as_str()).collect();
        contributions.push(
            language.format(Message::SettingsPluginsTabActions, &[("actions", &titles.join(", "))]),
        );
    }
    if let Some(workspace) = &plugin.workspace {
        if workspace.sidebar.is_some() {
            contributions.push(language.text(Message::SettingsPluginsSidebarStyle).to_owned());
        }
        if workspace.scratch_terminal {
            contributions.push(language.text(Message::ScratchTerminalTitle).to_owned());
        }
    }
    contributions
}

pub(super) fn plugin_button(id: impl Into<gpui::ElementId>) -> Button {
    Button::new(id).small().h(px(32.0)).rounded(px(6.0))
}

fn plugin_tab(id: &'static str, selected: bool, cx: &App) -> Button {
    let theme = cx.theme();
    let color = if selected { theme.link } else { theme.foreground };
    plugin_button(id)
        .debug_selector(move || id.into())
        .custom(
            ButtonCustomVariant::new(cx)
                .foreground(color)
                .color(if selected { color.opacity(0.12) } else { theme.transparent })
                .hover(color.opacity(if selected { 0.17 } else { 0.07 }))
                .active(color.opacity(if selected { 0.22 } else { 0.12 })),
        )
        .rounded(px(7.0))
        .toggled(selected)
}

pub(super) fn puzzle_icon() -> Icon {
    Icon::empty().path(crate::gpui_shell::assets::nav::PUZZLE)
}

pub(super) fn plugin_icon(icon: impl Into<Icon>, color: Hsla) -> gpui::Div {
    h_flex()
        .size(px(30.0))
        .flex_shrink_0()
        .justify_center()
        .rounded(px(7.0))
        .bg(color.opacity(0.13))
        .text_color(color)
        .child(icon.into().size(px(16.0)))
}

pub(super) fn plugin_status(text: impl Into<SharedString>, color: Hsla) -> gpui::Div {
    div()
        .max_w_full()
        .px_2()
        .py(px(2.0))
        .rounded(px(5.0))
        .bg(color.opacity(0.12))
        .text_color(color)
        .text_xs()
        .whitespace_normal()
        .child(text.into())
}

pub(super) fn plugin_notice(
    icon: IconName,
    color: Hsla,
    text: impl Into<SharedString>,
    cx: &App,
) -> gpui::Div {
    design::settings_card(cx).p_3().child(
        h_flex().gap_2().child(plugin_icon(icon, color)).child(
            div().min_w_0().text_xs().text_color(color).whitespace_normal().child(text.into()),
        ),
    )
}

fn row_frame(index: usize, cx: &App) -> gpui::Stateful<gpui::Div> {
    design::settings_card(cx)
        .id(("plugin-row", index))
        .debug_selector(move || format!("plugin-row-{index}"))
        .p_3()
        .gap_2()
}

fn permission_line(text: &str, color: Hsla) -> gpui::Div {
    h_flex()
        .items_start()
        .gap_2()
        .child(Icon::new(IconName::Check).size(px(14.0)).text_color(color).flex_shrink_0())
        .child(div().min_w_0().text_xs().whitespace_normal().child(text.to_owned()))
}

/// `row` is `None` for page-level results and the row index otherwise.
fn feedback_line(
    row: Option<usize>,
    result: &Result<String, String>,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let (text, icon, color) = match result {
        Ok(message) => (message.clone(), IconName::CircleCheck, cx.theme().success),
        Err(message) => (message.clone(), IconName::TriangleAlert, cx.theme().danger),
    };
    let selector = match row {
        Some(index) => format!("plugin-feedback-{index}"),
        None => "plugins-feedback".to_owned(),
    };
    h_flex()
        .id(("plugin-feedback", row.map_or(usize::MAX, |index| index)))
        .debug_selector(move || selector.clone())
        .items_start()
        .gap_2()
        .p_2()
        .rounded(px(6.0))
        .bg(color.opacity(0.10))
        .text_xs()
        .text_color(color)
        .child(Icon::new(icon).size(px(16.0)).flex_shrink_0())
        .child(div().min_w_0().whitespace_normal().child(text))
}

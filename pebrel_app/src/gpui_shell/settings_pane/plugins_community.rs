//! Community catalog controls and ownership of the download-to-review handoff.

use std::path::PathBuf;
use std::sync::Arc;

use super::plugins::{Busy, Pending};
use super::plugins_view::{plugin_button, plugin_icon, plugin_notice, plugin_status, puzzle_icon};
use super::*;
use crate::i18n::{Message, UiLanguage};
use crate::plugins::community::{self, Entry};

impl SettingsPane {
    pub(super) fn show_plugin_community(&mut self, open: bool, cx: &mut Context<Self>) {
        self.plugins.community_open = open;
        if open && self.plugins.community.is_none() {
            self.refresh_plugin_community(cx);
        }
        cx.notify();
    }

    fn refresh_plugin_community(&mut self, cx: &mut Context<Self>) {
        if !self.plugins.idle() {
            return;
        }
        self.plugins.busy = Some(Busy::CommunityCatalog);
        let task = cx.background_executor().spawn(async { community::fetch_catalog() });
        cx.spawn(async move |this, cx| {
            let catalog = task.await.map_err(|error| error.to_string());
            let _ = this.update(cx, |this, cx| {
                this.plugins.community = Some(catalog);
                this.plugins.busy = None;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn review_community_plugin(
        &mut self,
        entry: Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.plugins.idle() || !entry.supported() {
            return;
        }
        let language = crate::gpui_shell::config::ui_language(cx);
        let selected = entry.choose_executable.then(|| {
            cx.prompt_for_paths(gpui::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some(
                    language.text(Message::SettingsPluginsCommunityChooseExecutable).into(),
                ),
            })
        });
        self.plugins.busy = Some(Busy::CommunityPackage);
        self.plugins.feedback = None;
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let executable = match selected {
                Some(picked) => match picked.await {
                    Ok(Ok(Some(paths))) => Ok(paths.into_iter().next()),
                    Ok(Err(error)) => Err(error.to_string()),
                    Ok(Ok(None)) | Err(_) => Ok(None),
                },
                None => Ok(None),
            };
            let result = match executable {
                Ok(None) if entry.choose_executable => Ok(None),
                Err(error) => Err(error),
                Ok(path) => executor.spawn(async move { stage(&entry, path) }).await.map(Some),
            };
            let _ = this.update(cx, |this, cx| {
                this.plugins.busy = None;
                match result {
                    Ok(pending) => this.plugins.pending = pending,
                    Err(error) => {
                        let language = crate::gpui_shell::config::ui_language(cx);
                        this.plugins.feedback = Some((
                            None,
                            Err(language
                                .format(Message::SettingsPluginsFailed, &[("error", &error)])),
                        ));
                    },
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn plugin_community_actions(&self, cx: &Context<Self>) -> gpui::Div {
        let language = crate::gpui_shell::config::ui_language(cx);
        h_flex()
            .flex_wrap()
            .gap_1()
            .child(
                plugin_button("plugins-community-repository")
                    .ghost()
                    .icon(IconName::Github)
                    .label(language.text(Message::SettingsPluginsCommunityRepository))
                    .on_click(|_, _, cx| cx.open_url(community::REPOSITORY)),
            )
            .child(
                plugin_button("plugins-community-refresh")
                    .ghost()
                    .icon(IconName::Redo2)
                    .label(language.text(Message::SettingsPluginsRefresh))
                    .loading(self.plugins.busy == Some(Busy::CommunityCatalog))
                    .disabled(!self.plugins.idle())
                    .on_click(cx.listener(|this, _, _, cx| this.refresh_plugin_community(cx))),
            )
    }

    pub(super) fn plugin_community_section(&self, cx: &Context<Self>) -> gpui::Div {
        let language = crate::gpui_shell::config::ui_language(cx);
        let theme = cx.theme();
        let panel = v_flex().w_full().gap_2().when(
            self.plugins.busy == Some(Busy::CommunityPackage),
            |panel| {
                panel.child(plugin_notice(
                    IconName::Loader,
                    theme.muted_foreground,
                    language.text(Message::SettingsPluginsCommunityPreparing),
                    cx,
                ))
            },
        );
        match &self.plugins.community {
            None => panel.child(plugin_notice(
                IconName::Loader,
                theme.muted_foreground,
                language.text(Message::SettingsPluginsCommunityLoading),
                cx,
            )),
            Some(Err(error)) => panel.child(plugin_notice(
                IconName::TriangleAlert,
                theme.danger,
                language.format(Message::SettingsPluginsFailed, &[("error", error)]),
                cx,
            )),
            Some(Ok(entries)) if entries.is_empty() => panel.child(plugin_notice(
                IconName::Inbox,
                theme.muted_foreground,
                language.text(Message::SettingsPluginsCommunityEmpty),
                cx,
            )),
            Some(Ok(entries)) => {
                let listing =
                    self.plugins.listing.as_ref().and_then(|listing| listing.as_ref().ok());
                panel.child(h_flex().w_full().flex_wrap().items_stretch().gap_2().children(
                    entries.iter().map(|entry| {
                        let installed = listing.is_some_and(|listing| {
                            listing.plugins.iter().any(|plugin| {
                                plugin.id == entry.id && plugin.version == entry.version
                            })
                        });
                        self.community_plugin_card(entry, installed, language, cx)
                    }),
                ))
            },
        }
    }

    fn community_plugin_card(
        &self,
        entry: &Entry,
        installed: bool,
        language: UiLanguage,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let theme = cx.theme();
        let (name, description) = if matches!(language, UiLanguage::ZhCn | UiLanguage::ZhTw) {
            (&entry.name_zh, &entry.description_zh)
        } else {
            (&entry.name, &entry.description)
        };
        let plugin = entry.clone();
        let selector = format!("plugins-community-review-{}", entry.id);
        let action = if installed {
            Some(
                plugin_status(
                    language.text(Message::SettingsPluginsCommunityInstalled),
                    theme.success,
                )
                .into_any_element(),
            )
        } else if entry.supported() {
            Some(
                plugin_button(SharedString::from(selector.clone()))
                    .debug_selector(move || selector.clone())
                    .primary()
                    .outline()
                    .icon(IconName::ArrowRight)
                    .label(language.text(Message::SettingsPluginsCommunityReview))
                    .disabled(!self.plugins.idle())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.review_community_plugin(plugin.clone(), window, cx);
                    }))
                    .into_any_element(),
            )
        } else {
            None
        };
        let url = entry.source_url();
        let source = language.text(Message::SettingsPluginsCommunitySource);
        let card_selector = format!("plugins-community-card-{}", entry.id);
        design::settings_card(cx)
            .debug_selector(move || card_selector.clone())
            .flex_1()
            .w_auto()
            .flex_basis(px(315.0))
            .p_3()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(plugin_icon(puzzle_icon(), theme.link))
                    .child(
                        v_flex()
                            .min_w_0()
                            .flex_1()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .whitespace_normal()
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .whitespace_normal()
                                    .child(format!("{} · {}", entry.version, entry.id)),
                            ),
                    )
                    .child(
                        plugin_button(SharedString::from(format!("plugins-source-{}", entry.id)))
                            .ghost()
                            .w(px(32.0))
                            .icon(IconName::ExternalLink)
                            .aria_label(source)
                            .tooltip(source)
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .whitespace_normal()
                    .text_color(theme.muted_foreground)
                    .child(description.clone()),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .gap_1()
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .when(entry.windows_only, |metadata| {
                                metadata.child(plugin_status(
                                    language.text(Message::SettingsPluginsCommunityWindowsOnly),
                                    theme.muted_foreground,
                                ))
                            })
                            .when(entry.choose_executable, |metadata| {
                                metadata.child(plugin_status(
                                    language.text(Message::SettingsPluginsCommunityLocalExecutable),
                                    theme.muted_foreground,
                                ))
                            }),
                    )
                    .children(action),
            )
    }
}

fn stage(entry: &Entry, executable: Option<PathBuf>) -> Result<Pending, String> {
    let (package, request) =
        community::download(entry, executable.as_deref()).map_err(|error| error.to_string())?;
    Ok(Pending {
        path: package.path().to_owned(),
        request,
        community: Some((entry.source_url(), Arc::new(package))),
    })
}

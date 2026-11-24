//! Community catalog controls and ownership of the download-to-review handoff.

use std::path::PathBuf;
use std::sync::Arc;

use super::plugins::{Busy, Pending};
use super::*;
use crate::i18n::{Message, UiLanguage};
use crate::plugins::community::{self, Entry};

impl SettingsPane {
    pub(super) fn toggle_plugin_community(&mut self, cx: &mut Context<Self>) {
        if self.plugins.community_open {
            self.plugins.community_open = false;
        } else {
            self.plugins.community_open = true;
            if self.plugins.community.is_none() {
                self.refresh_plugin_community(cx);
            }
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

    pub(super) fn plugin_community_section(&self, cx: &mut Context<Self>) -> gpui::Div {
        let language = crate::gpui_shell::config::ui_language(cx);
        let idle = self.plugins.idle();
        let muted = cx.theme().muted_foreground;
        let mut panel = v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(crate::gpui_shell::theme::settings_hairline(cx))
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("plugins-community-repository")
                            .small()
                            .ghost()
                            .label(language.text(Message::SettingsPluginsCommunityRepository))
                            .on_click(|_, _, cx| cx.open_url(community::REPOSITORY)),
                    )
                    .child(
                        Button::new("plugins-community-refresh")
                            .small()
                            .ghost()
                            .label(language.text(Message::SettingsPluginsRefresh))
                            .loading(self.plugins.busy == Some(Busy::CommunityCatalog))
                            .disabled(!idle)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.refresh_plugin_community(cx)),
                            ),
                    ),
            );
        if self.plugins.busy == Some(Busy::CommunityPackage) {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(language.text(Message::SettingsPluginsCommunityPreparing)),
            );
        }
        match &self.plugins.community {
            None => panel.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(language.text(Message::SettingsPluginsCommunityLoading)),
            ),
            Some(Err(error)) => {
                panel.child(
                    div().text_sm().text_color(cx.theme().danger).whitespace_normal().child(
                        language.format(Message::SettingsPluginsFailed, &[("error", error)]),
                    ),
                )
            },
            Some(Ok(entries)) if entries.is_empty() => panel.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(language.text(Message::SettingsPluginsCommunityEmpty)),
            ),
            Some(Ok(entries)) => {
                for entry in entries {
                    let installed = self
                        .plugins
                        .listing
                        .as_ref()
                        .and_then(|listing| listing.as_ref().ok())
                        .is_some_and(|listing| {
                            listing.plugins.iter().any(|plugin| {
                                plugin.id == entry.id && plugin.version == entry.version
                            })
                        });
                    let supported = entry.supported();
                    let action = if installed {
                        Message::SettingsPluginsCommunityInstalled
                    } else if !supported {
                        Message::SettingsPluginsCommunityWindowsOnly
                    } else {
                        Message::SettingsPluginsCommunityReview
                    };
                    let (name, description) =
                        if matches!(language, UiLanguage::ZhCn | UiLanguage::ZhTw) {
                            (&entry.name_zh, &entry.description_zh)
                        } else {
                            (&entry.name, &entry.description)
                        };
                    let plugin = entry.clone();
                    let selector = format!("plugins-community-review-{}", entry.id);
                    panel = panel.child(
                        h_flex()
                            .gap_3()
                            .w_full()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child(format!("{} · {}", name, entry.version)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .whitespace_normal()
                                            .child(description.clone()),
                                    ),
                            )
                            .child(
                                Button::new(SharedString::from(selector.clone()))
                                    .debug_selector(move || selector.clone())
                                    .small()
                                    .outline()
                                    .flex_shrink_0()
                                    .label(language.text(action))
                                    .disabled(!idle || installed || !supported)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.review_community_plugin(plugin.clone(), window, cx);
                                    })),
                            ),
                    );
                }
                panel
            },
        }
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

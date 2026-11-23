//! Appearance controls for the five boundaries between sidebar rows.

use super::*;
use crate::i18n::Message;
use pebrel_settings::SidebarGap;

pub(super) struct GapEdit {
    gap: SidebarGap,
    input: Entity<InputState>,
    _subscription: Subscription,
}

impl SettingsPane {
    pub(super) fn sidebar_spacing_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = crate::gpui_shell::config::ui_language(cx);
        let group = self.group(language.text(Message::SettingsSidebarSpacingTitle), cx);
        SidebarGap::ALL.into_iter().fold(group, |group, gap| {
            let label = language.text(gap_message(gap));
            let value = self.runtime.sidebar_spacing.get(gap);
            let control =
                if let Some(edit) = self.sidebar_gap_edit.as_ref().filter(|edit| edit.gap == gap) {
                    div()
                        .w(px(120.0))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key == "escape" {
                                cx.stop_propagation();
                                this.finish_sidebar_gap_edit(false, window, cx);
                            }
                        }))
                        .child(Input::new(&edit.input).aria_label(label))
                        .into_any_element()
                } else {
                    let value = value.map_or_else(
                        || language.text(Message::CommonAuto).to_owned(),
                        |value| format!("{value} px"),
                    );
                    let key = gap.key();
                    Button::new(key)
                        .debug_selector(move || key.to_owned())
                        .ghost()
                        .w(px(120.0))
                        .label(value.clone())
                        .aria_label(format!("{label}, {value}"))
                        .tooltip(language.text(Message::SettingsSidebarSpacingEdit))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.begin_sidebar_gap_edit(gap, window, cx);
                        }))
                        .into_any_element()
                };
            group.child(self.row_with_reset(
                label,
                if gap == SidebarGap::Tabs {
                    language.text(Message::SettingsSidebarSpacingDescription)
                } else {
                    ""
                },
                value.is_some(),
                move |this, window, cx| this.save_sidebar_gap(gap, None, window, cx),
                control,
                cx,
            ))
        })
    }

    fn begin_sidebar_gap_edit(
        &mut self,
        gap: SidebarGap,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = crate::gpui_shell::config::ui_language(cx);
        let value = self
            .runtime
            .sidebar_spacing
            .get(gap)
            .map_or_else(String::new, |value| format!("{value}"));
        let input = cx
            .new(|cx| InputState::new(window, cx).placeholder(language.text(Message::CommonAuto)));
        let subscription =
            cx.subscribe_in(&input, window, |this, input, event: &InputEvent, window, cx| {
                if this.sidebar_gap_edit.as_ref().is_none_or(|edit| edit.input != *input) {
                    return;
                }
                match event {
                    InputEvent::PressEnter { .. } => this.finish_sidebar_gap_edit(true, window, cx),
                    InputEvent::Blur => this.finish_sidebar_gap_edit(false, window, cx),
                    _ => {},
                }
            });
        input.update(cx, |input, cx| {
            input.set_value(value.clone(), window, cx);
            input.focus(window, cx);
            input.set_selected_range(0..value.len(), cx);
        });
        self.sidebar_gap_edit = Some(GapEdit { gap, input, _subscription: subscription });
        cx.notify();
    }

    fn finish_sidebar_gap_edit(
        &mut self,
        apply: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.sidebar_gap_edit.as_ref() else { return };
        let gap = edit.gap;
        if apply {
            let value = edit.input.read(cx).value();
            let value = value.trim();
            let parsed = SidebarGap::parse(value);
            if parsed.is_none() && !value.is_empty() {
                let language = crate::gpui_shell::config::ui_language(cx);
                crate::gpui_shell::toast::toast(
                    window,
                    cx,
                    crate::gpui_shell::toast::ToastKind::Warning,
                    language.format(
                        Message::SettingsSidebarSpacingInvalid,
                        &[("max", &format!("{}", SidebarGap::MAX))],
                    ),
                );
                return;
            }
            self.save_sidebar_gap(gap, parsed, window, cx);
        }
        if let Some(edit) = self.sidebar_gap_edit.take()
            && edit.input.read(cx).focus_handle(cx).is_focused(window)
        {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    fn save_sidebar_gap(
        &mut self,
        gap: SidebarGap,
        value: Option<u8>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.runtime.sidebar_spacing.get(gap) == value {
            return;
        }
        if let Err(error) = self.try_persist(
            &[(gap.key(), value.map_or_else(String::new, |value| format!("{value}")))],
            cx,
        ) {
            let language = crate::gpui_shell::config::ui_language(cx);
            crate::gpui_shell::toast::toast(
                window,
                cx,
                crate::gpui_shell::toast::ToastKind::Warning,
                language.format(Message::SettingsSaveFailed, &[("error", &format!("{error}"))]),
            );
        }
    }
}

fn gap_message(gap: SidebarGap) -> Message {
    match gap {
        SidebarGap::Tabs => Message::SettingsSidebarSpacingTabs,
        SidebarGap::HeadingToTab => Message::SettingsSidebarSpacingHeadingToTab,
        SidebarGap::Headings => Message::SettingsSidebarSpacingHeadings,
        SidebarGap::TabToGroup => Message::SettingsSidebarSpacingTabToGroup,
        SidebarGap::BeforeUngrouped => Message::SettingsSidebarSpacingBeforeUngrouped,
    }
}

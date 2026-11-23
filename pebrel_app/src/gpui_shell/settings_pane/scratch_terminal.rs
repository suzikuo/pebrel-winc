use super::appearance::FontSizeTarget;
use super::*;
use crate::i18n::Message;
use pebrel_settings::{ScratchPlacement, ScratchTerminal};

impl SettingsPane {
    pub(super) fn scratch_terminal_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = crate::gpui_shell::config::ui_language(cx);
        let settings = self.runtime.scratch_terminal;
        let mut placement = h_flex().flex_wrap().gap_1();
        for (value, label) in ScratchPlacement::ALL.into_iter().zip([
            Message::ScratchPlacementDocked,
            Message::ScratchPlacementBottom,
            Message::ScratchPlacementRight,
        ]) {
            placement = placement.child(
                Button::new(value.settings_value())
                    .debug_selector(move || format!("scratch-placement-{}", value.settings_value()))
                    .small()
                    .outline()
                    .selected(settings.placement == value)
                    .label(language.text(label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.save_scratch_setting(
                            ScratchTerminal::PLACEMENT,
                            value.settings_value().into(),
                            window,
                            cx,
                        );
                    })),
            );
        }
        let mut sizes = h_flex().flex_wrap().gap_1();
        for size in ScratchTerminal::SIZES {
            sizes = sizes.child(
                Button::new(("scratch-size", size as usize))
                    .debug_selector(move || format!("scratch-size-{size}"))
                    .small()
                    .outline()
                    .selected(settings.size == size)
                    .label(format!("{size}%"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.save_scratch_setting(
                            ScratchTerminal::SIZE,
                            format!("{size}"),
                            window,
                            cx,
                        );
                    })),
            );
        }
        self.group(language.text(Message::ScratchTerminalTitle), cx)
            .child(self.maybe_marked(
                ScratchTerminal::PLACEMENT,
                language.text(Message::ScratchPlacement),
                "",
                placement,
                cx,
            ))
            .child(self.maybe_marked(
                ScratchTerminal::SIZE,
                language.text(Message::ScratchSize),
                "",
                sizes,
                cx,
            ))
            .child(self.font_size_row(FontSizeTarget::Scratch, cx))
            .child(self.switch_row(
                ScratchTerminal::INHERIT_DIRECTORY,
                language.text(Message::ScratchInheritDirectory),
                language.text(Message::ScratchInheritDirectoryHelp),
                settings.inherit_directory,
                cx,
            ))
            .child(self.switch_row(
                ScratchTerminal::ESCAPE_TO_HIDE,
                language.text(Message::ScratchEscape),
                language.text(Message::ScratchEscapeHelp),
                settings.escape_to_hide,
                cx,
            ))
    }

    pub(super) fn save_scratch_setting(
        &mut self,
        key: &'static str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.try_persist(&[(key, value)], cx) {
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

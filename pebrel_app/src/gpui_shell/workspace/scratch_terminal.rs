//! The workspace owns temporary terminal sessions independently of its split trees.

mod floating;
#[cfg(all(test, feature = "gpui-test-support"))]
mod native_tests;

use super::*;
use crate::gpui_shell::widgets::{TOOLBAR_BUTTON_SIZE, toolbar_button};
use crate::i18n::Message;
use crate::runtime_api::tabs::TabId;
use crate::session::LaunchSession;
use pebrel_settings::{ScratchPlacement, ScratchTerminal};
use std::collections::HashMap;

pub(super) const KEY_CONTEXT: &str = "ScratchTerminalEnabled";
pub(super) const ESCAPE_CONTEXT: &str = "ScratchTerminalEscape";
pub(super) const SHORTCUT: &str = "ctrl-alt-j";
gpui::actions!(scratch_terminal, [ToggleScratchTerminal, HideScratchTerminal]);

struct Session {
    pane: TerminalPane,
    number: u64,
}

#[derive(Default)]
struct Panel {
    sessions: Vec<Session>,
    selected: usize,
    next_number: u64,
    visible: bool,
    scroll: gpui::ScrollHandle,
    frames: [Option<floating::Frame>; 2],
}

impl Panel {
    fn active(&self) -> Option<&TerminalPane> {
        self.sessions.get(self.selected).map(|session| &session.pane)
    }

    fn push(&mut self, pane: TerminalPane) {
        self.next_number += 1;
        self.selected = self.sessions.len();
        self.sessions.push(Session { pane, number: self.next_number });
        self.scroll.scroll_to_item(self.selected);
        self.visible = true;
    }

    fn remove(&mut self, index: usize) -> Session {
        let session = self.sessions.remove(index);
        if self.selected > index {
            self.selected -= 1;
        } else if self.selected == index {
            self.selected = index.min(self.sessions.len().saturating_sub(1));
        }
        self.scroll.scroll_to_item(self.selected);
        session
    }
}

pub(super) struct State {
    pub(super) settings: ScratchTerminal,
    available: bool,
    panels: HashMap<TabId, Panel>,
    floating: floating::Floating,
}

impl State {
    pub(super) fn new(settings: ScratchTerminal) -> Self {
        Self { settings, available: false, panels: HashMap::new(), floating: Default::default() }
    }

    pub(super) fn set_available(&mut self, available: bool) {
        self.available = available;
        if !available {
            for panel in self.panels.values_mut() {
                panel.visible = false;
            }
        }
    }

    pub(super) fn busy_process(&self, tab_id: &TabId, cx: &App) -> Option<String> {
        self.panels
            .get(tab_id)?
            .sessions
            .iter()
            .find_map(|session| session.pane.view.read(cx).busy_process())
    }

    pub(super) fn apply_settings(&self, cx: &mut App) {
        for session in self.panels.values().flat_map(|panel| &panel.sessions) {
            session.pane.view.update(cx, |view, cx| view.apply_settings(cx));
        }
    }
}

impl NebulaWorkspace {
    fn scratch_owner(&self) -> Option<&TabId> {
        (!self.settings_open).then_some(())?;
        Some(&self.tab_meta.get(self.active)?.runtime_id)
    }

    fn scratch_origin(&self, cx: &App) -> Option<&Entity<TerminalView>> {
        let view = self.tabs.get(self.active)?.focused_view()?;
        let terminal = view.read(cx);
        if matches!(
            terminal.session_launch,
            LaunchSession::Ssh { .. } | LaunchSession::Serial { .. }
        ) || terminal
            .exec_context
            .as_ref()
            .is_some_and(|context| matches!(context.wsl_distribution(), Some(None)))
        {
            return None;
        }
        Some(view)
    }

    pub(super) fn scratch_terminal_enabled(&self, cx: &App) -> bool {
        self.scratch.available
            && self.scratch_owner().is_some_and(|id| {
                self.scratch.panels.get(id).and_then(Panel::active).is_some()
                    || self.scratch_origin(cx).is_some()
            })
    }

    fn visible_scratch(&self) -> Option<&Panel> {
        let panel = self.scratch.panels.get(self.scratch_owner()?)?;
        (self.scratch.available && panel.visible).then_some(panel)
    }

    pub(super) fn focused_scratch_view(
        &self,
        window: &Window,
        cx: &App,
    ) -> Option<&Entity<TerminalView>> {
        let view = &self.visible_scratch()?.active()?.view;
        view.read(cx).focus_handle(cx).is_focused(window).then_some(view)
    }

    pub(super) fn sync_scratch_terminals(&mut self, cx: &mut Context<Self>) {
        self.discard_stale_scratch_float_drag();
        if self.scratch.panels.is_empty() {
            return;
        }
        let active = self.scratch_owner().cloned().filter(|_| !self.window_hidden);
        self.scratch.panels.retain(|id, panel| {
            if !self.tab_meta.iter().any(|meta| &meta.runtime_id == id) {
                for session in &panel.sessions {
                    session.pane.view.read(cx).shutdown();
                }
                return false;
            }
            for (index, session) in panel.sessions.iter().enumerate() {
                let visible =
                    panel.visible && active.as_ref() == Some(id) && index == panel.selected;
                session.pane.view.update(cx, |view, cx| view.set_output_visible(visible, cx));
            }
            true
        });
    }

    pub(super) fn toggle_scratch_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.scratch_terminal_enabled(cx) {
            cx.propagate();
            return;
        }
        if self.visible_scratch().is_some() {
            self.hide_scratch_terminal(window, cx);
            return;
        }
        let Some(id) = self.scratch_owner().cloned() else { return };
        if let Some(panel) =
            self.scratch.panels.get_mut(&id).filter(|panel| panel.active().is_some())
        {
            panel.visible = true;
            if let Some(pane) = panel.active() {
                focus_view(&pane.view, window, cx);
            }
            cx.notify();
        } else {
            self.add_scratch_terminal(window, cx);
        }
    }

    fn add_scratch_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.scratch.available {
            return;
        }
        let Some(id) = self.scratch_owner().cloned() else { return };
        let Some(origin) = self.scratch_origin(cx) else { return };
        let origin = origin.read(cx);
        let grid = (origin.grid_cols() as u16, origin.grid_rows() as u16);
        let launch = scratch_launch(origin, self.scratch.settings.inherit_directory);
        let pane = self.new_pane(grid, launch, None, window, cx);
        pane.view.update(cx, |view, cx| view.use_scratch_font_size(cx));
        focus_view(&pane.view, window, cx);
        self.scratch.panels.entry(id).or_default().push(pane);
        cx.notify();
    }

    fn select_scratch_terminal(
        &mut self,
        pane_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.scratch_owner().cloned() else { return };
        let Some(panel) = self.scratch.panels.get_mut(&id).filter(|panel| panel.visible) else {
            return;
        };
        let Some(index) = panel.sessions.iter().position(|session| session.pane.id == pane_id)
        else {
            return;
        };
        panel.selected = index;
        focus_view(&panel.sessions[index].pane.view, window, cx);
        cx.notify();
    }

    pub(super) fn hide_scratch_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.scratch_owner().cloned() else { return };
        if let Some(panel) = self.scratch.panels.get_mut(&id) {
            panel.visible = false;
            self.focus_active(window, cx);
            cx.notify();
        }
    }

    fn end_scratch_terminals(
        &mut self,
        id: &TabId,
        panes: &[u64],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_active = self.scratch_owner() == Some(id);
        let Some(panel) = self.scratch.panels.get_mut(id) else { return };
        let move_focus = was_active
            && panel.visible
            && panel.active().is_some_and(|pane| panes.contains(&pane.id));
        for pane in panes {
            if let Some(index) = panel.sessions.iter().position(|session| session.pane.id == *pane)
            {
                panel.remove(index).pane.view.read(cx).shutdown();
            }
        }
        let next = panel.active().map(|pane| pane.view.clone());
        if next.is_none() {
            panel.visible = false;
        }
        if move_focus {
            if let Some(view) = next {
                focus_view(&view, window, cx);
            } else {
                self.focus_active(window, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn request_end_scratch_terminal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_end_scratch_sessions(false, window, cx);
    }

    fn request_end_scratch_sessions(
        &mut self,
        all: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(id), Some(panel)) = (self.scratch_owner().cloned(), self.visible_scratch())
        else {
            return;
        };
        let sessions = if all {
            panel.sessions.as_slice()
        } else {
            let Some(session) = panel.sessions.get(panel.selected) else { return };
            std::slice::from_ref(session)
        };
        let panes: Vec<_> = sessions.iter().map(|session| session.pane.id).collect();
        let Some(process) =
            sessions.iter().find_map(|session| session.pane.view.read(cx).busy_process())
        else {
            self.end_scratch_terminals(&id, &panes, window, cx);
            return;
        };
        let language = crate::gpui_shell::config::ui_language(cx);
        let (title, confirm, body) = if panes.len() > 1 {
            (
                Message::ScratchTerminalEndAll,
                Message::ScratchTerminalEndAll,
                language.text(Message::ScratchTerminalEndAllRunning).to_owned(),
            )
        } else {
            (
                Message::ScratchTerminalEnd,
                Message::CommonClose,
                language.format(Message::WorkspaceCloseRunningProcess, &[("process", &process)]),
            )
        };
        let workspace = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let workspace = workspace.clone();
            let id = id.clone();
            let panes = panes.clone();
            confirm_dialog(
                dialog,
                window,
                language.text(title),
                body.clone(),
                language.text(confirm),
                language.text(Message::CommonCancel),
                ButtonVariant::Danger,
            )
            .on_ok(move |_, window, cx| {
                // Restore the dialog's prior focus before removing its panes.
                // The default animated close would restore it after ours.
                window.close_dialog(cx);
                let _ = workspace
                    .update(cx, |this, cx| this.end_scratch_terminals(&id, &panes, window, cx));
                false
            })
        });
    }

    pub(super) fn handle_scratch_event(
        &mut self,
        view: &Entity<TerminalView>,
        event: &TerminalViewEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((id, pane_id)) = self.scratch.panels.iter().find_map(|(id, panel)| {
            panel
                .sessions
                .iter()
                .find(|session| session.pane.view.entity_id() == view.entity_id())
                .map(|session| (id, session.pane.id))
        }) else {
            return false;
        };
        match event {
            TerminalViewEvent::Exited if !view.read(cx).local_session_failed() => {
                let id = id.clone();
                self.end_scratch_terminals(&id, &[pane_id], window, cx);
            },
            TerminalViewEvent::FocusRequested
                if self
                    .visible_scratch()
                    .and_then(Panel::active)
                    .is_some_and(|pane| pane.id == pane_id) =>
            {
                focus_view(view, window, cx);
            },
            TerminalViewEvent::ChooseShell => {
                self.hide_scratch_terminal(window, cx);
                self.open_shell_palette(window, cx);
            },
            _ => {},
        }
        true
    }

    pub(super) fn scratch_toolbar_width(&self) -> f32 {
        if self.scratch.available {
            TOOLBAR_BUTTON_SIZE + window_titlebar::TITLE_BAR_CONTROL_GAP
        } else {
            0.0
        }
    }

    pub(super) fn scratch_button(&self, cx: &mut Context<Self>) -> Option<Button> {
        self.scratch.available.then(|| {
            let language = crate::gpui_shell::config::ui_language(cx);
            toolbar_button(
                "scratch-terminal-toggle",
                Icon::new(Icon::empty()).path(crate::gpui_shell::assets::nav::TERMINAL),
                !self.scratch_terminal_enabled(cx),
            )
            .selected(self.visible_scratch().is_some())
            .debug_selector(|| "scratch-terminal-toggle".into())
            .aria_label(language.text(Message::ScratchTerminalTitle))
            .tooltip(language.text(Message::ScratchTerminalShortcut))
            .on_click(cx.listener(|this, _, window, cx| this.toggle_scratch_terminal(window, cx)))
        })
    }

    pub(super) fn render_with_scratch(
        &self,
        content: gpui::AnyElement,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(scratch) = self.visible_scratch() else { return content };
        let Some(pane) = scratch.active() else { return content };
        let language = crate::gpui_shell::config::ui_language(cx);
        let mode = floating::Mode::from_placement(self.scratch.settings.placement);
        let ratio = relative(f32::from(self.scratch.settings.size) / 100.0);
        let panel = div()
            .id("scratch-terminal-panel")
            .debug_selector(|| "scratch-terminal-panel".into())
            .when(self.scratch.settings.escape_to_hide, |panel| panel.key_context(ESCAPE_CONTEXT))
            .flex()
            .flex_col()
            .flex_shrink_0()
            .overflow_hidden()
            .occlude()
            .bg(cx.theme().background)
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(36.0))
                    .flex_shrink_0()
                    .px_2()
                    .bg(cx.theme().title_bar)
                    .child(self.scratch_session_tabs(scratch, mode.is_some(), cx))
                    .child(
                        toolbar_button(
                            "scratch-terminal-add",
                            IconName::Plus,
                            self.scratch_origin(cx).is_none(),
                        )
                        .debug_selector(|| "scratch-terminal-add".into())
                        .aria_label(language.text(Message::ScratchTerminalAdd))
                        .tooltip(language.text(Message::ScratchTerminalAdd))
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.add_scratch_terminal(window, cx)
                            }),
                        ),
                    )
                    .child(
                        toolbar_button("scratch-terminal-hide", IconName::Minus, false)
                            .debug_selector(|| "scratch-terminal-hide".into())
                            .aria_label(language.text(Message::ScratchTerminalHide))
                            .tooltip(language.text(Message::ScratchTerminalHide))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.hide_scratch_terminal(window, cx)
                            })),
                    )
                    .child(
                        toolbar_button("scratch-terminal-end", IconName::Close, false)
                            .debug_selector(|| "scratch-terminal-end".into())
                            .aria_label(language.text(Message::ScratchTerminalEnd))
                            .tooltip(language.text(Message::ScratchTerminalEnd))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.request_end_scratch_terminal(window, cx)
                            })),
                    )
                    .when(scratch.sessions.len() > 1, |header| {
                        let label = language.text(Message::ScratchTerminalEndAll);
                        header.child(
                            toolbar_button("scratch-terminal-end-all", IconName::CircleX, false)
                                .debug_selector(|| "scratch-terminal-end-all".into())
                                .aria_label(label)
                                .tooltip(label)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.request_end_scratch_sessions(true, window, cx)
                                })),
                        )
                    }),
            )
            .child(
                div().flex_1().min_h_0().w_full().child(terminal_activity::terminal_surface(pane)),
            );
        if let Some(mode) = mode {
            self.render_scratch_float(mode, scratch, content, panel, cx)
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().flex_1().min_h_0().w_full().child(content))
                .child(panel.h(ratio).w_full().border_t_1())
                .into_any_element()
        }
    }
    fn scratch_session_tabs(
        &self,
        panel: &Panel,
        floating: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = crate::gpui_shell::config::ui_language(cx);
        div()
            .id("scratch-terminal-tabs")
            .debug_selector(|| "scratch-terminal-tabs".into())
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .overflow_x_scroll()
            .track_scroll(&panel.scroll)
            .gap_1()
            .when(floating, |title| self.scratch_float_title(title, cx))
            .children(panel.sessions.iter().enumerate().map(|(index, session)| {
                let pane_id = session.pane.id;
                let number = session.number;
                let name = language
                    .format(Message::ScratchTerminalSession, &[("number", &number.to_string())]);
                Button::new(("scratch-session", pane_id as usize))
                    .debug_selector(move || format!("scratch-terminal-session-{number}").into())
                    .small()
                    .ghost()
                    .selected(index == panel.selected)
                    .when(floating, |tab| tab.cursor_grab())
                    .label(name)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_scratch_terminal(pane_id, window, cx);
                    }))
            }))
    }
}

fn focus_view(view: &Entity<TerminalView>, window: &mut Window, cx: &mut App) {
    let focus = view.read(cx).focus_handle(cx);
    window.defer(cx, move |window, cx| window.focus(&focus, cx));
}

fn scratch_launch(
    origin: &TerminalView,
    inherit_directory: bool,
) -> crate::gpui_shell::terminal::view::TerminalLaunch {
    use crate::gpui_shell::terminal::view::TerminalLaunch;
    use pebrel_terminal::tty::Shell;

    if let Some(distro) = origin.wsl_distro() {
        let mut args = vec!["--distribution".into(), distro.to_owned()];
        if let Some(user) = origin.exec_context.as_ref().and_then(|context| context.wsl_user()) {
            args.extend(["--user".into(), user.to_owned()]);
        }
        if inherit_directory && let Some(path) = crate::shell_detect::wsl_guest_cwd(&origin.cwd) {
            args.extend(["--cd".into(), path.to_owned()]);
        } else {
            args.extend(["--cd".into(), "~".into()]);
        }
        return TerminalLaunch::Local {
            cwd: None,
            shell: Some(Shell::new("wsl.exe".into(), args)),
            shell_name: Some("WSL".into()),
        };
    }
    let cwd = inherit_directory
        .then(|| origin.working_directory().map(std::path::PathBuf::from))
        .flatten();
    let shell = crate::shell_detect::resolve_id(&crate::platform::shell::default_shell_id());
    TerminalLaunch::Local { cwd, shell: shell.map(|shell| shell.shell()), shell_name: None }
}

//! Drag one live pane from its header. The same drop intent drives preview and release.

use gpui::{
    Context, InteractiveElement as _, IntoElement, MouseButton, MouseMoveEvent, ParentElement as _,
    Pixels, Point, Styled as _, Window, canvas, div, px,
};

use crate::gpui_shell::prelude::*;
use crate::i18n::Message;

use super::super::{
    NebulaWorkspace, TabMeta, WorkspaceTab,
    tab_drag::outside_window,
    windowing::{self, CrossWindowDropDestination, CrossWindowTerminalDrag},
};
use super::drag_crossed_threshold;

pub(in super::super) struct PaneDrag {
    pub(super) source: CrossWindowTerminalDrag,
    press: Point<Pixels>,
    position: Point<Pixels>,
    pub(super) active: bool,
    force_new_window: bool,
    pub(super) target: PaneDrop,
}

#[derive(Debug, Default, PartialEq)]
pub(super) enum PaneDrop {
    #[default]
    None,
    Swap(u64),
    NewTab,
    NewWindow,
    Dock(CrossWindowDropDestination),
}

impl PaneDrop {
    fn preview_window(&self) -> Option<u64> {
        match self {
            Self::Dock(destination) => Some(destination.window_id),
            _ => None,
        }
    }
}

impl NebulaWorkspace {
    pub(in super::super) fn begin_pane_drag(
        &mut self,
        tab_ix: usize,
        pane_id: u64,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(source) = self.cross_window_pane_payload(tab_ix, pane_id, cx) else { return };
        self.cancel_pane_drag(cx);
        self.pane_drag = Some(PaneDrag {
            source,
            press: position,
            position,
            active: false,
            force_new_window: false,
            target: PaneDrop::None,
        });
        cx.notify();
    }

    fn update_pane_drag(
        &mut self,
        event: &MouseMoveEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(drag) = self.pane_drag.as_ref() else { return false };
        // Win32 non-client moves can arrive with no button flag during capture.
        if !drag.active && event.pressed_button != Some(MouseButton::Left) {
            self.cancel_pane_drag(cx);
            return false;
        }
        self.update_pane_drag_at(event.position, event.modifiers.alt, window, cx);
        self.pane_drag.is_some()
    }

    fn update_pane_drag_at(
        &mut self,
        position: Point<Pixels>,
        alt: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.pane_drag.as_ref() else { return };
        if self.settings_open || drag.source.source_tab_index(self) != Some(self.active) {
            self.cancel_pane_drag(cx);
            return;
        }
        let force_new_window = drag.force_new_window || alt;
        let delta = position - drag.press;
        let active = drag.active || drag_crossed_threshold(f32::from(delta.x), f32::from(delta.y));
        let target = if active {
            self.pane_drop_at(drag, position, force_new_window, window, cx)
        } else {
            PaneDrop::None
        };
        let drag = self.pane_drag.as_mut().expect("drag checked above");
        drag.position = position;
        drag.active = active;
        drag.force_new_window = force_new_window;
        drag.target = target;
        if active {
            cx.notify();
        }
    }

    fn pane_drop_at(
        &self,
        drag: &PaneDrag,
        position: Point<Pixels>,
        force_new_window: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> PaneDrop {
        let previous = drag.target.preview_window();
        if force_new_window {
            windowing::clear_cross_window_drag_target(previous, cx);
            return PaneDrop::NewWindow;
        }
        if let Some(destination) =
            windowing::update_cross_window_drag_target(&drag.source, previous, cx)
        {
            return if destination.dock.is_some() {
                PaneDrop::Dock(destination)
            } else {
                PaneDrop::None
            };
        }
        if outside_window(position, window) {
            return PaneDrop::NewWindow;
        }
        let (x, y) = (f32::from(position.x), f32::from(position.y));
        if self.active_terminal_area().is_some_and(|area| !area.contains(x, y)) {
            PaneDrop::NewTab
        } else if let Some(target) = self.pane_at(x, y).filter(|id| *id != drag.source.pane_id) {
            PaneDrop::Swap(target)
        } else {
            PaneDrop::None
        }
    }

    /// A header occludes workspace hitboxes. Window capture also sees releases
    /// outside the viewport while the source window holds native pointer capture.
    pub(in super::super) fn pane_drag_capture(&self, cx: &Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let view = owner.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase.capture()
                        && view
                            .update(cx, |this, cx| this.update_pane_drag(event, window, cx))
                            .unwrap_or(false)
                    {
                        cx.stop_propagation();
                    }
                });
                let view = owner.clone();
                window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, window, cx| {
                    if phase.capture()
                        && event.button == MouseButton::Left
                        && view
                            .update(cx, |this, cx| this.release_pane_drag(event, window, cx))
                            .unwrap_or(false)
                    {
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .inset_0()
    }

    fn release_pane_drag(
        &mut self,
        event: &gpui::MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // Recompute from mouse-up. A fast drag can release before its preview paints.
        self.update_pane_drag_at(event.position, event.modifiers.alt, window, cx);
        let Some(drag) = self.pane_drag.take() else { return false };
        if !drag.active {
            return false;
        }
        windowing::clear_cross_window_drag_target(drag.target.preview_window(), cx);
        let pane_id = drag.source.pane_id;
        match drag.target {
            PaneDrop::None => {},
            PaneDrop::Swap(target) => {
                if let Some(WorkspaceTab::Terminal { tree, .. }) = self.tabs.get_mut(self.active)
                    && tree.swap_leaves(pane_id, target)
                {
                    self.mark_structural_resize(self.active, cx);
                    self.focus_active(window, cx);
                }
            },
            PaneDrop::NewTab => {
                if let Some(tab) = self.detach_split_pane(self.active, pane_id, cx) {
                    let at = self.active + 1;
                    self.insert_tab_at(at, tab, TabMeta::default());
                    self.active = at;
                    self.mark_structural_resize(at, cx);
                    self.focus_active(window, cx);
                    self.sync_side_panel_to_active(true, cx);
                    self.reveal_active_tab();
                }
            },
            PaneDrop::NewWindow => {
                cx.defer(move |cx| windowing::move_terminal_to_new_window(drag.source, cx));
            },
            PaneDrop::Dock(destination) => {
                cx.defer(move |cx| {
                    windowing::drop_terminal_to_existing_window(drag.source, destination, cx);
                });
            },
        }
        cx.notify();
        true
    }

    pub(in super::super) fn cancel_pane_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.pane_drag.take() else { return false };
        windowing::clear_cross_window_drag_target(drag.target.preview_window(), cx);
        cx.notify();
        true
    }

    /// Keep the terminal entity and session alive while collapsing its old split.
    pub(in super::super) fn detach_split_pane(
        &mut self,
        tab_ix: usize,
        pane_id: u64,
        cx: &mut Context<Self>,
    ) -> Option<WorkspaceTab> {
        let WorkspaceTab::Terminal { panes, tree, focused, zoomed, broadcast } =
            self.tabs.get_mut(tab_ix)?
        else {
            return None;
        };
        if panes.len() < 2 {
            return None;
        }
        let index = panes.iter().position(|pane| pane.id == pane_id)?;
        let pebrel_split::RemoveOutcome::Collapsed(next_focus) = tree.remove_leaf(pane_id) else {
            return None;
        };
        let pane = panes.remove(index);
        if *focused == pane_id {
            *focused = next_focus;
        }
        *zoomed = false;
        if panes.len() == 1 {
            *broadcast = false;
        }
        self.pane_bounds.borrow_mut().remove(&pane_id);
        self.split_bounds.borrow_mut().clear();
        self.mark_structural_resize(tab_ix, cx);
        Some(WorkspaceTab::Terminal {
            tree: pebrel_split::SplitTree::leaf(pane_id),
            panes: vec![pane],
            focused: pane_id,
            zoomed: false,
            broadcast: false,
        })
    }

    pub(in super::super) fn pane_drag_overlay(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let drag = self.pane_drag.as_ref().filter(|drag| drag.active)?;
        let theme = cx.theme();
        let language = crate::gpui_shell::config::ui_language(cx);
        let (message, accent) = match drag.target {
            PaneDrop::None => (Message::WorkspacePaneExtractDragHint, false),
            PaneDrop::Swap(_) => (Message::WorkspacePaneSwapRelease, true),
            PaneDrop::NewTab => (Message::WorkspacePaneExtractRelease, true),
            PaneDrop::NewWindow => (Message::WorkspacePaneWindowRelease, true),
            PaneDrop::Dock(_) => (Message::WorkspacePaneDockRelease, true),
        };
        let viewport = window.viewport_size();
        let max_width = (viewport.width - px(16.0)).clamp(px(1.0), px(320.0));
        let left = (drag.position.x + px(12.0))
            .clamp(px(8.0), (viewport.width - max_width - px(8.0)).max(px(8.0)));
        let top =
            (drag.position.y + px(12.0)).clamp(px(8.0), (viewport.height - px(48.0)).max(px(8.0)));
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .cursor_grab()
                .child(
                    div()
                        .absolute()
                        .left(left)
                        .top(top)
                        .max_w(max_width)
                        .px_2()
                        .py(px(3.0))
                        .rounded(px(6.0))
                        .bg(if accent { theme.primary } else { theme.muted })
                        .text_size(px(11.0))
                        .text_color(if accent {
                            theme.primary_foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(language.text(message)),
                )
                .into_any_element(),
        )
    }
}

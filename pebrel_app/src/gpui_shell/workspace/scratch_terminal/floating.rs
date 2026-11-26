//! Floating geometry is relative to the terminal card and kept with its tab.

use super::*;
use gpui::{CursorStyle, Point, Size};
use std::cell::Cell;

const MIN_WIDTH: f32 = 248.0;
const MIN_HEIGHT: f32 = 140.0;
const EDGE: f32 = 7.0;
const CORNER: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Bottom,
    Right,
}

impl Mode {
    pub(super) fn from_placement(placement: ScratchPlacement) -> Option<Self> {
        match placement {
            ScratchPlacement::Docked => None,
            ScratchPlacement::FloatingBottom => Some(Self::Bottom),
            ScratchPlacement::FloatingRight => Some(Self::Right),
        }
    }

    fn default_frame(self, ratio: f32) -> Frame {
        match self {
            Self::Bottom => Frame { left: 0., top: 1. - ratio, right: 1., bottom: 1. },
            Self::Right => Frame { left: 1. - ratio, top: 0., right: 1., bottom: 1. },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Frame {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Start,
    End,
    Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gesture {
    Move,
    Resize(Edge, Edge),
}

impl Gesture {
    fn cursor(self) -> CursorStyle {
        use CursorStyle::*;
        use Edge::*;
        match self {
            Self::Move => ClosedHand,
            Self::Resize(Fixed, _) => ResizeUpDown,
            Self::Resize(_, Fixed) => ResizeLeftRight,
            Self::Resize(Start, Start) | Self::Resize(End, End) => ResizeUpLeftDownRight,
            Self::Resize(_, _) => ResizeUpRightDownLeft,
        }
    }
}

impl Frame {
    fn adjusted(self, gesture: Gesture, delta: Point<Pixels>, size: Size<Pixels>) -> Self {
        let width = f32::from(size.width).max(1.0);
        let height = f32::from(size.height).max(1.0);
        let dx = f32::from(delta.x) / width;
        let dy = f32::from(delta.y) / height;
        match gesture {
            Gesture::Move => {
                let dx = dx.clamp(-self.left, 1.0 - self.right);
                let dy = dy.clamp(-self.top, 1.0 - self.bottom);
                Self {
                    left: self.left + dx,
                    right: self.right + dx,
                    top: self.top + dy,
                    bottom: self.bottom + dy,
                }
            },
            Gesture::Resize(x, y) => {
                let (left, right) = resize_axis(self.left, self.right, dx, MIN_WIDTH / width, x);
                let (top, bottom) = resize_axis(self.top, self.bottom, dy, MIN_HEIGHT / height, y);
                Self { left, top, right, bottom }
            },
        }
    }
}

fn resize_axis(start: f32, end: f32, delta: f32, minimum: f32, edge: Edge) -> (f32, f32) {
    let minimum = minimum.min(end - start);
    match edge {
        Edge::Start => ((start + delta).clamp(0.0, end - minimum), end),
        Edge::End => (start, (end + delta).clamp(start + minimum, 1.0)),
        Edge::Fixed => (start, end),
    }
}

struct Drag {
    owner: TabId,
    mode: Mode,
    gesture: Gesture,
    start: Point<Pixels>,
    size: Size<Pixels>,
    original: Frame,
    preview: Frame,
    active: bool,
    reset_on_click: bool,
}

impl Drag {
    fn activate_at(&mut self, position: Point<Pixels>) -> bool {
        let delta = position - self.start;
        let threshold = px(super::super::TAB_DRAG_THRESHOLD);
        self.active |= delta.x.abs() >= threshold || delta.y.abs() >= threshold;
        self.active
    }
}

#[derive(Default)]
pub(super) struct Floating {
    bounds: Rc<Cell<Size<Pixels>>>,
    drag: Option<Drag>,
}

impl NebulaWorkspace {
    fn scratch_float_mode(&self) -> Option<Mode> {
        Mode::from_placement(self.scratch.settings.placement)
    }

    pub(in crate::gpui_shell::workspace) fn scratch_float_visible(&self) -> bool {
        self.scratch_float_mode().is_some() && self.visible_scratch().is_some()
    }

    pub(in crate::gpui_shell::workspace) fn has_scratch_float_gesture(&self) -> bool {
        self.scratch.floating.drag.is_some()
    }

    pub(in crate::gpui_shell::workspace) fn cancel_scratch_float_drag(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.scratch.floating.drag.take().is_none() {
            return false;
        }
        cx.notify();
        true
    }

    pub(super) fn discard_stale_scratch_float_drag(&mut self) {
        if self.scratch.floating.drag.as_ref().is_some_and(|drag| {
            self.scratch_owner() != Some(&drag.owner)
                || self.visible_scratch().is_none()
                || self.scratch_float_mode() != Some(drag.mode)
        }) {
            self.scratch.floating.drag = None;
        }
    }

    // The drag overlay occludes the workspace root's hover hitbox, so the
    // window capture listener delivers moves both inside and outside it.
    pub(in crate::gpui_shell::workspace) fn move_scratch_float_drag(
        &mut self,
        event: &gpui::MouseMoveEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = &mut self.scratch.floating.drag else { return };
        if event.pressed_button == Some(MouseButton::Left) {
            let was_active = drag.active;
            if drag.activate_at(event.position) {
                let frame =
                    drag.original.adjusted(drag.gesture, event.position - drag.start, drag.size);
                if !was_active || drag.preview != frame {
                    drag.preview = frame;
                    cx.notify();
                }
            }
        }
        if drag.active {
            cx.stop_propagation();
        }
    }

    pub(in crate::gpui_shell::workspace) fn finish_scratch_float_drag(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.discard_stale_scratch_float_drag();
        let Some(mut drag) = self.scratch.floating.drag.take() else { return false };
        let moved = drag.activate_at(position);
        if let Some(panel) = self.scratch.panels.get_mut(&drag.owner) {
            if moved {
                let frame = drag.original.adjusted(drag.gesture, position - drag.start, drag.size);
                if frame != drag.original {
                    panel.frames[drag.mode as usize] = Some(frame);
                }
            } else if drag.reset_on_click {
                panel.frames[drag.mode as usize] = None;
            }
        }
        if moved || drag.reset_on_click {
            cx.notify();
        }
        moved
    }

    fn start_scratch_float_drag(
        &mut self,
        gesture: Gesture,
        event: &gpui::MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        let (Some(mode), Some(owner), Some(panel)) =
            (self.scratch_float_mode(), self.scratch_owner(), self.visible_scratch())
        else {
            return;
        };
        let size = self.scratch.floating.bounds.get();
        if size.width <= px(0.) || size.height <= px(0.) {
            return;
        }
        let active = gesture != Gesture::Move;
        let original = self.scratch_float_frame(mode, panel);
        self.scratch.floating.drag = Some(Drag {
            owner: owner.clone(),
            mode,
            gesture,
            start: event.position,
            size,
            original,
            preview: original,
            active,
            reset_on_click: !active && event.click_count == 2,
        });
        gpui_component::GlobalState::suppress_text_selection(cx);
        if active {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn scratch_float_frame(&self, mode: Mode, panel: &Panel) -> Frame {
        self.scratch
            .floating
            .drag
            .as_ref()
            .filter(|drag| Some(&drag.owner) == self.scratch_owner() && mode == drag.mode)
            .map(|drag| drag.preview)
            .or(panel.frames[mode as usize])
            .unwrap_or_else(|| mode.default_frame(f32::from(self.scratch.settings.size) / 100.0))
    }

    pub(super) fn render_scratch_float(
        &self,
        mode: Mode,
        scratch: &Panel,
        content: gpui::AnyElement,
        panel: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let frame = self.scratch_float_frame(mode, scratch);
        let bounds = self.scratch.floating.bounds.clone();
        div()
            .size_full()
            .relative()
            .child(content)
            .child(
                div()
                    .absolute()
                    .inset_2()
                    .child(
                        gpui::canvas(
                            move |bounds_in_window, _, _| bounds.set(bounds_in_window.size),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .child(
                        panel
                            .absolute()
                            .left(relative(frame.left))
                            .top(relative(frame.top))
                            .w(relative(frame.right - frame.left))
                            .h(relative(frame.bottom - frame.top))
                            .rounded_lg()
                            .border_1()
                            .shadow_md()
                            .children(resize_handles(cx)),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn scratch_float_title(
        &self,
        title: gpui::Stateful<gpui::Div>,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let text = crate::gpui_shell::config::ui_language(cx).text(Message::ScratchTerminalDrag);
        title
            .h_full()
            .aria_label(text)
            .tooltip(move |window, cx| Tooltip::new(text).build(window, cx))
            .cursor_grab()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event, _, cx| {
                    this.start_scratch_float_drag(Gesture::Move, event, cx);
                }),
            )
    }

    pub(in crate::gpui_shell::workspace) fn scratch_float_drag_overlay(&self) -> Option<gpui::Div> {
        let drag = self.scratch.floating.drag.as_ref().filter(|drag| drag.active)?;
        Some(div().absolute().inset_0().occlude().cursor(drag.gesture.cursor()))
    }
}

fn resize_handles(
    cx: &Context<NebulaWorkspace>,
) -> impl Iterator<Item = gpui::Stateful<gpui::Div>> {
    use Edge::*;
    [
        ("scratch-resize-left", Start, Fixed),
        ("scratch-resize-right", End, Fixed),
        ("scratch-resize-top", Fixed, Start),
        ("scratch-resize-bottom", Fixed, End),
        ("scratch-resize-top-left", Start, Start),
        ("scratch-resize-top-right", End, Start),
        ("scratch-resize-bottom-left", Start, End),
        ("scratch-resize-bottom-right", End, End),
    ]
    .map(|(id, x, y)| {
        let gesture = Gesture::Resize(x, y);
        let handle = div()
            .id(id)
            .debug_selector(move || id.into())
            .absolute()
            .occlude()
            .cursor(gesture.cursor());
        let handle = match x {
            Start => handle.left_0().w(px(if y == Fixed { EDGE } else { CORNER })),
            End => handle.right_0().w(px(if y == Fixed { EDGE } else { CORNER })),
            Fixed => handle.left(px(CORNER)).right(px(CORNER)),
        };
        match y {
            Start => handle.top_0().h(px(if x == Fixed { EDGE } else { CORNER })),
            End => handle.bottom_0().h(px(if x == Fixed { EDGE } else { CORNER })),
            Fixed => handle.top(px(CORNER)).bottom(px(CORNER)),
        }
        .when(x != Fixed && y == End, |handle| {
            handle.child(
                div()
                    .absolute()
                    .bottom(px(3.0))
                    .size(px(8.0))
                    .border_b_1()
                    .border_color(cx.theme().muted_foreground.opacity(0.55))
                    .when(x == Start, |corner| corner.left(px(3.0)).border_l_1())
                    .when(x == End, |corner| corner.right(px(3.0)).border_r_1()),
            )
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event, _, cx| {
                this.start_scratch_float_drag(gesture, event, cx);
            }),
        )
    })
    .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_corner_can_shrink_a_right_float_and_the_title_can_move_it_to_an_edge() {
        let area = size(px(1000.), px(800.));
        let frame = Mode::Right.default_frame(0.5);
        let frame = frame.adjusted(
            Gesture::Resize(Edge::Start, Edge::End),
            gpui::point(px(200.), px(-500.)),
            area,
        );
        assert_eq!(frame, Frame { left: 0.7, top: 0., right: 1., bottom: 0.375 });
        let frame = frame.adjusted(Gesture::Move, gpui::point(px(-3000.), px(3000.)), area);
        assert_eq!(frame.left, 0.);
        assert_eq!(frame.bottom, 1.);
        assert!((frame.right - 0.3).abs() < 0.0001);
    }

    #[test]
    fn a_title_click_stays_pending_but_a_drag_stays_active_after_returning_to_its_start() {
        let original = Mode::Right.default_frame(0.5);
        let mut drag = Drag {
            owner: TabId("test-tab".into()),
            mode: Mode::Right,
            gesture: Gesture::Move,
            start: gpui::point(px(20.), px(20.)),
            size: size(px(1000.), px(800.)),
            original,
            preview: original,
            active: false,
            reset_on_click: false,
        };
        assert!(!drag.activate_at(gpui::point(px(23.), px(17.))));
        assert!(drag.activate_at(gpui::point(px(20.), px(24.))));
        assert!(drag.activate_at(drag.start));
    }

    #[test]
    fn resize_preserves_the_fixed_edge_and_stops_at_a_usable_minimum() {
        assert_eq!(resize_axis(0., 1., 3., 0.25, Edge::Start), (0.75, 1.));
        assert_eq!(resize_axis(0., 1., -3., 0.25, Edge::End), (0., 0.25));
        assert_eq!(resize_axis(0., 1., -3., 2., Edge::End), (0., 1.));
        assert_eq!(resize_axis(0.5, 0.625, -3., 0.25, Edge::End), (0.5, 0.625));
    }
}

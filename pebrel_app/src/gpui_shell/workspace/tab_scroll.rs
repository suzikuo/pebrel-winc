//! 侧栏 TABS 列表的整行滚动（旧壳 `chrome_tab_layout` + `nebula_tabs_scroll`）。
//!
//! 标签多到超出侧栏剩余高度时，只画一个连续窗口，右侧贴覆盖式 thumb
//! （`overlay_scrollbar`）：不占行宽、不压状态徽章。滚轮按整行步进；thumb
//! 仅在列表悬停或拖拽时绘制。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Bounds, Context, InteractiveElement as _, IntoElement as _, MouseButton,
    MouseDownEvent, ParentElement as _, Pixels, Point, ScrollWheelEvent,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, px,
};

use crate::gpui_shell::prelude::*;
use crate::plugins::workspace::SidebarStyle;
use crate::scrollbar::{self, OverlayScrollbar};

use pebrel_settings::DensityName;

use super::{NebulaWorkspace, tab_groups::SidebarRow};

pub(super) const TAB_ROW_HEIGHT: f32 = crate::display::ui::tokens::control::MIN_HIT_TARGET;

fn sidebar_row_height(row: SidebarRow, density: DensityName, style: SidebarStyle) -> f32 {
    match row {
        SidebarRow::Group(_) => style.group_height.map_or_else(
            || match density {
                DensityName::Standard => 42.0,
                DensityName::Compact => 38.0,
            },
            f32::from,
        ),
        _ => style.tab_height.map_or(TAB_ROW_HEIGHT, f32::from),
    }
}

pub(super) fn tab_row_gap(density: DensityName) -> f32 {
    use crate::display::ui::tokens::space;
    match density {
        DensityName::Standard => space::XS,
        DensityName::Compact => space::XXS,
    }
}

/// 旧壳 `tab_right_pad`：给覆盖式滚动条留的恒定沟槽，出现与否都不改行宽。
pub(super) const TAB_SCROLL_GUTTER: f32 = 9.0;

/// Additional space at the boundary between two sidebar sections.
const SECTION_GAP: f32 = 6.0;

struct RowBand {
    top: f32,
    bottom: f32,
}

pub(super) struct SidebarRowLayout {
    bands: Vec<RowBand>,
    gap: f32,
}

impl SidebarRowLayout {
    pub(super) fn new(rows: &[SidebarRow], density: DensityName, style: SidebarStyle) -> Self {
        let gap = tab_row_gap(density);
        let section_gap = style.section_gap.map_or(SECTION_GAP, f32::from);
        let mut offset = 0.0;
        let bands = rows
            .iter()
            .enumerate()
            .map(|(index, &row)| {
                if index > 0 && matches!(row, SidebarRow::Group(_) | SidebarRow::Ungrouped) {
                    offset += section_gap;
                }
                let top = offset;
                let bottom = top + sidebar_row_height(row, density, style);
                offset = bottom + gap;
                RowBand { top, bottom }
            })
            .collect();
        Self { bands, gap }
    }

    pub(super) fn row_height(&self, index: usize) -> f32 {
        let band = &self.bands[index];
        band.bottom - band.top
    }

    pub(super) fn row_bottom(&self, index: usize, scroll: usize) -> f32 {
        self.bands[index].bottom - self.bands[scroll].top
    }

    pub(super) fn extra_gap_before(&self, index: usize) -> f32 {
        self.bands[index].top - self.bands[index - 1].bottom - self.gap
    }

    fn content_height(&self) -> f32 {
        self.bands.last().map_or(0.0, |band| band.bottom)
    }

    fn visible_end(&self, start: usize, avail_h: f32) -> usize {
        let Some(band) = self.bands.get(start) else { return start };
        // The first frame has no measured height yet.
        if avail_h <= 1.0 {
            return self.bands.len();
        }
        let limit = band.top + avail_h + 0.5;
        start + self.bands[start..].partition_point(|band| band.bottom <= limit)
    }

    fn first_fitting_start(&self, last: usize, avail_h: f32) -> usize {
        if avail_h <= 1.0 {
            return 0;
        }
        let minimum = self.bands[last].bottom - avail_h - 0.5;
        self.bands[..last].partition_point(|band| band.top < minimum)
    }

    pub(super) fn row_at(&self, y: f32, scroll: usize, show: usize) -> Option<usize> {
        let bands = self.bands.get(scroll..scroll + show)?;
        let absolute_y = y + bands.first()?.top;
        let index = bands.partition_point(|band| band.top <= absolute_y).checked_sub(1)?;
        (absolute_y < bands[index].bottom).then_some(scroll + index)
    }
}

/// 旧壳 `sidebar_wheel`：一次手势一行，符号与 winit 的 `-signum` 一致。
pub(super) fn wheel_rows(delta_y: f32) -> i32 {
    if delta_y > 0.0 {
        -1
    } else if delta_y < 0.0 {
        1
    } else {
        0
    }
}

pub(super) fn apply_wheel(scroll: usize, max: usize, rows: i32) -> usize {
    (scroll as i32 + rows).clamp(0, max as i32) as usize
}

fn point_in_rect((rx, ry, rw, rh): (f32, f32, f32, f32), x: f32, y: f32) -> bool {
    x >= rx && y >= ry && x < rx + rw && y < ry + rh
}

fn tabs_scroll_thumb(bar: &OverlayScrollbar, color: gpui::Hsla) -> gpui::Div {
    // 滑块只负责绘制，命中由独立轨道处理；遮挡父列表会形成 hover 显隐反馈循环。
    div()
        .debug_selector(|| "sidebar-tabs-scroll-thumb".to_owned())
        .absolute()
        .left(px(bar.thumb.0))
        .top(px(bar.thumb.1))
        .w(px(bar.thumb.2))
        .h(px(bar.thumb.3))
        .rounded_full()
        .bg(color)
}

struct TabsWindow {
    scroll: usize,
    show: usize,
    max: usize,
}

impl NebulaWorkspace {
    pub(super) fn tab_row_pitch(&self) -> f32 {
        sidebar_row_height(SidebarRow::Tab(0, None), self.density, self.plugin_workspace.sidebar)
            + tab_row_gap(self.density)
    }

    pub(super) fn sidebar_row_layout_for(&self, rows: &[SidebarRow]) -> SidebarRowLayout {
        SidebarRowLayout::new(rows, self.density, self.plugin_workspace.sidebar)
    }

    fn sidebar_row_layout(&self) -> SidebarRowLayout {
        self.sidebar_row_layout_for(&self.sidebar_rows())
    }

    fn tabs_window(&self, layout: &SidebarRowLayout) -> TabsWindow {
        let max = layout
            .bands
            .len()
            .checked_sub(1)
            .map_or(0, |last| layout.first_fitting_start(last, self.tabs_viewport_h));
        let scroll = self.tabs_scroll.min(max);
        let show = layout.visible_end(scroll, self.tabs_viewport_h) - scroll;
        TabsWindow { scroll, show, max }
    }

    pub(super) fn tabs_visible_window(&self, layout: &SidebarRowLayout) -> (usize, usize) {
        let window = self.tabs_window(layout);
        (window.scroll, window.show)
    }

    pub(super) fn clamp_tabs_scroll(&mut self) {
        let max = self.tabs_window(&self.sidebar_row_layout()).max;
        self.tabs_scroll = self.tabs_scroll.min(max);
    }

    pub(super) fn reveal_active_tab(&mut self) {
        let active_row = self.reveal_sidebar_tab();
        let layout = self.sidebar_row_layout();
        let window = self.tabs_window(&layout);
        self.tabs_scroll = if window.show == 0 {
            0
        } else if active_row < window.scroll {
            active_row
        } else if active_row >= window.scroll + window.show {
            layout.first_fitting_start(active_row, self.tabs_viewport_h).min(window.max)
        } else {
            window.scroll
        };
        self.top_tabs_scroll.scroll_to_item(if self.settings_open {
            self.tabs.len()
        } else {
            self.active
        });
    }

    fn tabs_overlay_bar(
        &self,
        layout: &SidebarRowLayout,
        window: &TabsWindow,
    ) -> Option<OverlayScrollbar> {
        if self.tabs_section_collapsed || window.show == 0 || window.max == 0 {
            return None;
        }
        let viewport = self.tabs_viewport_h;
        let content = layout.content_height();
        // The thumb's travel and drag mapping use the same row-based range.
        let offset = (content - viewport) * window.scroll as f32 / window.max as f32;
        scrollbar::overlay_scrollbar(
            (0.0, 0.0, self.tabs_list_width.max(1.0), viewport),
            viewport,
            content,
            offset,
            1.0,
        )
    }

    fn local_tabs_point(&self, position: Point<Pixels>) -> (f32, f32) {
        (
            f32::from(position.x - self.tabs_list_origin.x),
            f32::from(position.y - self.tabs_list_origin.y),
        )
    }

    fn press_tabs_scrollbar(&mut self, event: &MouseDownEvent) -> bool {
        let layout = self.sidebar_row_layout();
        let window = self.tabs_window(&layout);
        let Some(bar) = self.tabs_overlay_bar(&layout, &window) else { return false };
        let (x, y) = self.local_tabs_point(event.position);
        if !bar.hit_test(x, y) {
            return false;
        }
        let grab = if point_in_rect(bar.thumb, x, y) { y - bar.thumb.1 } else { bar.thumb.3 * 0.5 };
        self.tabs_scroll_grab = Some(grab);
        self.tabs_scroll = bar.target_offset(y, grab, window.max);
        true
    }

    fn drag_tabs_scrollbar_to(&mut self, position: Point<Pixels>) -> bool {
        let Some(grab) = self.tabs_scroll_grab else { return false };
        let layout = self.sidebar_row_layout();
        let window = self.tabs_window(&layout);
        let Some(bar) = self.tabs_overlay_bar(&layout, &window) else { return false };
        let (_, y) = self.local_tabs_point(position);
        let next = bar.target_offset(y, grab, window.max);
        if next == self.tabs_scroll {
            return false;
        }
        self.tabs_scroll = next;
        true
    }

    fn end_tabs_scrollbar_drag(&mut self) -> bool {
        self.tabs_scroll_grab.take().is_some()
    }

    fn on_tabs_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta_y = f32::from(event.delta.pixel_delta(px(self.tab_row_pitch())).y);
        let rows = wheel_rows(delta_y);
        if rows == 0 {
            return;
        }
        let max = self.tabs_window(&self.sidebar_row_layout()).max;
        let next = apply_wheel(self.tabs_scroll, max, rows);
        if next != self.tabs_scroll {
            self.tabs_scroll = next;
            cx.notify();
        }
        cx.stop_propagation();
    }

    /// 列表槽：恒定右沟槽 + 覆盖 thumb + 滚轮。`items` 已是可见窗口。
    pub(super) fn wrap_tabs_scroll_list(
        &self,
        layout: &SidebarRowLayout,
        items: impl IntoIterator<Item = gpui::AnyElement>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let bar = self.tabs_overlay_bar(layout, &self.tabs_window(layout));
        let dragging = self.tabs_scroll_grab.is_some();
        let show_thumb = bar.is_some() && (self.tabs_list_hot || dragging);
        let workspace = cx.entity().downgrade();
        let thumb_color = {
            let theme = cx.theme();
            let alpha = if dragging {
                0.78
            } else if self.tabs_list_hot {
                0.64
            } else {
                0.46
            };
            theme.scrollbar_thumb.opacity(alpha)
        };

        div()
            .id("sidebar-tabs-list")
            .flex_1()
            .w_full()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .pr(px(TAB_SCROLL_GUTTER))
            .on_scroll_wheel(cx.listener(Self::on_tabs_wheel))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if this.tabs_list_hot != *hovered {
                    this.tabs_list_hot = *hovered;
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        let _ = workspace.update(cx, |this, cx| {
                            this.remember_tabs_list_bounds(bounds, cx);
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(v_flex().w_full().gap(px(tab_row_gap(self.density))).children(items))
            .when_some(bar.filter(|_| show_thumb), |list, bar| {
                list.child(tabs_scroll_thumb(&bar, thumb_color))
            })
            .when_some(bar, |list, bar| {
                list.child(
                    div()
                        .id("sidebar-tabs-scrollbar")
                        .absolute()
                        .left(px(bar.hit.0))
                        .top(px(bar.hit.1))
                        .w(px(bar.hit.2))
                        .h(px(bar.hit.3))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                if this.press_tabs_scrollbar(event) {
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                            }),
                        ),
                )
            })
            .into_any_element()
    }

    fn remember_tabs_list_bounds(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        let h = f32::from(bounds.size.height);
        let w = f32::from(bounds.size.width);
        let origin = bounds.origin;
        let height_changed = (self.tabs_viewport_h - h).abs() > 0.5;
        let width_changed = (self.tabs_list_width - w).abs() > 0.5;
        self.tabs_list_origin = origin;
        if !height_changed && !width_changed {
            return;
        }
        // Collapse clips the list to an unusable height; keep the last open viewport.
        if !self.tabs_section_collapsed && !self.tabs_fold_frozen && h >= TAB_ROW_HEIGHT {
            self.tabs_viewport_h = h;
        }
        self.tabs_list_width = w;
        self.clamp_tabs_scroll();
        cx.notify();
    }

    pub(super) fn tabs_scrollbar_drag_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.tabs_scroll_grab.is_none() {
            return None;
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                    if this.drag_tabs_scrollbar_to(event.position) {
                        cx.notify();
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if this.end_tabs_scrollbar_drag() {
                            cx.notify();
                        }
                    }),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{DensityName, SidebarRow, SidebarRowLayout, SidebarStyle, apply_wheel, wheel_rows};

    #[test]
    fn compact_rows_fit_more_tabs_without_shrinking_hit_targets() {
        for (density, expected_show) in [(DensityName::Standard, 2), (DensityName::Compact, 3)] {
            let layout = SidebarRowLayout::new(&[SidebarRow::Tab(0, None); 10], density, SidebarStyle::default());
            assert_eq!(layout.row_height(0), 32.0);
            assert_eq!(layout.visible_end(0, 104.0), expected_show);
            assert_eq!(layout.first_fitting_start(9, 104.0), 10 - expected_show);
            assert_eq!(layout.visible_end(0, 0.0), 10);
            assert_eq!(layout.visible_end(0, 20.0), 0);
        }
    }

    #[test]
    fn plugin_sizes_drive_both_visible_rows_and_drop_bands() {
        use SidebarRow::{Group, Tab};
        let rows = [Group(0), Tab(0, Some(0)), Group(1)];
        let style = SidebarStyle {
            tab_height: Some(36),
            group_height: Some(48),
            section_gap: Some(10),
            ..Default::default()
        };
        let layout = SidebarRowLayout::new(&rows, DensityName::Standard, style);
        assert_eq!(layout.row_height(0), 48.0);
        assert_eq!(layout.row_height(1), 36.0);
        assert_eq!(layout.visible_end(0, 92.0), 2);
        assert_eq!(layout.row_at(95.0, 0, 3), None);
        assert_eq!(layout.row_at(110.0, 0, 3), Some(2));
    }

    #[test]
    fn section_gaps_count_toward_the_viewport_only_between_visible_rows() {
        use SidebarRow::{Group, Tab, Ungrouped};
        let rows = [Group(0), Tab(0, Some(0)), Group(1), Ungrouped, Tab(1, None)];
        let layout = SidebarRowLayout::new(&rows, DensityName::Standard, SidebarStyle::default());
        let group = layout.row_height(0);
        let tab = layout.row_height(1);
        assert_eq!((group, tab), (42.0, 32.0));
        assert_eq!(layout.extra_gap_before(1), 0.0);
        assert_eq!(layout.extra_gap_before(2), 6.0);
        assert_eq!(layout.extra_gap_before(3), 6.0);
        assert_eq!(layout.extra_gap_before(4), 0.0);
        assert_eq!(layout.content_height(), 2.0 * group + 3.0 * tab + 4.0 * layout.gap + 12.0);
        let group_and_tab = group + tab + layout.gap;
        assert_eq!(layout.visible_end(0, group_and_tab), 2);
        assert_eq!(layout.visible_end(1, group_and_tab), 2);
        assert_eq!(layout.visible_end(2, group_and_tab + 6.0), 4);
        let two_tabs = 2.0 * tab + layout.gap;
        assert_eq!(layout.first_fitting_start(4, two_tabs), 3);
        assert_eq!(layout.visible_end(3, two_tabs), 5);
    }

    #[test]
    fn wheel_steps_one_row_and_clamps() {
        assert_eq!(wheel_rows(12.0), -1);
        assert_eq!(wheel_rows(-3.0), 1);
        assert_eq!(wheel_rows(0.0), 0);
        assert_eq!(apply_wheel(0, 5, -1), 0);
        assert_eq!(apply_wheel(5, 5, 1), 5);
        assert_eq!(apply_wheel(2, 5, 1), 3);
    }

    #[cfg(feature = "gpui-test-support")]
    mod hover {
        use super::super::{scrollbar, tabs_scroll_thumb};
        use gpui::prelude::*;
        use gpui::{Context, MouseButton, Render, TestAppContext, Window, div, point, px};

        #[derive(Default)]
        struct ScrollHoverProbe {
            hovered: bool,
            presses: usize,
            hover_transitions: usize,
        }

        impl Render for ScrollHoverProbe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let bar =
                    scrollbar::overlay_scrollbar((0.0, 0.0, 200.0, 200.0), 200.0, 800.0, 0.0, 1.0)
                        .unwrap();
                div()
                    .id("tabs-hover-probe")
                    .relative()
                    .w(px(200.0))
                    .h(px(200.0))
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        if this.hovered != *hovered {
                            this.hovered = *hovered;
                            this.hover_transitions += 1;
                            // 本场景只进出列表各一次；直接拦住反馈循环，避免回归时挂住测试。
                            assert!(
                                this.hover_transitions <= 2,
                                "ancestor hover keeps oscillating"
                            );
                            cx.notify();
                        }
                    }))
                    .when(self.hovered, |list| list.child(tabs_scroll_thumb(&bar, gpui::black())))
                    .child(
                        div()
                            .id("tabs-hover-track")
                            .absolute()
                            .left(px(bar.hit.0))
                            .top(px(bar.hit.1))
                            .w(px(bar.hit.2))
                            .h(px(bar.hit.3))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.presses += 1;
                                    cx.stop_propagation();
                                }),
                            ),
                    )
            }
        }

        #[gpui::test]
        fn thumb_keeps_parent_hover_stable_and_track_clickable(cx: &mut TestAppContext) {
            let (probe, cx) = cx.add_window_view(|_, _| ScrollHoverProbe::default());
            let draw = |cx: &mut gpui::VisualTestContext| {
                cx.update(|window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                });
            };
            cx.simulate_mouse_move(point(px(195.0), px(190.0)), None, Default::default());
            draw(cx);
            let thumb = cx.debug_bounds("sidebar-tabs-scroll-thumb").expect("hover shows thumb");
            cx.simulate_mouse_move(thumb.center(), None, Default::default());
            // 鼠标保持不动、连续重绘，复现遮挡父 hover 导致的显示/隐藏反馈循环。
            for frame in 0..8 {
                draw(cx);
                assert!(probe.read_with(cx, |probe, _| probe.hovered), "frame {frame}");
                assert_eq!(cx.debug_bounds("sidebar-tabs-scroll-thumb"), Some(thumb));
            }
            cx.simulate_mouse_down(thumb.center(), MouseButton::Left, Default::default());
            cx.simulate_mouse_up(thumb.center(), MouseButton::Left, Default::default());
            assert_eq!(probe.read_with(cx, |probe, _| probe.presses), 1);
            cx.simulate_mouse_move(point(px(220.0), px(100.0)), None, Default::default());
            draw(cx);
            assert!(!probe.read_with(cx, |probe, _| probe.hovered));
            assert!(cx.debug_bounds("sidebar-tabs-scroll-thumb").is_none());
        }
    }
}

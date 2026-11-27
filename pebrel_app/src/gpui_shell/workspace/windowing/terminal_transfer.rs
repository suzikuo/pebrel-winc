//! Transfer live terminal tabs and panes between workspace windows.

use super::*;

pub(crate) fn move_terminal_to_new_window(payload: CrossWindowTerminalDrag, cx: &mut App) {
    if !payload
        .source
        .upgrade()
        .is_some_and(|source| payload.source_tab_index(source.read(cx)).is_some())
    {
        payload.report_failed_pane_move(cx);
        return;
    }
    let opened =
        open_workspace_window(cx, WorkspaceStartup::Empty, None, None, true, WindowRole::Regular);
    let Ok((target_window_id, target_workspace)) = opened else {
        log::warn!("failed to open a window for moved terminal");
        payload.report_failed_pane_move(cx);
        return;
    };
    let handle = target_workspace.read(cx).window_handle;
    let result = handle.update(cx, |_, window, cx| {
        target_workspace.update(cx, |workspace, cx| {
            workspace.accept_terminal_transfer(&payload, None, window, cx)
        })
    });
    if !matches!(result, Ok(true)) {
        unregister(target_window_id, cx);
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        payload.report_failed_pane_move(cx);
    }
}

/// Enter the destination's window context before the source can be detached.
pub(crate) fn drop_terminal_to_existing_window(
    payload: CrossWindowTerminalDrag,
    destination: CrossWindowDropDestination,
    cx: &mut App,
) -> bool {
    let Some(entry) = entry_by_id(destination.window_id, cx) else {
        payload.report_failed_pane_move(cx);
        return false;
    };
    let target = entry.workspace.clone();
    let result = entry.handle.update(cx, |_, window, cx| {
        target.update(cx, |workspace, cx| {
            if payload.pane_source_tab.is_some()
                && !destination.pane_dock_tab.as_ref().is_some_and(|tab| {
                    workspace
                        .tab_meta
                        .get(workspace.active)
                        .is_some_and(|meta| &meta.runtime_id == tab)
                        && destination.dock.is_some_and(|dock| workspace.can_dock_terminal_at(dock))
                })
            {
                return false;
            }
            let moved = workspace.accept_terminal_transfer(&payload, destination.dock, window, cx);
            if moved && payload.pane_source_tab.is_some() {
                window.activate_window();
            }
            moved
        })
    });
    let moved = matches!(result, Ok(Ok(true)));
    if !moved {
        payload.report_failed_pane_move(cx);
    }
    moved
}

impl NebulaWorkspace {
    pub(crate) fn cross_window_drag_payload(
        &self,
        ix: usize,
        cx: &Context<Self>,
    ) -> Option<CrossWindowTerminalDrag> {
        let WorkspaceTab::Terminal { focused, .. } = self.tabs.get(ix)? else { return None };
        Some(CrossWindowTerminalDrag {
            source: cx.entity().downgrade(),
            source_window_id: self.runtime_window_id,
            pane_id: *focused,
            pane_source_tab: None,
        })
    }

    pub(in super::super) fn cross_window_pane_payload(
        &self,
        ix: usize,
        pane_id: u64,
        cx: &Context<Self>,
    ) -> Option<CrossWindowTerminalDrag> {
        let WorkspaceTab::Terminal { panes, tree, .. } = self.tabs.get(ix)? else { return None };
        if panes.len() < 2 || !tree.contains(pane_id) {
            return None;
        }
        Some(CrossWindowTerminalDrag {
            source: cx.entity().downgrade(),
            source_window_id: self.runtime_window_id,
            pane_id,
            pane_source_tab: Some(self.tab_meta.get(ix)?.runtime_id.clone()),
        })
    }

    pub(crate) fn cross_window_drag_preview(
        _payload: &CrossWindowTerminalDrag,
        cx: &mut App,
    ) -> Entity<TabDragPreview> {
        cx.new(|_| TabDragPreview)
    }

    pub(crate) fn schedule_move_tab_to_new_window(&self, ix: usize, cx: &mut Context<Self>) {
        let Some(payload) = self.cross_window_drag_payload(ix, cx) else { return };
        cx.defer(move |cx| move_terminal_to_new_window(payload, cx));
    }

    pub(crate) fn accept_terminal_transfer(
        &mut self,
        payload: &CrossWindowTerminalDrag,
        dock: Option<DockTarget>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if payload.source_window_id == self.runtime_window_id {
            return false;
        }
        let detached = match payload.source.update(cx, |source, cx| {
            let ix = payload.source_tab_index(source)?;
            if payload.pane_source_tab.is_some() {
                let tab = source.detach_split_pane(ix, payload.pane_id, cx)?;
                Some(DetachedTerminalTab {
                    tab,
                    meta: TabMeta::default(),
                    pane_ids: vec![payload.pane_id],
                    source_window_id: source.runtime_window_id,
                    source_handle: source.window_handle,
                    source_became_empty: false,
                })
            } else {
                source.detach_terminal_tab(ix)
            }
        }) {
            Ok(Some(detached)) => detached,
            _ => return false,
        };

        let pane_ids = detached.pane_ids.clone();
        let source_window_id = detached.source_window_id;
        let source_handle = detached.source_handle;
        let source_became_empty = detached.source_became_empty;
        let mut tab = detached.tab;
        if let WorkspaceTab::Terminal { panes, .. } = &mut tab {
            for pane in panes {
                pane.view.update(cx, |view, cx| view.rebind_window(window, cx));
                pane._subscription = cx.subscribe_in(&pane.view, window, Self::on_terminal_event);
            }
        }
        self.runtime_hub.move_panes_to_window(source_window_id, self.runtime_window_id, &pane_ids);
        self.attach_terminal_tab(tab, detached.meta, dock, window, cx);

        let source = payload.source.clone();
        if source_became_empty {
            unregister(source_window_id, cx);
            let _ = source_handle.update(cx, |_, source_window, _| source_window.remove_window());
        } else {
            let _ = source_handle.update(cx, move |_, source_window, cx| {
                let _ = source.update(cx, |source, cx| {
                    if source.tabs.is_empty() && source.settings_tab_open {
                        source.open_settings(source_window, cx);
                    }
                    source.reveal_active_tab();
                    source.focus_active(source_window, cx);
                    source.sync_side_panel_to_active(true, cx);
                    cx.notify();
                });
            });
        }
        publish_runtime_snapshot_with_current(
            self.window_hidden,
            self.runtime_window_snapshot(window, cx).1,
            cx,
        );
        true
    }

    fn detach_terminal_tab(&mut self, ix: usize) -> Option<DetachedTerminalTab> {
        let (tab, meta) = self.remove_tab_at(ix)?;
        let WorkspaceTab::Terminal { panes, .. } = &tab else {
            self.insert_tab_at(ix, tab, meta);
            return None;
        };
        let pane_ids = panes.iter().map(|pane| pane.id).collect::<Vec<_>>();
        for pane_id in &pane_ids {
            self.pane_bounds.borrow_mut().remove(pane_id);
        }
        self.split_bounds.borrow_mut().clear();
        self.tab_drag = None;
        self.tab_menu = None;
        self.tab_rename = None;
        if ix < self.active {
            self.active -= 1;
        }
        if !self.tabs.is_empty() {
            self.active = self.active.min(self.tabs.len() - 1);
        }
        let source_became_empty = self.tabs.is_empty() && !self.settings_tab_open;
        Some(DetachedTerminalTab {
            tab,
            meta,
            pane_ids,
            source_window_id: self.runtime_window_id,
            source_handle: self.window_handle,
            source_became_empty,
        })
    }

    fn attach_terminal_tab(
        &mut self,
        tab: WorkspaceTab,
        meta: TabMeta,
        dock: Option<DockTarget>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let can_dock = dock.is_some_and(|target| self.can_dock_terminal_at(target));
        if can_dock {
            let Some(target) = dock else { unreachable!("can_dock requires a direction") };
            let WorkspaceTab::Terminal {
                panes: source_panes,
                tree: source_tree,
                focused: source_focused,
                ..
            } = tab
            else {
                unreachable!("cross-window drag payloads only contain terminal tabs")
            };
            let Some(WorkspaceTab::Terminal { panes, tree, focused, zoomed, broadcast, .. }) =
                self.tabs.get_mut(self.active)
            else {
                unreachable!("can_dock checked the active terminal tab")
            };
            if let Some(pane) = target.pane {
                tree.dock_at_leaf(pane, source_tree, target.nav)
                    .expect("destination checked above");
            } else {
                let previous = std::mem::replace(tree, SplitTree::leaf(source_focused));
                *tree = previous.joined(source_tree, target.nav);
            }
            panes.extend(source_panes);
            *focused = source_focused;
            *zoomed = false;
            // 跨窗 dock 改变了 tab 的接收集合，不能继承任一侧的广播开关。
            *broadcast = false;
        } else {
            let at = self.tabs.len();
            self.insert_tab_at(at, tab, meta);
            self.active = at;
        }
        self.mark_structural_resize(self.active, cx);
        self.cross_window_dock = None;
        self.reveal_active_tab();
        self.focus_active(window, cx);
        self.sync_side_panel_to_active(true, cx);
        cx.notify();
    }
}

use super::*;
use crate::gpui_shell::terminal::view::TerminalLaunch;
use gpui::{Modifiers, TestAppContext, VisualTestContext, point};

#[gpui::test]
fn floating_title_distinguishes_session_clicks_from_panel_drags(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (workspace, mut window) = open_floating_panel(cx, directory.path());
    redraw(&mut window);
    let initial = window.debug_bounds("scratch-terminal-panel").unwrap();
    let first = window.debug_bounds("scratch-terminal-session-1").unwrap().center();
    let modifiers = Modifiers::default();
    window.simulate_mouse_down(first, MouseButton::Left, modifiers);
    window.simulate_mouse_up(first + point(px(2.), px(0.)), MouseButton::Left, modifiers);
    workspace.read_with(&window, |view, _| {
        assert_eq!(view.visible_scratch().unwrap().selected, 0);
        assert!(!view.has_scratch_float_gesture());
    });

    let second = window.debug_bounds("scratch-terminal-session-2").unwrap().center();
    let destination = second - point(px(0.), px(40.));
    window.simulate_mouse_down(second, MouseButton::Left, modifiers);
    window.simulate_mouse_move(destination, MouseButton::Left, modifiers);
    redraw(&mut window);
    assert_eq!(
        window.debug_bounds("scratch-terminal-panel").unwrap().top(),
        initial.top() - px(40.)
    );
    window.simulate_mouse_up(destination, MouseButton::Left, modifiers);
    redraw(&mut window);
    workspace.read_with(&window, |view, _| {
        assert_eq!(view.visible_scratch().unwrap().selected, 0);
        assert!(!view.has_scratch_float_gesture());
    });

    let title = window.debug_bounds("scratch-terminal-tabs").unwrap();
    let blank = point(title.right() - px(12.), title.center().y);
    assert!(blank.x > window.debug_bounds("scratch-terminal-session-2").unwrap().right());
    window.simulate_mouse_down(blank, MouseButton::Left, modifiers);
    window.simulate_mouse_move(blank - point(px(0.), px(30.)), MouseButton::Left, modifiers);
    window.simulate_mouse_up(blank - point(px(0.), px(30.)), MouseButton::Left, modifiers);
    redraw(&mut window);
    assert_eq!(
        window.debug_bounds("scratch-terminal-panel").unwrap().top(),
        initial.top() - px(70.)
    );

    let hide = window.debug_bounds("scratch-terminal-hide").unwrap().center();
    window.simulate_click(hide, modifiers);
    workspace.read_with(&window, |view, _| assert!(view.visible_scratch().is_none()));
}

fn redraw(window: &mut VisualTestContext) {
    window.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn open_floating_panel(
    cx: &mut TestAppContext,
    directory: &std::path::Path,
) -> (Entity<NebulaWorkspace>, VisualTestContext) {
    let hub = crate::runtime_api::RuntimeHub::new();
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_shell::math_view::register(cx);
        crate::gpui_shell::file_editor::init(cx);
        super::super::init(cx);
        windowing::initialize(cx, hub.clone());
        cx.set_reduce_motion(true);
    });
    let launch = || TerminalLaunch::Local {
        cwd: Some(directory.into()),
        shell: Some(pebrel_terminal::tty::Shell::new(
            directory.join("missing-test-shell").to_string_lossy().into_owned(),
            vec![],
        )),
        shell_name: None,
    };
    let mut workspace = None;
    let (_, mut window) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            NebulaWorkspace::new(
                window,
                None,
                None,
                1,
                hub,
                windowing::WorkspaceStartup::Empty,
                windowing::WindowRole::Regular,
                cx,
            )
        });
        view.update(cx, |view, cx| {
            let main = view.new_pane((80, 24), launch(), None, window, cx);
            let focused = main.id;
            view.insert_tab_at(
                0,
                WorkspaceTab::Terminal {
                    panes: vec![main],
                    tree: SplitTree::leaf(focused),
                    focused,
                    zoomed: false,
                    broadcast: false,
                },
                TabMeta::default(),
            );
            view.active = 0;
            view.scratch.set_available(true);
            view.scratch.settings.placement = ScratchPlacement::FloatingBottom;
            let owner = view.scratch_owner().unwrap().clone();
            let mut panel = Panel::default();
            for _ in 0..2 {
                panel.push(view.new_pane((80, 24), launch(), None, window, cx));
            }
            view.scratch.panels.insert(owner, panel);
            view.focus_active(window, cx);
            cx.notify();
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    window.update(|window, _| window.activate_window());
    window.run_until_parked();
    (workspace.unwrap(), window.clone())
}

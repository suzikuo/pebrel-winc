# Temporary terminals in the workspace

## Status

Implemented in the Winc worktree.

## Context

A user keeps Pi in one pane and opens another pane for occasional commands in the same directory. The quick terminal uses an independent window and starts with its own default context. An in-window panel can offer the same convenience without adding permanent leaves to the tab's split tree.

## Evidence

The workspace already owns native terminal views, process checks, focus, and startup context. Installed workspace contributions already carry receipts bound to the manifest. The focused terminal provides a shell-reported directory and, for WSL, the launch-time distribution and explicit user.

## Decision

Use `workspace.scratch_terminal` as a reviewed declarative opt-in for the native panel. Only an explicit host UI action can create a Shell. The workspace owns a set of terminal views per stable tab ID in its window and renders only the selected view while the panel is visible. Every Shell has its own PTY. A close confirmation identifies the exact pane so a later selection cannot retarget it. When the user closes all scratch shells, the dialog checks every shell in the tab and captures the set of pane IDs at the time of the request. The workspace restores focus once after closing the captured shells. The scratch slots do not participate in split broadcast, Runtime pane targeting, or session snapshots. A hidden Shell continues running. Disabling the last contributing plugin hides the panels and lets the existing Shells live until they exit or their owning tab leaves the window.

Preferences use the normal profile settings and reset flow. Placement, default size, and Esc policy apply to open panels. Each floating layout keeps its own rectangle per live tab, expressed as fractions of the terminal area so it stays inside the workspace when the window changes size. During a drag the workspace uses the window capture listener for movement and release both inside and outside the window. The drag overlay occludes the workspace root's hover hitbox. Escape discards the preview. Double-clicking the grip clears the rectangle and restores the layout and size from settings. Directory inheritance is sampled on new Shell creation. WSL launches a fresh interactive Shell in the same distribution and user, with the guest directory when available and enabled. An SSH or serial pane cannot create a scratch Shell.

## Rejected alternatives

An ordinary split would alter the persistent tree and resize adjacent panes even for the floating layouts. Giving Luau access to terminal data or process arguments is unnecessary for this interaction. Using the existing quick terminal would make unrelated tabs share the same Shell.

## Consequences

Commands typed by the user run in a regular terminal session, and foreground jobs participate in close checks for the containing tab or window. Scratch sessions belong to the current window, so moving a tab to another window ends its scratch sessions. Plugins share the host entry and preferences. They receive no pane data.

## Validation

The settings model tests preference decoding and invalid-value fallback. The installation test covers explicit grants, the enabled flag, and modified package rejection. A Windows GPUI fixture uses the toolbar and shortcut to open two PowerShell sessions, verifies independent variables and focus after hiding or closing a session, and checks that Escape hides a focused panel and that the selected placement survives reopening the application. A separate Ubuntu-20.04 fixture checks the new shell’s distribution and working directory. The floating-layout fixture uses real pointer input to measure the panel midway through both a corner resize and a move. It also checks the upper-right clamp, release outside the app, Escape cancellation, reopening with the same geometry and shell state, and reset with a double-click. The close fixture exercises an unselected busy shell, cancellation, confirmation, another tab's surviving shell, and the one-shell close. Geometry tests cover the fixed edge, the minimum resize size, and clamping during movement.

## Supersedes

None.

## Revisit when

Revisit window ownership if scratch sessions need to travel with tabs between windows or survive restarts.

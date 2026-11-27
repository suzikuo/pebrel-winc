# Move a live split pane between workspace windows

## Status

Implemented on the Windows product branch.

## Context

A pane header could exchange positions within a split or extract a pane into a
separate tab. Moving that pane to another system window required a second gesture
on the resulting tab.

## Evidence

The [existing tab drag contract](2026-10-10-tab-drag-release-dispatch.md) records
why Windows keeps mouse capture in the source window. Its source-side native
lookup can resolve the visible Pebrel window beneath the pointer. The workspace
already knows how to transfer a live terminal entity, move its Runtime ownership,
and bind its event subscription to the receiving window.

## Decision

Use [the pane header's window capture](../../../../../pebrel_app/src/gpui_shell/workspace/pane_header/drag.rs)
to resolve the gesture from the final mouse position. An in-window drop outside
terminal content extracts a tab. A drop outside the window creates a window,
and Alt requests the same operation from inside a maximized window. An existing
window exposes its current terminal as a docking target on Windows.

The [shared transfer](../../../../../pebrel_app/src/gpui_shell/workspace/windowing/terminal_transfer.rs)
opens or validates the destination before extracting the leaf. Revalidation uses
the original tab identity and pane ID on the source and the previewed tab and
split target on the receiver. The extracted terminal entity keeps its pane ID.
The receiver takes over its subscription and Runtime ownership together with the
layout change. The terminal view rebinds its focus and activation observers to the
receiver and recreates window-owned effect resources as needed.

## Rejected alternatives

- Close and recreate a terminal using saved launch settings. This would lose live
  processes, transport state and scrollback.
- Commit an intermediate tab and then transfer it. A failed window creation or
  stale target would leave an unintended tab and could change focus twice.

## Consequences

The source collapses the removed leaf's parent and retains the surviving panes.
Existing tab gestures still move an entire tree. The terminal, persistence format
and GPUI platform dependencies require no new ownership contracts.

## Validation

[Pane header tests](../../../../../pebrel_app/src/gpui_shell/workspace/pane_header/native_tests.rs)
exercise hit testing, cancellation, swapping and extraction into a tab.
[Transfer tests](../../../../../pebrel_app/src/gpui_shell/workspace/windowing/transfer_tests.rs)
cover window capture, stale destinations and retained entity identity. Native
cross-window cursor lookup and mixed-DPI placement require desktop verification.

## Supersedes

None.

## Revisit when

The pinned GPUI release exposes a shared native cross-window gesture for the
supported desktop platforms.

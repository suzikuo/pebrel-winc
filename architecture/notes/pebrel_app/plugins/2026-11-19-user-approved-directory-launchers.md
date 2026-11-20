# User-approved directory launchers

## Status

Implemented in the source checkout. Windows build and interactive validation are pending.

## Context

A user needs a plugin command that opens a local Windows or WSL pane's working directory in an external application while the pane remains interactive.

## Evidence

The workspace already knows the focused pane's host directory or pinned WSL distribution and guest directory. The plugin library already owns installation grants and enablement. The Runtime allowlist covers `runtime.describe`.

## Decision

A validated declarative launcher provides the command title, absolute Windows executable and separate argument arrays. The installation confirmation shows those values and the library records a digest of the approved declarations. Editing a declaration requires reinstalling the package with approval. The Windows workspace title bar presents buttons for enabled launchers in both tab layouts. Each workspace refreshes its button list on a background task at startup, on activation, and when Settings updates the plugin library. The button captures the focused pane's directory at invocation. A background task acquires the plugin library lock, reloads the install and verifies the selected declaration before submitting the program to the OS.

## Rejected alternatives

General Runtime access would let plugin commands choose arbitrary targets and operations beyond the focused directory. A resident helper would need an independent lifecycle. Terminal text submission would depend on whichever program currently owns terminal input.

## Consequences

Windows and WSL retain their native path forms. The spawn is independent of the pane's input and process state. Disabling or uninstalling the plugin revokes future launches. An application the OS already started retains its own lifetime. The executable path resolves at click time so updates to the application can use the same path.

## Validation

Regression cases are in the plugin installation and workspace title-bar tests. Native acceptance should cover clicking a plugin button on Windows and WSL with a busy pane, both tab layouts, and the button disappearing when the plugin is disabled or uninstalled.

## Supersedes

None.

## Revisit when

Pebrel adds a shared workspace action registry or supports this launcher capability on additional hosts.

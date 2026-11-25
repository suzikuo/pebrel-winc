# Group startup directories keep their launch environment

## Status

Implemented for the GPUI shell picker.

## Context

A named tab group can represent a project whose terminals should open in a fixed directory, even after its last tab closes. A group can contain both native and WSL terminals and can contain WSL terminals from different distributions.

## Evidence

[Group definitions](../../../../../pebrel_app/src/gpui_shell/workspace/tab_groups.rs) already persist independently of sessions. [Terminal working directories](../../../../../pebrel_app/src/gpui_shell/terminal/view/tab_identity.rs) and the distribution captured at PTY spawn identify the focused pane's current location. [Explicit launches](../../../../../pebrel_app/src/gpui_shell/workspace/shell_launch.rs) can set a directory while retaining WSL user and guest-command arguments.

## Decision

Store an optional absolute startup path in the existing profile group definition. Identify its environment as native or as a named WSL distribution. The configuration dialog can copy this pair from a grouped tab's focused pane or accept an explicit directory. The stored value is a snapshot and stays available after the source tab closes.

The group picker recommends a compatible launcher and shows the effective group path for the selected row. At launch, the matching group directory takes precedence over configured defaults and uses the existing explicit-directory adapter. The selected launcher supplies the login user. A WSL recipe without an explicit named distribution continues to use its regular directory rules because its default distribution can change independently.

## Rejected alternatives

Deriving the path from the active tab would change the startup directory when switching tabs or running `cd`. Storing a bare Linux path would lose the identity needed to avoid forwarding that directory to another distribution. A WSL filesystem probe during setup would wake the guest and introduce latency before the launch boundary, where WSL can report missing directories and permission errors directly.

## Consequences

Old group files load with no startup directory. Clearing the field restores the usual shell picker behavior. Group membership and existing sessions remain independent of this new setting. A split tab supplies the directory of its focused pane. Only panes with an identifiable native or WSL directory are available as sources.

## Validation

The group model tests cover legacy data, save and clear. Launcher tests exercise path format, environment matching, and WSL argument preservation. Windows desktop checks at 144 DPI used Nord and NordLight with native Command Prompt and Ubuntu-20.04 sources. They verified mouse and keyboard selection, the actual terminal startup path, cancellation, invalid input, clearing, and persistence across restart with no group members. Manual WSL directory entry was also exercised in an empty group.

## Supersedes

None.

## Revisit when

SSH hosts gain group-scoped startup directories or a launcher can supply a verified distribution identity for recipes that follow the system default.

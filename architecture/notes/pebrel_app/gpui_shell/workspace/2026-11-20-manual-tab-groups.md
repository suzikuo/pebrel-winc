# Manual tab groups belong to the profile

## Status

Implemented for the GPUI sidebar.

## Context

Users need named groups that survive tab closure, restart, and changes to shell,
working directory, and foreground task. A window may mix local, WSL, and SSH tabs.

## Evidence

`TabMeta` already follows the live tab across reordering and window transfers.
`TabSession` is shared by autosave and workspace export. The sidebar scrollbar
uses fixed-height rows and previously assumed a one-to-one mapping to tab indices.

## Decision

Store named, ordered group definitions independently in the profile's
`tab-groups.json`, using the existing atomic state writer. Live windows share
that list through GPUI state. Give groups randomly generated IDs and carry an
optional reference in tab metadata and the compatible session schema. A missing
reference displays as ungrouped. A definition file with a parse or read error
is preserved and edits fail visibly.

When a group has a chosen color, store it in the same profile definition. Older
entries use the automatic palette. The sidebar uses the same accent for the
header and member guides.

Project sidebar headers, members, and ungrouped tabs before applying viewport
scrolling. Keep collapse local to a window. Activating a hidden tab expands its
group. Dragging a terminal onto another group's header or member appends it to
that group and expands the destination. The Ungrouped row is a drop target even
when it has no members. Reorder within a group and keep assignment in the tab
menu. The top tab layout continues to use the flat tab order.

## Rejected alternatives

Grouping by directory or shell would change membership as terminals run.
Storing definitions exclusively in the session file would tie empty groups to
session recovery and lose group definitions when recovery is disabled. Physically
reordering all tabs on every group edit would couple a presentation concern to
runtime tab ordering and active tab indices.

## Consequences

Closing the last member leaves its group available. Deleting a group moves its
tabs to the ungrouped section and leaves their processes alive. Exports carry
membership references and use the destination profile's definitions on restore.
Documents and settings follow the existing session persistence policy.

## Validation

Projection regression coverage includes empty and collapsed groups and orphaned
membership after deletion. The Windows Release opened a seeded isolated profile,
restored both grouped and ungrouped terminals, and preserved an empty group.
The group header collapsed the active group without closing its terminal.
Creating a group produced a new profile definition. Group-local creation wrote
the new terminal's group ID into the session snapshot.

## Supersedes

None.

## Revisit when

A group must travel with an exported layout, or the top tab layout gains group
headers and explicit cross-group drop targets.

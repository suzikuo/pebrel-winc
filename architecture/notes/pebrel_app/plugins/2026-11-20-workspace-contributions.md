# Workspace contribution ownership

## Status

Implemented in Pebrel Winc.

## Context

Users want to edit sidebar appearance and automate repetitive tab actions from the UI. The
workspace already owns tab identity, group membership, and layout geometry. Existing plugins
execute one command at a time, and the directory launcher already demonstrates a native UI
contribution authorized at installation.

## Evidence

The sidebar's render positions, viewport, and group drop bands share `SidebarRowLayout`. The
runtime tab identity travels on `TabMeta` when tabs reorder. A row index can refer to another tab
by the time a background action returns.

## Decision

Use an explicit `workspace_ui` grant. Load approved declarative style beside the directory launcher
inventory. Render through the shared layout and host controls. Install review binds the parsed
manifest and exact menu source, and menu dispatch runs the verified in-memory source in a bounded
VM. A handler receives a click-scoped tab and group snapshot, and returns a typed intent. The
workspace resolves the target again and uses existing operations. The window owns pending actions,
and a plugin-library change from Settings cancels their UI continuation.

## Rejected alternatives

- Calling a script during render couples input and repaint latency to plugin execution.
- Forwarding the full runtime snapshot and arbitrary method names couples plugin grants to the
  larger automation protocol and exposes unrelated windows, terminal data, and mutations.
- Applying by row index lets delayed actions mutate replacement tabs after reorder or close.

## Consequences

A single style applies per sidebar. Menu entries from all enabled plugins can coexist. Applying a
name or group edit uses host session data, so its lifetime is independent of the contributing plugin.
New panel types and automatic rules will need their own refresh and lifecycle contracts.

## Validation

Installation tests exercise receipt binding, changed source, explicit grants, disabled dispatch and
real Luau output. Sidebar layout tests exercise plugin sizing in the viewport and drop projection.

## Supersedes

None.

## Revisit when

Multiple style providers need explicit user selection, or a contribution requires longer-lived state,
subscriptions, or a view outside an existing workspace slot.

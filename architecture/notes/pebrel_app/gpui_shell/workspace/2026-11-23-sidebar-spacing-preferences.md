# Sidebar spacing preferences

## Status

Implemented in Pebrel Winc.

## Context

Users want to tune the boundary between a group heading, its members, the next group, and
Ungrouped independently. The installed workspace contribution can also set the extra gap before
sections. Existing profiles need to retain the current layout when the new fields are absent.

## Evidence

`SidebarRowLayout` already controls the visible range, menu position and group drop hit testing.
The renderer previously applied a flex gap plus a separate padding adjustment. An override may
be smaller than either component, including zero.

## Decision

Persist optional per-boundary values as ordinary scalar preferences. Use the saved value as the
total gap for that boundary. In Auto, the density supplies tab spacing and the plugin's extra
section spacing still applies where applicable. Render the resulting gap from the same projection
used for the viewport and hit testing. The tab-to-tab pitch also uses the saved value for reorder.

## Rejected alternatives

- CSS/flex gap overrides in the view alone would leave the viewport and drag hit-testing on
  different geometry.
- An additive user adjustment would make zero ambiguous and couple the result to density or
  whichever plugin is enabled.

## Consequences

Preferences apply through the ordinary settings path. An empty value resumes inheritance. Reset
removes the override using the same settings contract as other appearance preferences.

## Validation

Settings tests cover persisted values, zero, invalid values and restore defaults. Row-layout tests
exercise all boundary combinations and drop hit testing. Windows interaction covered entering
values, rejecting an invalid value, immediate layout changes, restart and per-row reset.

## Supersedes

None.

## Revisit when

Sidebar replacements define their own row kinds or users need different spacing profiles per
workspace.

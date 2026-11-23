# Scratch terminal font size

## Status

Implemented in the Winc worktree.

## Context

Scratch shells can occupy a small part of the workspace. Their text needs its own scale while the user keeps the regular terminal at a comfortable working size.

## Evidence

The terminal view's Ctrl+wheel handler wrote the shared `font_size` preference. The main window and its scratch views both applied that value. The GPUI settings snapshot already distinguishes the configured base size from the persistent regular-terminal zoom.

## Decision

Store an optional `scratch_terminal_font_size` in the profile in logical pixels, with the same 4–64 range as Ctrl+wheel zoom. The default uses the base size resolved from the theme and configuration. Reset clears the override. Scratch views observe the published font setting so open and hidden views in each window update together. The settings page reuses the existing terminal font stepper and numeric editor.

## Rejected alternatives

Per-shell overrides would make new scratch shells start at different sizes and leave multiple values to reset. Using the ordinary terminal zoom as the default would resize scratch views whenever the ordinary terminal changed.

## Consequences

All scratch shells in the profile use the same size. Ctrl+wheel and a focused scratch view's zoom shortcuts update the scratch preference. Existing profiles resolve a base size immediately, and an explicit scratch preference survives restart.

## Validation

The settings test decodes a fractional size and checks missing, invalid, and out-of-range values. The Windows fixture checks the stored preference and the live ConPTY grid widths of ordinary and scratch panes through wheel zoom and the settings control.

## Supersedes

None.

## Revisit when

Revisit scope if a tab or a shell needs an independent font size.

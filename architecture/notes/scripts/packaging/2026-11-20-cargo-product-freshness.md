# Windows ZIP freshness follows the Cargo build

## Status

Implemented for the Windows Release ZIP.

## Context

The packaging script scanned all workspace sources and compared the newest
modification time with the GUI and CLI executable timestamps. The build can
reuse a CLI executable when the latest change only affects the GUI.

## Evidence

After a GUI-only toolbar change, the explicit Release product build succeeded
and reused `pebrel-cli.exe`. The subsequent timestamp check still rejected the
ZIP. Cargo's dependency information listed the changed toolbar code for
`pebrel.exe` only.

## Decision

A normal ZIP build runs the explicit Cargo product build and uses its freshness
result. The conservative timestamp scan applies to `-SkipBuild` invocations.
Both paths still validate the staged artifacts and their architecture.

## Rejected alternatives

Forcing a CLI rebuild on every package would add work for GUI-only changes.
Maintaining a second per-binary source list in PowerShell would duplicate Cargo's
dependency graph.

## Consequences

A verified Cargo build can package unchanged binary outputs. The offline
`-SkipBuild` path can still conservatively reject an older CLI timestamp.

## Validation

The local Release menu entry point generated a valid 18-file ZIP after the fix.
The executable and runtime entries matched the built artifacts by SHA-256.
The same CLI timestamp was rejected when invoking the packager with `-SkipBuild`.
The PowerShell packaging suite passed with the normal build path enabled.

## Supersedes

None.

## Revisit when

Revisit the timestamp fallback if Windows packaging no longer supports using
prebuilt artifacts.

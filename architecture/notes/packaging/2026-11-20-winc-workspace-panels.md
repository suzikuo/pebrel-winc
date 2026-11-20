# Optional file and Git panels in Winc builds

## Status

The Winc build once omitted the Files and Git panels for a size comparison.
The local build now includes both panels.

## Context

The user opens the focused directory in Winc and wants a simpler Pebrel workspace.
The embedded document viewer uses the same right-hand drawer as Files and Git.

## Evidence

The workspace Git and SVN panels invoke external clients. Local and remote file
views share filesystem, repository, and SFTP code with other entry points.
The existing Windows Release executable is about 127.3 MiB. Its embedded
`MapleMonoNormal-NF-CN-Regular.ttf` accounts for 19.6 MiB. The other Maple Mono
source font is absent from that executable. The recorded full Release build
spent 443.6 s including dependency work, with long tasks for Windows bindings,
shader translation, and image support.

## Decision

Select the Winc panel policy at build time through the local package runner.
The GPUI workspace omits management panels and their command and shortcut
entries in these builds. The reader still offers document information and
outlines. The official scripts use their usual configuration when invoked
outside the Winc runner.

## Rejected alternatives

Deleting the shared filesystem and Git models would require changing their
other callers. Hiding just the toolbar button would leave the command palette,
shortcuts, and drawer tabs able to open those panels.

## Consequences

The Rust dependency graph stays intact. Release dead code elimination may
remove unreachable panel-only code. Measure the output of equivalent Release
builds before assigning a size or timing savings to this change.

## Validation

Equivalent Release builds with warm dependencies measured 127.407 MiB with the
panels and 124.923 MiB without them. ZIP Deflate compressed the executable
alone to 38.003 and 36.852 MiB respectively. Both GUI builds took about five
minutes and 22 seconds. A fresh Windows profile showed the condensed title bar
and restored manual groups in the Winc build.

## Supersedes

None.

## Revisit when

A size comparison identifies a meaningful removable dependency or a supported
asset variant with acceptable terminal font coverage.

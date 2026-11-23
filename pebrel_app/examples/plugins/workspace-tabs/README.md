# Workspace Tabs

Use Settings → Plugins → Install from folder and select this directory, or run the following
from the repository root with a Pebrel Winc build that supports `workspace-ui`.

```text
pebrel plugin install pebrel_app/examples/plugins/workspace-tabs --grant workspace-ui
```

The sidebar uses 48 px group headers, 36 px rows, 10 px section spacing, a 16 px group radius,
and a 14 px trailing inset. Right-click a terminal tab and open **Plugins** to toggle its star
marker, move it into or out of the first group, or open the Shell picker for its group. The
second click on a starred title restores the automatic title. Create at least one group to try
the group command.

Disable the example in Settings → Plugins to restore the previous appearance and remove the
menu. Changes already made to names and group membership follow the normal session rules.

See the [workspace contribution API](../../../src/plugins/README.md#workspace-contributions).

# Settings surfaces and window opacity

## Status
Implemented in Pebrel Winc.

## Context
The workspace background already follows the window slider. Settings adds its own card fills, which can cover the wallpaper even at 0%. Settings navigation over a transparent background can also lose text contrast.

## Evidence
Upstream issue [#1077](https://github.com/Kuddev/pebrel/issues/1077) reports the solid Settings surfaces at 0%. Issue [#903](https://github.com/Kuddev/pebrel/issues/903) records the contrasting readability problem. The opt-in preference and regression tests are adapted from Aschenbath’s upstream [PR #1096](https://github.com/Kuddev/pebrel/pull/1096), commit `52bfa67f`.

## Decision
`pebrel_settings` reads `settings_follow_opacity` as a boolean that defaults to off and is cleared by Restore Defaults. GPUI resolves it alongside the existing runtime settings. With the preference on, the Settings navigation and the Winc Plugin Center canvas use the workspace’s existing background. Card fills, borders and shadows use the same live surface alpha. The Plugin Center blends its card tint before applying that alpha. Text, controls and popups use their own colors.

## Rejected alternatives

- Always fade Settings cards and navigation. This exposes users to the contrast issue without an explicit choice.
- Paint a second translucent navigation or plugin canvas over the workspace. At intermediate opacity the two backgrounds would composite into a stronger fill than the slider indicates.
- Fade the whole Settings element. This would also fade text, controls and focus feedback.

## Consequences
The switch takes effect immediately after a successful settings save. Its off state gives Settings navigation a solid theme background. Existing settings files can omit the new key.

## Validation
The settings and GPUI regressions passed on Windows, including default and invalid values, serialization, reset, save failures, and light and dark card styles at 0%, 50% and 100%. At 150% display scaling, a native Nord window with an isolated configuration showed the wallpaper through Settings navigation and cards at 0% with the switch on. Clicking the switch off restored the solid navigation and tinted card background.

## Supersedes
None.

## Revisit when
Upstream lands its Settings opacity fix or changes ownership of the workspace background.

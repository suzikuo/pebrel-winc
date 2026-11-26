# Backing the Settings workspace beneath wallpaper

## Status
Implemented in Pebrel Winc.

## Context
At 100% window opacity with a 45% extended image, the Settings page showed text from the Windows window behind it. The same image was visible as expected in the terminal.

## Evidence
Opening Settings replaces the normal workspace card and shell with a full-width settings view. The wallpaper was still painted over a transparent workspace. Its 45% image alpha left the rest of the Settings body open to the Windows compositor. Testing with a fully opaque wallpaper concealed the missing backing.

## Decision
Give the full-width Settings workspace a theme-colored backing below the wallpaper. It is opaque while Settings follows window opacity is off. When the user enables that preference, the backing and Settings card treatments use the live window slider without the terminal wallpaper scrim’s alpha cap. Keep the transparent component root and the terminal composition. Native AppKit video retains its existing underlay path because the video sits below the GPUI scene.

## Rejected alternatives

- Paint an opaque layer over Settings wallpaper. This would hide or darken the image regardless of its own opacity.
- Force the entire native window opaque. This would affect the terminal and the explicit Settings transparency preference.
- Increase the settings card opacity alone. Exposed areas around the cards would still reveal the window behind them.

## Consequences
Wallpaper opacity still controls the image contribution. With the preference enabled, window opacity separately controls the Settings backing. An opaque backing also covers letterboxed areas and applies while the wallpaper is loading.

## Validation
The regression loads an extended image at 45%, checks the backing during and after loading, and covers 0%, 50% and 100% window opacity with the Settings preference. A Windows native comparison at 150% scaling used the same wallpaper and an independent window behind both builds. The released build showed the window’s text through Settings. The patched build showed the wallpaper and fully covered that text. With the preference enabled, the rear window became visible again at 50% and 0% while Settings text stayed visible.

## Supersedes
The full-width workspace backdrop portion of [Settings surfaces and window opacity](2026-11-26-settings-background-opacity.md).

## Revisit when
The Settings view composition or native video layer ordering changes.

# Windows fork releases and updates

## Status

Implemented for the Pebrel Winc fork.

## Context

The fork now has its own public GitHub repository. It needs a release feed and a Windows publishing command that works with the existing local build menu.

## Evidence

The existing updater compares Cargo versions and requires a matching platform installer and SHA-256. The portable channel opens a ZIP link. Upstream automation triggers on `v*.*.*` Git tags, and the upstream installer uses a separate Windows product identity.

## Decision

Windows x64 checks `suzikuo/pebrel-winc` for `winc-vX.Y.Z` releases. API discovery, rate-limit fallback, ZIP links, and installer validation use this feed. Only the fork's marked installer layout can perform in-app installation. The fork installer uses its own setup identity, install directory, and default settings directory.

The menu confirms the repository, tag, and source commit. Publishing builds both Windows packages from the committed source, records the SHA-256 values, and verifies draft assets before making the release public. The source commit remains accessible at the published tag.

## Rejected alternatives

The upstream release feed can deliver binaries without fork changes. Reusing upstream `vX.Y.Z` tags can trigger the upstream cross-platform workflow. Reusing the upstream installer identity can replace an existing official installation.

## Consequences

The first install requires a fork package or a newly built fork binary with updater support. Portable users replace their extracted ZIP manually. If a failed upload leaves a draft, the maintainer can inspect and remove that draft and retry with the same source commit.

## Validation

The updater tests cover fork tag parsing, matching package names, and exact download URLs. The publisher verifies the remote branch, tag, release body, and asset metadata as part of its normal run.

## Supersedes

[Disable application updates in self-built Pebrel](2026-11-20-self-built-app-updates.md).

## Revisit when

The fork adds a second packaging platform or moves its release feed.

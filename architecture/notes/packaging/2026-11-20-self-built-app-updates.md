# Disable application updates in self-built Pebrel

## Status

Superseded by [Windows fork releases and updates](2026-11-20-winc-release-updates.md).

## Context

The local build menu is the source of new Debug and Release binaries. The user manages when to rebuild and replace them.

## Evidence

The application had separate startup checks in GPUI and the legacy shell. The GPUI About page could also check GitHub on demand. The installer handoff could run on startup from an earlier appointment without making a new release request.

## Decision

Ordinary builds disable update checks and installer handoffs at the application boundary. The About page displays the local version and keeps its project links. The existing `update-test-source` feature retains the updater for explicit debug acceptance runs.

## Rejected alternatives

A settings default would leave existing profiles and their scheduled updates in control of startup. Removing the updater source would also remove the debug acceptance path used to rehearse installer behavior.

## Consequences

Existing update preferences, caches, and tickets can stay on disk. Ordinary builds ignore them. The local installer and portable archive continue to use the existing packaging scripts.

## Validation

The Windows GPUI Release build and repository architecture check passed. In a disposable profile with update preferences enabled, the Application page showed the local version and release notes without update controls. The distribution regression test exercises the refusal of release checks and in-app installation in ordinary builds.

## Supersedes

None.

## Revisit when

This checkout needs to distribute builds that receive managed updates.

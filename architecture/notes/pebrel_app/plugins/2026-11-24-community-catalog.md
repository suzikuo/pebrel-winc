# Community catalog packages and local review

## Status

Implemented in Pebrel Winc.

## Context

Winc and Scratch Terminal are optional plugins. Their sources and catalog belong in an independent repository so adding a plugin does not require a new Pebrel Winc release. The existing Winc manifest references a machine-local executable and a locally compiled Explorer helper.

## Evidence

The existing installer only copies declared files, rejects undeclared grants, and verifies the reviewed launcher configuration before replacing an install. Scratch Terminal needs only its manifest. Winc needs a user-selected local executable and the focused pane's directory.

## Decision

Settings requests the fixed GitHub catalog only when opened or refreshed. Each entry selects a manifest by plugin ID and immutable Git commit. Network responses are bounded, with a finite deadline and no redirects. The host stages the manifest in a temporary directory and submits it to the existing install review, including packages with no additional grants. The staging directory lives through approval and is removed on cancellation, view destruction or completed installation.

A reviewed launcher may ask the Windows shell to open the focused folder in Explorer. The native action accepts only that directory and uses the existing WSL UNC mapping. A package may request one local executable selection. The selected path, arguments and Explorer action go through the same review as a folder installation.

## Rejected alternatives

- Opening the catalog at startup would require network I/O before the user asks to browse plugins.
- Downloading the user's Winc executable or a compiled Explorer helper would add a binary distribution and execution path to the initial catalog.
- Copying package grants into settings would duplicate the install library's review and receipt rules.

## Consequences

The catalog accepts standalone manifests. Users can still install existing Luau and theme packages through the local folder flow. The catalog does not update or enable installed plugins in the background. A replacement version requires a fresh review.

## Validation

The Windows plugin suites exercise catalog validation, manifest identity and the install library's permission check. A GPUI Debug run at 150% scaling loaded both published manifests, verified cancellation and both approvals, opened Scratch Terminal and used the Explorer action on Windows and WSL folders with spaces.

## Supersedes

None.

## Revisit when

The catalog needs to distribute packages with additional files or support a second registry.

[CmdletBinding()]
param([switch] $Plan, [string] $ExpectedCommit)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = 'suzikuo/pebrel-winc'
$repositoryUrl = "https://github.com/$repository.git"
$branch = 'winc-local'
$repo = Split-Path -Parent $PSScriptRoot
$manifest = Get-Content -LiteralPath (Join-Path $repo 'pebrel_app\Cargo.toml') -Raw -Encoding UTF8
$versionMatch = [regex]::Match($manifest, '(?m)^version\s*=\s*"([^"]+)"\s*$')
$version = $versionMatch.Groups[1].Value
if ($version -cnotmatch '\A(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\z') {
    throw 'The Pebrel package version must be X.Y.Z.'
}
$null = [version]$version
$tag = "winc-v$version"
$title = "Pebrel Winc $version"
$notesPath = Join-Path $repo "docs\release-notes\$tag.md"

function Invoke-Native([string] $Executable, [string[]] $CommandArgs) {
    & $Executable @CommandArgs
    if ($LASTEXITCODE -ne 0) { throw "$Executable $($CommandArgs[0]) failed (exit $LASTEXITCODE)." }
}

function Invoke-Git([string[]] $CommandArgs) {
    Invoke-Native 'git' (@('-C', $repo, '-c', 'core.autocrlf=false') + $CommandArgs)
}

function Assert-CleanSource {
    # Windows Git may report an upstream CRLF blob as modified under eol=lf.
    # Compare text modulo CRLF, while retaining checks for staged and untracked files.
    Invoke-Git @('diff', '--no-ext-diff', '--quiet', '--ignore-cr-at-eol', 'HEAD', '--')
    if ((Invoke-Git @('diff', '--cached', '--name-only', '--')) -or
        (Invoke-Git @('ls-files', '--others', '--exclude-standard'))) {
        throw 'Commit the source and release notes before publishing.'
    }
}

if ((Invoke-Git @('branch', '--show-current')) -cne $branch) {
    throw "Switch to $branch before publishing."
}
Assert-CleanSource
$commit = Invoke-Git @('rev-parse', 'HEAD')
Invoke-Git @('ls-files', '--error-unmatch', "docs/release-notes/$tag.md") | Out-Null
$notes = (Get-Content -LiteralPath $notesPath -Raw -Encoding UTF8).TrimEnd()
if (!$notes) { throw "Release notes are empty: $notesPath" }
if ($Plan) {
    return [pscustomobject]@{ Repository = $repository; Tag = $tag; Commit = $commit }
}
if ($commit -cne $ExpectedCommit) { throw 'The source changed. Open the menu and confirm again.' }

$gh = (Get-Command gh -CommandType Application).Source
$releasesEndpoint = "repos/$repository/releases"
# The authenticated collection exposes drafts that use the same tag.
function Get-ExistingRelease {
    $rows = Invoke-Native $gh @('api', '--paginate', $releasesEndpoint, '--jq', '.[] | @base64')
    foreach ($row in $rows) {
        $entry = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($row)) | ConvertFrom-Json
        if ($entry.tag_name -ceq $tag) { return $entry }
    }
}

if (Get-ExistingRelease) { throw "Release $tag already exists. Inspect it in $repository before retrying." }
$remoteTags = Invoke-Git @('ls-remote', '--refs', '--tags', $repositoryUrl, 'refs/tags/winc-v*')
foreach ($ref in $remoteTags) {
    if ($ref -match '^([a-f0-9]{40})\s+refs/tags/winc-v([0-9]+\.[0-9]+\.[0-9]+)$') {
        $remoteCommit = $Matches[1]
        $remoteVersion = $Matches[2]
        if ([version]$remoteVersion -gt [version]$version -or
            ($remoteVersion -ceq $version -and $remoteCommit -cne $commit)) {
            throw "Version $version conflicts with remote tag winc-v$remoteVersion. Increase the Cargo version and commit it."
        }
    }
}
$remoteBranch = Invoke-Git @('ls-remote', '--heads', $repositoryUrl, "refs/heads/$branch")
if ($remoteBranch -cne "$commit`trefs/heads/$branch") {
    throw "Push $commit to $repository/$branch before publishing."
}

$output = Join-Path $repo "dist\$tag"
$target = Join-Path $repo 'target\winc-publish'
& (Join-Path $PSScriptRoot 'prepare-windows-runtime.ps1') `
    -Destination (Join-Path $repo 'assets\windows\conhost') -Architecture x64 | Out-Host
& (Join-Path $PSScriptRoot 'build-installer.ps1') -Version $version -WincInstall `
    -TargetDirectory $target -OutputDirectory $output -Force | Out-Host
& (Join-Path $PSScriptRoot 'package-release.ps1') -Version $version `
    -TargetDirectory $target -OutputDirectory $output -Force | Out-Host
if ((Invoke-Git @('rev-parse', 'HEAD')) -cne $commit) {
    throw 'Source changed during packaging.'
}
Assert-CleanSource
$checksums = Join-Path $output 'SHA256SUMS'
$files = @(
    (Join-Path $output "Pebrel-v$version-windows-x64-setup.exe"),
    (Join-Path $output "Pebrel-v$version-windows-x64.zip")
)
$lines = foreach ($file in $files) {
    $name = Split-Path -Leaf $file
    $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    "${hash}  $name"
}
[IO.File]::WriteAllLines($checksums, [string[]]$lines, [Text.Encoding]::ASCII)
$files += $checksums
$bodyPath = Join-Path $output 'release-body.md'
$body = $notes + "`n`n## SHA256`n`n" + (($lines | ForEach-Object {
    $hash, $name = $_ -split '  ', 2
    '- `{0}`: `{1}`' -f $name, $hash
}) -join "`n") + "`n"
[IO.File]::WriteAllText($bodyPath, $body, [Text.UTF8Encoding]::new($false))

# Create the tag without force. A failed upload can never replace a released tag.
# The per-command helper uses the existing GitHub CLI login.
$helper = '!"' + $gh.Replace('\', '/') + '" auth git-credential'
Invoke-Git @('-c', 'credential.helper=', '-c', "credential.https://github.com.helper=$helper",
    'push', $repositoryUrl, "${commit}:refs/tags/$tag") | Write-Host
$expectedTag = "$commit`trefs/tags/$tag"
if ((Invoke-Git @('ls-remote', '--refs', '--tags', $repositoryUrl, "refs/tags/$tag")) -cne $expectedTag) {
    throw "Remote tag $tag does not match $commit."
}
Invoke-Native $gh (@('release', 'create', $tag, '--repo', $repository,
    '--verify-tag', '--draft', '--title', $title, '--notes-file', $bodyPath) + $files) | Write-Host
$draft = Get-ExistingRelease
if (!$draft -or !$draft.draft -or $draft.prerelease -or $draft.name -cne $title -or
    $draft.body.TrimEnd() -cne $body.TrimEnd() -or @($draft.assets).Count -ne $files.Count) {
    throw "Release draft $tag metadata did not match. Inspect the draft on GitHub."
}
foreach ($file in $files) {
    $local = Get-Item -LiteralPath $file
    $assets = @($draft.assets | Where-Object { $_.name -ceq $local.Name })
    $digest = 'sha256:' + (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($assets.Count -ne 1 -or $assets[0].state -cne 'uploaded' -or $assets[0].label -or
        $assets[0].size -ne $local.Length -or $assets[0].digest -cne $digest) {
        throw "Release draft asset verification failed: $($local.Name)"
    }
}
$published = Invoke-Native $gh @('api', '--method', 'PATCH', "$releasesEndpoint/$($draft.id)",
    '-F', 'draft=false', '-f', 'make_latest=true') | ConvertFrom-Json
$latest = Invoke-Native $gh @('api', "$releasesEndpoint/latest") | ConvertFrom-Json
if ($published.draft -or $published.prerelease -or $published.tag_name -cne $tag -or
    $latest.id -ne $draft.id) {
    throw "GitHub did not verify $tag as the latest public release."
}
Write-Output $published.html_url

<#
.SYNOPSIS
    Run or package Pebrel on Windows x64 from this checkout.
.DESCRIPTION
    Run opens an existing executable with the PebrelWincNative settings profile.
    Debug builds incrementally and creates a ZIP for use on the build machine.
    Release builds the product and creates a portable ZIP in dist\.
    Installer builds the product and creates an Inno Setup installer in dist\.
    Close the selected build before rebuilding its executable on Windows.
.PARAMETER Action
    Run, Debug, Release, or Installer. Defaults to Run.
.PARAMETER Profile
    The existing build to open with Run. Defaults to release.
.PARAMETER Force
    Replace the existing package for the selected action.
.EXAMPLE
    .\pebrel-local.ps1
.EXAMPLE
    .\pebrel-local.ps1 Debug -Force
.EXAMPLE
    .\pebrel-local.ps1 Run -Profile debug
.EXAMPLE
    .\pebrel-local.ps1 Release
.EXAMPLE
    .\pebrel-local.ps1 Installer
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('Run', 'Debug', 'Release', 'Installer')]
    [string] $Action = 'Run',

    [ValidateSet('debug', 'release')]
    [string] $Profile = 'release',

    [switch] $Force
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = $PSScriptRoot
$scripts = Join-Path $repo 'scripts'
$targetDirectory = Join-Path $repo 'target'
$outputDirectory = Join-Path $repo 'dist'

if ($Action -eq 'Run') {
    $binaryDirectory = Join-Path $targetDirectory $Profile.ToLowerInvariant()
    $executable = Join-Path $binaryDirectory 'pebrel.exe'
    if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
        throw "Missing $executable. Build it with .\pebrel-local.ps1 $Profile first."
    }

    $profileDirectory = Join-Path $env:LOCALAPPDATA 'PebrelWincNative'
    $previousPebrelDirectory = $env:PEBREL_CONFIG_DIR
    $previousNebulaDirectory = $env:NEBULA_CONFIG_DIR
    try {
        $env:PEBREL_CONFIG_DIR = $profileDirectory
        $env:NEBULA_CONFIG_DIR = $profileDirectory
        # A command-line directory requests a fresh terminal and skips workspace restore.
        Start-Process -FilePath $executable -WorkingDirectory $repo
    } finally {
        $env:PEBREL_CONFIG_DIR = $previousPebrelDirectory
        $env:NEBULA_CONFIG_DIR = $previousNebulaDirectory
    }
    return
}

$debugZip = Join-Path $outputDirectory 'Pebrel-debug-local-windows-x64.zip'
if ($Action -eq 'Debug' -and (Test-Path -LiteralPath $debugZip) -and -not $Force) {
    throw "Package already exists at $debugZip. Pass -Force to replace it."
}

& (Join-Path $scripts 'prepare-windows-runtime.ps1') `
    -Destination (Join-Path $repo 'assets\windows\conhost') -Architecture x64

switch ($Action) {
    'Release' {
        & (Join-Path $scripts 'package-release.ps1') -TargetDirectory $targetDirectory `
            -OutputDirectory $outputDirectory -Architecture x64 -Force:$Force
    }
    'Installer' {
        & (Join-Path $scripts 'build-installer.ps1') -WincInstall -TargetDirectory $targetDirectory `
            -OutputDirectory $outputDirectory -Architecture x64 -Force:$Force
    }
    'Debug' {
        & (Join-Path $scripts 'build-windows-product.ps1') -Configuration debug `
            -TargetDirectory $targetDirectory
        $binaryDirectory = Join-Path $targetDirectory 'debug'
        . (Join-Path $scripts 'windows-package-architecture.ps1')
        Assert-WindowsPackageArchitecture -Root $binaryDirectory -Architecture x64

        # Debug GPUI reads HLSL from the build machine's Cargo checkout.
        # Keep this archive for local use and use the release script for distribution.
        $entries = [ordered]@{
            'pebrel.exe'              = 'pebrel.exe'
            'pebrel.com'              = 'pebrel-cli.exe'
            'runtime/pebrel-hook.exe' = 'pebrel-hook.exe'
            'runtime/conpty.dll'      = 'conpty.dll'
            'runtime/OpenConsole.exe' = 'OpenConsole.exe'
        }
        Add-Type -AssemblyName System.IO.Compression
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
        $tempZip = Join-Path $outputDirectory ".pebrel-debug-$([guid]::NewGuid().ToString('N')).tmp.zip"
        try {
            $archive = [IO.Compression.ZipFile]::Open($tempZip, [IO.Compression.ZipArchiveMode]::Create)
            try {
                foreach ($entry in $entries.GetEnumerator()) {
                    $path = Join-Path $binaryDirectory $entry.Value
                    [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                        $archive, $path, $entry.Key, [IO.Compression.CompressionLevel]::Fastest
                    ) | Out-Null
                }
            } finally {
                $archive.Dispose()
            }
            if ($Force -and (Test-Path -LiteralPath $debugZip -PathType Leaf)) {
                [IO.File]::Replace($tempZip, $debugZip, [NullString]::Value)
            } else {
                [IO.File]::Move($tempZip, $debugZip)
            }
            Write-Output "Created $debugZip for local use on this build machine."
        } finally {
            if (Test-Path -LiteralPath $tempZip) { Remove-Item -LiteralPath $tempZip -Force }
        }
    }
}

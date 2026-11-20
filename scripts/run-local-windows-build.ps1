[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('Debug', 'Release', 'Installer', 'Publish')]
    [string] $Action,
    [ValidatePattern('^[a-f0-9]{40}$')]
    [string] $ExpectedCommit,
    [switch] $Force
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

try {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

public static class PebrelBuildCpuLimit
{
    private const int JobObjectCpuRateControlInformation = 15;
    private const uint Enable = 0x1;
    private const uint HardCap = 0x4;

    [StructLayout(LayoutKind.Sequential)]
    private struct CpuRateControl
    {
        public uint Flags;
        public uint Rate;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern SafeFileHandle CreateJobObject(IntPtr attributes, string name);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(SafeFileHandle job, int infoClass,
        ref CpuRateControl info, uint size);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(SafeFileHandle job, IntPtr process);

    [DllImport("kernel32.dll")]
    private static extern IntPtr GetCurrentProcess();

    public static void Apply()
    {
        // A job survives closing its handle while it still owns a running process.
        using (SafeFileHandle job = CreateJobObject(IntPtr.Zero, null))
        {
            if (job.IsInvalid)
                throw new Win32Exception(Marshal.GetLastWin32Error(), "Could not create the build CPU job.");

            var limit = new CpuRateControl { Flags = Enable | HardCap, Rate = 8000 };
            if (!SetInformationJobObject(job, JobObjectCpuRateControlInformation, ref limit, 8))
                throw new Win32Exception(Marshal.GetLastWin32Error(), "Could not set the build CPU cap.");

            if (!AssignProcessToJobObject(job, GetCurrentProcess()))
                throw new Win32Exception(Marshal.GetLastWin32Error(), "Could not apply the build CPU cap.");
        }
    }
}
'@
    [PebrelBuildCpuLimit]::Apply()
    Write-Host 'Windows build CPU limit: 80%'

    if ($Action -eq 'Publish') {
        & (Join-Path $PSScriptRoot 'publish-winc.ps1') -ExpectedCommit $ExpectedCommit
    } else {
        $runner = Join-Path (Split-Path -Parent $PSScriptRoot) 'pebrel-local.ps1'
        & $runner -Action $Action -Force:$Force
    }
} catch {
    Write-Output $_.Exception.Message
    exit 1
}

<#
.SYNOPSIS
  Purges the Windows standby list before a cold profiling run (plan 2026-11-21-engine-profile, spec 7.4).
  Run it yourself from an ELEVATED PowerShell: pwsh -NoProfile -File tools\profile-purge.ps1 -RunRoot <folder>

.DESCRIPTION
  -RunRoot <folder>  an existing run folder. The script writes <RunRoot>\purge-<yyyyMMdd-HHmmss>.json, created
                     new (it fails if the file exists), with the standby-list counters before and after.
  Not elevated: prints "run this from an elevated PowerShell" and exits 1. Otherwise it enables
  SeProfileSingleProcessPrivilege and calls NtSetSystemInformation(SystemMemoryListInformation = 80,
  MemoryPurgeStandbyList = 4). It never elevates itself, downloads or installs anything, and it deletes
  nothing.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RunRoot
)

$ErrorActionPreference = 'Stop'

$id = [Security.Principal.WindowsIdentity]::GetCurrent()
if (-not (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host 'run this from an elevated PowerShell'
    exit 1
}
if (-not (Test-Path -LiteralPath $RunRoot -PathType Container)) {
    Write-Host "RunRoot does not exist: $RunRoot (the runner creates it; pass the same folder)"
    exit 2
}
$RunRoot = [IO.Path]::GetFullPath($RunRoot)

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class StandbyPurge {
    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    struct TokPriv1Luid { public int Count; public long Luid; public int Attr; }
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool OpenProcessToken(IntPtr h, int acc, ref IntPtr tok);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool LookupPrivilegeValue(string host, string name, ref long luid);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool AdjustTokenPrivileges(IntPtr tok, bool disall, ref TokPriv1Luid newst, int len, IntPtr prev, IntPtr relen);
    [DllImport("kernel32.dll")]
    static extern IntPtr GetCurrentProcess();
    [DllImport("ntdll.dll")]
    static extern int NtSetSystemInformation(int infoClass, IntPtr info, int length);

    public static void EnablePrivilege(string name) {
        IntPtr tok = IntPtr.Zero;
        if (!OpenProcessToken(GetCurrentProcess(), 0x28, ref tok)) throw new Exception("OpenProcessToken failed: " + Marshal.GetLastWin32Error());
        TokPriv1Luid tp = new TokPriv1Luid(); tp.Count = 1; tp.Attr = 2;
        if (!LookupPrivilegeValue(null, name, ref tp.Luid)) throw new Exception("LookupPrivilegeValue failed: " + Marshal.GetLastWin32Error());
        if (!AdjustTokenPrivileges(tok, false, ref tp, 0, IntPtr.Zero, IntPtr.Zero)) throw new Exception("AdjustTokenPrivileges failed: " + Marshal.GetLastWin32Error());
        if (Marshal.GetLastWin32Error() != 0) throw new Exception("the privilege was not granted: " + Marshal.GetLastWin32Error());
    }

    // SystemMemoryListInformation = 80, command MemoryPurgeStandbyList = 4.
    public static int PurgeStandby() {
        IntPtr p = Marshal.AllocHGlobal(4);
        try { Marshal.WriteInt32(p, 4); return NtSetSystemInformation(80, p, 4); }
        finally { Marshal.FreeHGlobal(p); }
    }
}
'@

function Get-Standby {
    $names = '\Memory\Standby Cache Core Bytes', '\Memory\Standby Cache Normal Priority Bytes', '\Memory\Standby Cache Reserve Bytes', '\Memory\Free & Zero Page List Bytes'
    $o = [ordered]@{}
    foreach ($s in (Get-Counter -Counter $names -SampleInterval 1 -MaxSamples 1).CounterSamples) { $o[($s.Path -replace '^.*\\memory\\', '')] = [int64]$s.CookedValue }
    $o
}

$before = Get-Standby
[StandbyPurge]::EnablePrivilege('SeProfileSingleProcessPrivilege')
$status = [StandbyPurge]::PurgeStandby()
if ($status -ne 0) { Write-Host ('NtSetSystemInformation returned 0x{0:X8}' -f $status); exit 3 }
Start-Sleep -Seconds 2
$after = Get-Standby

$stamp = Get-Date
$path = Join-Path $RunRoot ('purge-' + $stamp.ToString('yyyyMMdd-HHmmss') + '.json')
$json = ([ordered]@{ stamp = $stamp.ToString('o'); status = $status; before = $before; after = $after } | ConvertTo-Json -Depth 4) + "`n"
$fs = [IO.File]::Open($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
try {
    $w = New-Object IO.StreamWriter($fs, (New-Object Text.UTF8Encoding($false)))
    $w.Write($json)
    $w.Flush()
} finally { $fs.Dispose() }
Write-Host "standby list purged; wrote $path"
exit 0

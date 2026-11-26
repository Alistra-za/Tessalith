<#
.SYNOPSIS
  Evicts a corpus's files from the Windows file cache before a cold profiling run (plan 2026-11-21-engine-profile,
  ruling EP-C5). No admin needed: opening a file with FILE_FLAG_NO_BUFFERING drops its cached pages.

.DESCRIPTION
  -RunRoot <folder> -Corpus <folder or list file>
      Opens every corpus file with FILE_FLAG_NO_BUFFERING (0x20000000), reads its first 4 KB into a page-aligned
      buffer and closes it. A folder corpus is every file below it; a list file holds one absolute path per line.
      Then writes <RunRoot>\evict-<yyyyMMdd-HHmmss>.json, created new (it fails if the file exists): stamp time,
      method "per-file-no-buffering", corpus (the full path as given), corpusKind, file count, bytes. A file that
      cannot be opened or read means no stamp and exit 1. profile-run.ps1 -Cohort cold-fresh accepts the stamp
      like a purge-*.json; only a folder stamp whose corpus equals the run's -Corpus can match.
  -SelfTest
      Writes a 256 MB file in a new temp folder and flushes it, reads it hot, evicts it, reads it again, and
      compares the raw cumulative disk read bytes (Win32_PerfRawData_PerfDisk_PhysicalDisk _Total,
      DiskReadBytesPersec) of each read: the hot read must stay under 25 % of the file and the read after the
      eviction must reach 90 %. Deletes the 256 MB file; the temp folder and its small stamp are left in place.
  RunRoot is refused when it overlaps a protected path (spec 7.8, profile-paths.ps1). The corpus is only read, so
  it may sit inside one (the real corpora do). The script writes nothing but the stamp, and deletes nothing but
  its own self-test file.
#>
[CmdletBinding(DefaultParameterSetName = 'Run')]
param(
    [Parameter(ParameterSetName = 'Run', Mandatory)][string]$RunRoot,
    [Parameter(ParameterSetName = 'Run', Mandatory)][string]$Corpus,
    [Parameter(ParameterSetName = 'SelfTest', Mandatory)][switch]$SelfTest
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

. (Join-Path $PSScriptRoot 'profile-paths.ps1')

# FILE_FLAG_NO_BUFFERING: the open that drops the file's cached pages.
$NoBufferingFlag = [uint32]0x20000000

if (-not ('NoBufferEvict' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class NoBufferEvict {
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern SafeFileHandle CreateFileW(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool ReadFile(SafeFileHandle h, IntPtr buffer, uint toRead, out uint read, IntPtr overlapped);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr VirtualAlloc(IntPtr addr, UIntPtr size, uint type, uint protect);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool VirtualFree(IntPtr addr, UIntPtr size, uint type);

    // Opens the file read-only with `flags`, reads its first 4 KB into a page-aligned buffer (an unbuffered read
    // needs an aligned address and size) and closes it. Returns null, or the error.
    public static string Touch(string path, uint flags) {
        IntPtr buf = VirtualAlloc(IntPtr.Zero, (UIntPtr)4096, 0x3000, 0x04);
        if (buf == IntPtr.Zero) return "VirtualAlloc failed: Win32 error " + Marshal.GetLastWin32Error();
        try {
            using (SafeFileHandle h = CreateFileW(path, 0x80000000, 7, IntPtr.Zero, 3, flags, IntPtr.Zero)) {
                if (h.IsInvalid) return "open failed: Win32 error " + Marshal.GetLastWin32Error();
                uint read;
                if (!ReadFile(h, buf, 4096, out read, IntPtr.Zero)) return "read failed: Win32 error " + Marshal.GetLastWin32Error();
            }
            return null;
        } finally { VirtualFree(buf, UIntPtr.Zero, 0x8000); }
    }
}
'@
}

function Write-NewText([string]$path, [string]$text) {
    $fs = [IO.File]::Open($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try {
        $w = New-Object IO.StreamWriter($fs, (New-Object Text.UTF8Encoding($false)))
        $w.Write($text)
        $w.Flush()
    } finally { $fs.Dispose() }
}

# The corpus's files: every file below a folder, or the absolute paths of a list file (blank lines skipped).
function Get-CorpusFiles([string]$corpus) {
    if (Test-Path -LiteralPath $corpus -PathType Container) {
        return [pscustomobject]@{ Kind = 'folder'; Files = @([IO.Directory]::EnumerateFiles($corpus, '*', [IO.SearchOption]::AllDirectories)) }
    }
    if (-not (Test-Path -LiteralPath $corpus -PathType Leaf)) { throw "corpus not found: $corpus" }
    $files = @(Get-Content -LiteralPath $corpus | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    $relative = @($files | Where-Object { -not [IO.Path]::IsPathFullyQualified($_) })
    if ($relative.Count) { throw "the list file holds relative paths, e.g. $($relative[0])" }
    [pscustomobject]@{ Kind = 'list'; Files = $files }
}

# Evicts every file; returns the count, bytes and failures. Writes nothing.
function Invoke-EvictFiles([string[]]$files, [uint32]$flags) {
    $bytes = [int64]0
    $failed = New-Object System.Collections.Generic.List[string]
    foreach ($f in $files) {
        try { $bytes += ([IO.FileInfo]$f).Length } catch { }
        $err = [NoBufferEvict]::Touch($f, $flags)
        if ($err) { $failed.Add("$f`t$err") }
    }
    [pscustomobject]@{ Files = $files.Count; Bytes = $bytes; Failed = $failed.ToArray() }
}

# Evicts the corpus and writes the stamp. Returns the stamp path; throws, with no stamp, on any failure.
function Invoke-Evict([string]$runRoot, [string]$corpus, [uint32]$flags) {
    $why = Get-RunRootProblem $runRoot (Get-ProtectedList)
    if ($why) { throw "refused: RunRoot $runRoot $why" }
    if (-not (Test-Path -LiteralPath $runRoot -PathType Container)) { throw "RunRoot does not exist: $runRoot (the runner creates it; pass the same folder)" }
    $runRoot = [IO.Path]::GetFullPath($runRoot)
    $corpus = [IO.Path]::GetFullPath($corpus).TrimEnd('\')
    $c = Get-CorpusFiles $corpus
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $r = Invoke-EvictFiles $c.Files $flags
    $sw.Stop()
    if ($r.Failed.Count) {
        throw ("{0} of {1} file(s) could not be evicted, no stamp written; first: {2}" -f $r.Failed.Count, $r.Files, $r.Failed[0])
    }
    $stamp = Get-Date
    $path = Join-Path $runRoot ('evict-' + $stamp.ToString('yyyyMMdd-HHmmss') + '.json')
    $json = [ordered]@{
        stamp = $stamp.ToString('o'); method = 'per-file-no-buffering'; corpus = $corpus; corpusKind = $c.Kind
        files = $r.Files; bytes = $r.Bytes; ms = $sw.ElapsedMilliseconds
    } | ConvertTo-Json -Depth 3
    Write-NewText $path ($json + "`n")
    $path
}

function Get-TotalDiskReadBytes {
    [uint64](Get-CimInstance Win32_PerfRawData_PerfDisk_PhysicalDisk -Filter "Name='_Total'").DiskReadBytesPersec
}

# Reads the whole file through the cache; returns the disk bytes read meanwhile.
function Measure-Read([string]$path) {
    $buf = New-Object byte[] (4MB)
    $before = Get-TotalDiskReadBytes
    $fs = [IO.File]::OpenRead($path)
    try { while ($fs.Read($buf, 0, $buf.Length) -gt 0) { } } finally { $fs.Dispose() }
    [int64]((Get-TotalDiskReadBytes) - $before)
}

function Invoke-SelfTest {
    $t = Join-Path ([IO.Path]::GetTempPath()) ('sdc-evict-selftest-' + (Get-Date).ToString('yyyyMMdd-HHmmss'))
    New-Item -ItemType Directory -Path $t | Out-Null
    $corpusDir = Join-Path $t 'corpus'
    New-Item -ItemType Directory -Path $corpusDir | Out-Null
    $results = New-Object System.Collections.Generic.List[object]
    function Case([string]$name, [scriptblock]$test) {
        try { $ok = [bool](& $test); $err = '' } catch { $ok = $false; $err = $_.Exception.Message }
        $results.Add([pscustomobject]@{ Name = $name; Ok = $ok })
        '{0} {1}{2}' -f ($(if ($ok) { 'PASS' } else { 'FAIL' })), $name, ($(if ($err) { "  ($err)" } else { '' }))
    }

    $big = Join-Path $corpusDir 'big.bin'
    $size = 256MB
    try {
        $rng = [Random]::new(20261126)
        $chunk = New-Object byte[] (4MB)
        $fs = [IO.File]::Open($big, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        try {
            for ($i = 0; $i -lt $size / $chunk.Length; $i++) { $rng.NextBytes($chunk); $fs.Write($chunk, 0, $chunk.Length) }
            $fs.Flush($true)
        } finally { $fs.Dispose() }
        [void](Measure-Read $big)   # the first read after the write makes sure every page is cached
        $hot = Measure-Read $big
        'hot read: {0:N0} disk bytes for a {1:N0}-byte file' -f $hot, $size
        Case 'eviction of a folder corpus writes an evict stamp with its file count and bytes' {
            $script:stampPath = Invoke-Evict $t $corpusDir $NoBufferingFlag
            $s = Get-Content -LiteralPath $script:stampPath -Raw | ConvertFrom-Json
            ([IO.Path]::GetFileName($script:stampPath) -match '^evict-\d{8}-\d{6}\.json$') -and $s.method -eq 'per-file-no-buffering' -and
            $s.files -eq 1 -and $s.bytes -eq $size -and $s.corpusKind -eq 'folder' -and $s.corpus -eq [IO.Path]::GetFullPath($corpusDir)
        }
        $cold = Measure-Read $big
        'read after the eviction: {0:N0} disk bytes' -f $cold
        Case 'the hot read came from the cache (under 25 % of the file from disk; else the disk was busy, rerun)' { $hot -lt 0.25 * $size }
        Case 'the read after the eviction came from disk (at least 90 % of the file)' { $cold -ge 0.9 * $size }
        Case 'a second stamp in the same second is not overwritten' {
            $p = $script:stampPath
            $threw = $false
            try { Write-NewText $p 'x' } catch { $threw = $true }
            $threw -and (Get-Content -LiteralPath $p -Raw).Contains('per-file-no-buffering')
        }
        Case 'a list file corpus is evicted, with relative paths refused' {
            $list = Join-Path $t 'list.txt'
            Write-NewText $list ($big + "`n`n")
            $c = Get-CorpusFiles $list
            $rel = Join-Path $t 'rel.txt'
            Write-NewText $rel "corpus\big.bin`n"
            $threw = $false
            try { [void](Get-CorpusFiles $rel) } catch { $threw = $true }
            $r = Invoke-EvictFiles $c.Files $NoBufferingFlag
            $c.Kind -eq 'list' -and $r.Files -eq 1 -and $r.Failed.Count -eq 0 -and $threw
        }
        Case 'a corpus file that cannot be opened means no stamp' {
            $rr = Join-Path $t 'rr-missing'
            New-Item -ItemType Directory -Path $rr | Out-Null
            $list = Join-Path $t 'missing.txt'
            Write-NewText $list ((Join-Path $t 'no-such-file.bin') + "`n")
            $threw = $false
            try { [void](Invoke-Evict $rr $list $NoBufferingFlag) } catch { $threw = $_.Exception.Message -like '*could not be evicted*' }
            $threw -and -not @(Get-ChildItem -LiteralPath $rr -Filter 'evict-*.json')
        }
        Case 'a protected RunRoot is refused before anything is read or written' {
            $threw = $false
            try { [void](Invoke-Evict 'G:\Acherus' $corpusDir $NoBufferingFlag) } catch { $threw = $_.Exception.Message -like 'refused: RunRoot*' }
            $threw
        }
        Case 'the script names no function after a built-in alias' {
            $text = [IO.File]::ReadAllText($PSCommandPath)
            $names = [regex]::Matches($text, '(?m)^\s*function\s+([\w-]+)') | ForEach-Object { $_.Groups[1].Value }
            $aliases = @(Get-Alias | ForEach-Object Name)
            -not @($names | Where-Object { $aliases -contains $_ })
        }
    } finally {
        if (Test-Path -LiteralPath $big) { [IO.File]::Delete($big) }
    }
    Case 'the 256 MB self-test file is deleted' { -not (Test-Path -LiteralPath $big) }
    $failed = @($results | Where-Object { -not $_.Ok })
    "self-test folder (left in place): $t"
    '{0} cases, {1} failed' -f $results.Count, $failed.Count
    if ($failed.Count) { exit 1 }
    exit 0
}

if ($SelfTest) { Invoke-SelfTest }

try {
    $p = Invoke-Evict $RunRoot $Corpus $NoBufferingFlag
    Write-Host "corpus evicted from the file cache; wrote $p"
    exit 0
} catch {
    Write-Host $_.Exception.Message
    exit 1
}

<#
.SYNOPSIS
  Engine profiling runner (plan 2026-11-21-engine-profile, Task 4). One attempt per call. NOTHING IN THIS SCRIPT
  EVER DELETES, EMPTIES, MOVES OR OVERWRITES A FILE OR FOLDER (EP-R3): every folder is created new (the create
  fails when it exists) and every file it writes is created new. Old runs are removed by hand.

.DESCRIPTION
  Three parameter sets, each with only its own parameters (PowerShell rejects a mix).

  Run:       -Exe <sdc.exe> -Corpus <folder> -Label <name>
             -Cohort <warmup|warm-fresh|warm-noop|warm-stale|cold-fresh|overhead|parity>
             -Build <release|base|release-off|profiling> [-RunRoot <folder>] [-Set <string[]>]
             [-CopyFrom <attempt name>] [-StaleEdge <clamp|wrap>] [-Samply <samply.exe>] [-Calibrate] [-DryRun]
             [-ExpectedBytes <n>]  size of the expected output; required for the first attempt of a corpus and
             build in a RunRoot, later ones size the free-space check from the newest earlier attempt's out.
             Nothing reads the corpus outside the command.
  Expected:  -Expected -Exe <base sdc.exe> -Corpus <folder> -Label <name> -RunRoot <folder> [-Set <string[]>]
             Dry run of the base build into a new empty folder under RunRoot with frozen APPDATA. Writes, each
             created new: expected-<Label>.txt, planned-not-written-<Label>.txt, expected-<Label>-events.txt.
  SelfTest:  -SelfTest [-RunRoot <folder>]   Works under a new folder <RunRoot or temp>\selftest-<stamp>, which
             it leaves in place.

  RunRoot defaults to F:\ai\dds\bench\profile\runs\<yyyyMMdd>-<Label> and is refused when it overlaps a
  protected path (spec 7.8). Each attempt gets <RunRoot>\<yyyyMMdd-HHmmss>-<Label>-<Cohort>-<k>-<8 hex> and
  <attempt>\out. Written per attempt: meta.json, sampler.csv, contention.csv, stderr.log, stdout.log,
  hashes.csv, last-run-<attempt>.json, and for warm-noop files-before.csv and files-after.csv.
  -CopyFrom: warm-noop and warm-stale copy a finished, uncontaminated warm-fresh attempt's out (same RunRoot,
  same corpus and exe) with robocopy, timestamps kept; the source is never written.
  -Calibrate: the first attempt of a session, on a fixture project; writes <RunRoot>\baseline.json. Later
  attempts refuse to run without it.
  -DryRun: prints the quiet check, preconditions, the resolved command and folders; runs nothing.
  The script never elevates, downloads or installs anything. Cold attempts need an owner-run
  tools\profile-purge.ps1 stamp (purge-*.json under 15 minutes old, not yet used).
#>
[CmdletBinding(DefaultParameterSetName = 'Run')]
param(
    [Parameter(ParameterSetName = 'Run', Mandatory)]
    [Parameter(ParameterSetName = 'Expected', Mandatory)]
    [string]$Exe,
    [Parameter(ParameterSetName = 'Run', Mandatory)]
    [Parameter(ParameterSetName = 'Expected', Mandatory)]
    [string]$Corpus,
    [Parameter(ParameterSetName = 'Run', Mandatory)]
    [Parameter(ParameterSetName = 'Expected', Mandatory)]
    [AllowEmptyString()][string]$Label,
    [Parameter(ParameterSetName = 'Run', Mandatory)]
    [ValidateSet('warmup', 'warm-fresh', 'warm-noop', 'warm-stale', 'cold-fresh', 'overhead', 'parity')]
    [string]$Cohort,
    [Parameter(ParameterSetName = 'Run', Mandatory)]
    [ValidateSet('release', 'base', 'release-off', 'profiling')]
    [string]$Build,
    [Parameter(ParameterSetName = 'Run')]
    [Parameter(ParameterSetName = 'Expected', Mandatory)]
    [Parameter(ParameterSetName = 'SelfTest')]
    [string]$RunRoot,
    [Parameter(ParameterSetName = 'Run')]
    [Parameter(ParameterSetName = 'Expected')]
    [string[]]$Set = @(),
    [Parameter(ParameterSetName = 'Run')]
    [string]$CopyFrom,
    [Parameter(ParameterSetName = 'Run')]
    [ValidateSet('clamp', 'wrap')]
    [string]$StaleEdge = 'clamp',
    [Parameter(ParameterSetName = 'Run')]
    [string]$Samply,
    [Parameter(ParameterSetName = 'Run')]
    [switch]$Calibrate,
    [Parameter(ParameterSetName = 'Run')]
    [switch]$DryRun,
    [Parameter(ParameterSetName = 'Expected', Mandatory)]
    [switch]$Expected,
    [Parameter(ParameterSetName = 'SelfTest', Mandatory)]
    [switch]$SelfTest,
    [Parameter(ParameterSetName = 'SelfTest', DontShow)]
    [string[]]$ProtectedForTest,
    [Parameter(ParameterSetName = 'Run')]
    [int64]$ExpectedBytes
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

# ---------------------------------------------------------------- paths and protection (spec 7.8)

. (Join-Path $PSScriptRoot 'profile-paths.ps1')

function Test-Label([AllowEmptyString()][string]$l) {
    $l -cmatch '^[A-Za-z0-9][A-Za-z0-9-]{0,31}\z'
}

function Get-AttemptName([string]$label, [string]$cohort, [int]$k) {
    $hex = [guid]::NewGuid().ToString('N').Substring(0, 8)
    '{0}-{1}-{2}-{3}-{4}' -f (Get-Date).ToString('yyyyMMdd-HHmmss'), $label, $cohort, $k, $hex
}

# Creates a folder that must not exist: without the force switch New-Item fails on an existing folder.
function New-NewDir([string]$p) {
    New-Item -ItemType Directory -Path $p -ErrorAction Stop | Out-Null
}

function Write-NewFile([string]$path, [string]$text) {
    $fs = [IO.File]::Open($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try {
        $w = New-Object IO.StreamWriter($fs, (New-Object Text.UTF8Encoding($false)))
        $w.Write($text)
        $w.Flush()
    } finally { $fs.Dispose() }
}

function Write-NewJson([string]$path, $obj) {
    Write-NewFile $path (($obj | ConvertTo-Json -Depth 8) + "`n")
}

function Read-Json([string]$path) {
    Get-Content -LiteralPath $path -Raw | ConvertFrom-Json -DateKind String
}

function Get-Prop($obj, [string]$name) {
    if ($null -ne $obj -and $obj.PSObject.Properties[$name]) { $obj.$name } else { $null }
}

# Returns the reason a -CopyFrom is refused, or $null.
function Get-CopyFromProblem([string]$runRoot, [string]$name, [string]$corpus, [string]$exeSha) {
    if ([string]::IsNullOrWhiteSpace($name) -or $name -ne [IO.Path]::GetFileName($name) -or $name -match '[\\/:]' -or $name -eq '..' -or $name -eq '.') {
        return 'must be an attempt folder name directly inside RunRoot'
    }
    $dir = Join-Path $runRoot $name
    if (-not (Test-Path -LiteralPath $dir -PathType Container)) { return "no attempt folder $name in RunRoot" }
    $parent = Resolve-Real (Split-Path -Parent (Resolve-Real $dir))
    if ($parent.TrimEnd('\') -ine (Resolve-Real $runRoot).TrimEnd('\')) { return 'does not sit directly in RunRoot' }
    $metaPath = Join-Path $dir 'meta.json'
    if (-not (Test-Path -LiteralPath $metaPath)) { return 'no meta.json' }
    $m = Read-Json $metaPath
    if ((Get-Prop $m 'cohort') -ne 'warm-fresh') { return 'is not a warm-fresh attempt' }
    if ((Get-Prop $m 'finished') -ne $true -or (Get-Prop $m 'exitCode') -ne 0) { return 'did not finish with exit code 0' }
    if ((Get-Prop (Get-Prop $m 'contention') 'contaminated') -ne $false) { return 'is contaminated or has no contention verdict' }
    if (-not [string]::Equals((Get-Prop $m 'corpus'), $corpus, 'OrdinalIgnoreCase')) { return 'is of another corpus' }
    if ((Get-Prop $m 'exeSha256') -ne $exeSha) { return 'was run with another exe' }
    $null
}

# Newest unused purge stamp under 15 minutes old, or $null (spec 7.4).
function Get-PurgeStamp([string]$runRoot, [datetime]$now) {
    $used = @{}
    foreach ($m in Get-ChildItem -LiteralPath $runRoot -Directory -ErrorAction SilentlyContinue) {
        $mp = Join-Path $m.FullName 'meta.json'
        if (Test-Path -LiteralPath $mp) {
            $s = Get-Prop (Read-Json $mp) 'purgeStamp'
            if ($s) { $used[$s] = $true }
        }
    }
    $best = $null
    foreach ($f in Get-ChildItem -LiteralPath $runRoot -File -Filter 'purge-*.json' -ErrorAction SilentlyContinue | Sort-Object Name) {
        if ($f.Name -notmatch '^purge-(\d{8}-\d{6})\.json$') { continue }
        $t = [datetime]::ParseExact($Matches[1], 'yyyyMMdd-HHmmss', $null)
        $age = ($now - $t).TotalMinutes
        if ($age -ge 0 -and $age -lt 15 -and -not $used.ContainsKey($f.Name)) { $best = $f.Name }
    }
    $best
}

# Reason a warm, overhead or parity attempt has no valid predecessor, or $null. last-run.json is the newest
# last-run-<attempt>.json by name, so no file is ever overwritten.
function Get-PredecessorProblem([string]$runRoot, [string]$corpus, [datetime]$now) {
    $f = @(Get-ChildItem -LiteralPath $runRoot -File -Filter 'last-run-*.json' -ErrorAction SilentlyContinue | Sort-Object Name) | Select-Object -Last 1
    if (-not $f) { return 'no last-run-*.json: run a warmup of the same corpus first' }
    $l = Read-Json $f.FullName
    if ((Get-Prop $l 'finished') -ne $true -or (Get-Prop $l 'killed') -ne $false) { return "$($f.Name) is not a finished run" }
    if (-not [string]::Equals((Get-Prop $l 'corpus'), $corpus, 'OrdinalIgnoreCase')) { return "$($f.Name) is of another corpus" }
    $ended = [DateTimeOffset]::FromUnixTimeMilliseconds([int64](Get-Prop $l 'endedUnixMs')).LocalDateTime
    $age = ($now - $ended).TotalMinutes
    if ($age -gt 5 -or $age -lt -0.1) { return ('{0} ended {1:N1} minutes before the check (limit 5)' -f $f.Name, $age) }
    $null
}

# ---------------------------------------------------------------- self-test

# The self-test's own folder: always checked against the real protected list, plus any stand-in.
function Get-SelfTestBaseProblem([string]$base, [string[]]$extra) {
    Get-RunRootProblem ([IO.Path]::GetFullPath($base)) (@(Get-ProtectedList) + @($extra))
}

function Invoke-SelfTest {
    $base = if ($RunRoot) { $RunRoot } else { Join-Path ([IO.Path]::GetTempPath()) 'sdc-profile-selftest' }
    $extra = @()
    if ($ProtectedForTest) { $extra = @($ProtectedForTest) }
    $baseWhy = Get-SelfTestBaseProblem $base $extra
    if ($baseWhy) { throw "refused: the self-test folder $base $baseWhy" }
    if (-not (Test-Path -LiteralPath $base)) { New-Item -ItemType Directory -Path $base | Out-Null }
    $t = Join-Path (Resolve-Path -LiteralPath $base).Path ('selftest-' + (Get-Date).ToString('yyyyMMdd-HHmmss'))
    New-Item -ItemType Directory -Path $t | Out-Null
    $sentinels = @{}
    function New-Sentinel([string]$rel) {
        $p = Join-Path $t $rel
        $d = Split-Path -Parent $p
        if (-not (Test-Path -LiteralPath $d)) { New-Item -ItemType Directory -Path $d | Out-Null }
        Write-NewFile $p "sentinel $rel"
        $sentinels[$p] = (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash
    }
    foreach ($s in 'prot\keep.txt', 'prot2\keep.txt', 'proj\sdc.toml', 'exist\keep.txt', 'outside\keep.txt') { New-Sentinel $s }
    New-Item -ItemType Junction -Path (Join-Path $t 'link') -Target (Join-Path $t 'prot') | Out-Null
    $prot = @((Join-Path $t 'prot')) + @(Get-ProtectedList) + $extra
    $results = New-Object System.Collections.Generic.List[object]
    function Case([string]$name, [scriptblock]$test) {
        try { $ok = [bool](& $test); $err = '' } catch { $ok = $false; $err = $_.Exception.Message }
        $results.Add([pscustomobject]@{ Name = $name; Ok = $ok; Err = $err })
        '{0} {1}{2}' -f ($(if ($ok) { 'PASS' } else { 'FAIL' })), $name, ($(if ($err) { "  ($err)" } else { '' }))
    }
    $refused = { param($root) [bool](Get-RunRootProblem $root $prot) }
    $allowed = { param($root) -not (Get-RunRootProblem $root $prot) }

    Case 'RunRoot equal to a protected path is refused' { & $refused (Join-Path $t 'prot') }
    Case 'RunRoot inside a protected path is refused' { & $refused (Join-Path $t 'prot\x\y') }
    Case 'RunRoot containing a protected path is refused' { & $refused $t }
    Case 'RunRoot below a folder holding sdc.toml is refused' { & $refused (Join-Path $t 'proj\runs') }
    Case 'RunRoot equal to a folder holding sdc.toml is refused' { & $refused (Join-Path $t 'proj') }
    Case 'sibling prefix: prot2 beside protected prot is allowed' { & $allowed (Join-Path $t 'prot2') }
    Case 'sibling prefix: prot\x is refused' { & $refused (Join-Path $t 'prot\x') }
    Case 'RunRoot reached through a junction into the protected tree is refused' { & $refused (Join-Path $t 'link\runs') }
    Case 'RunRoot that is the junction itself is refused' { & $refused (Join-Path $t 'link') }
    Case 'a plain folder outside every protected path is allowed' { & $allowed (Join-Path $t 'outside\runs') }
    Case 'a UNC RunRoot is refused as a UNC path' { (Get-RunRootProblem '\\server\share\runs' $prot) -like '*UNC*' }
    Case 'the self-test base folder guard refuses a real protected path, with and without a stand-in list' {
        (Get-SelfTestBaseProblem 'F:\ai\dds\converter\x' @()) -and (Get-SelfTestBaseProblem 'F:\ai\dds\converter\x' @('Z:\nothing')) -and
        (Get-SelfTestBaseProblem (Join-Path $t 'prot\x') @((Join-Path $t 'prot'))) -and -not (Get-SelfTestBaseProblem (Join-Path $t 'outside\runs') @((Join-Path $t 'prot')))
    }
    Case 'the purge script checks its RunRoot through the shared helper, which refuses owner data' {
        $purge = [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'profile-purge.ps1'))
        $purge.Contains('profile-paths.ps1') -and $purge.Contains('Get-RunRootProblem $RunRoot (Get-ProtectedList)') -and
        (Get-RunRootProblem 'B:\testdata\runs' (Get-ProtectedList)) -and (Get-RunRootProblem 'G:\Acherus' (Get-ProtectedList))
    }
    Case 'the real protected list holds every spec 7.8 entry (string check only)' {
        $real = Get-ProtectedList
        $need = 'B:\testdata', 'G:\', 'C:\_mount', 'F:\ai\dds\converter', 'F:\ai\dds\nifparseref', 'F:\ai\dds\PBR landscape modders resource', 'F:\ai\dds\complexgrass', 'F:\ai\dds\bench\profile\mixed-corpus', (Join-Path $env:APPDATA 'sdc'), (Join-Path $env:LOCALAPPDATA 'sdc')
        -not @($need | Where-Object { $real -notcontains $_ })
    }

    $long = 'a' * 33
    foreach ($bad in @('..', 'C:\x', 'a\b', 'a/b', '', '-x', $long)) {
        Case "label '$bad' is refused" { -not (Test-Label $bad) }
    }
    Case 'labels ok-1 and 32 characters are accepted' { (Test-Label 'ok-1') -and (Test-Label ('a' * 32)) }

    Case 'an attempt folder that already exists stops the attempt and its sentinel is unchanged' {
        $d = Join-Path $t 'exist'
        $threw = $false
        try { New-NewDir $d } catch { $threw = $true }
        $threw -and ((Get-FileHash -LiteralPath (Join-Path $d 'keep.txt') -Algorithm SHA256).Hash -eq $sentinels[(Join-Path $d 'keep.txt')])
    }
    Case 'a new folder is created' {
        New-NewDir (Join-Path $t 'fresh-dir')
        Test-Path -LiteralPath (Join-Path $t 'fresh-dir') -PathType Container
    }

    # -CopyFrom
    $rr = Join-Path $t 'copyfrom'
    New-NewDir $rr
    $corpus = 'F:\corpus\one'
    $sha = 'ABC123'
    function New-Meta([string]$dir, $meta) { New-NewDir (Join-Path $rr $dir); Write-NewJson (Join-Path $rr "$dir\meta.json") $meta }
    $good = @{ cohort = 'warm-fresh'; finished = $true; exitCode = 0; corpus = $corpus; exeSha256 = $sha; contention = @{ contaminated = $false } }
    New-Meta 'good' $good
    function With([string]$key, $value) { $c = $good.Clone(); $c[$key] = $value; $c }
    New-Meta 'noop' (With 'cohort' 'warm-noop')
    New-Meta 'dirty' (With 'contention' @{ contaminated = $true })
    New-Meta 'othercorpus' (With 'corpus' 'F:\corpus\two')
    New-Meta 'otherexe' (With 'exeSha256' 'FFF')
    New-Meta 'failed' (With 'exitCode' 1)
    New-Meta 'unfinished' (With 'finished' $false)
    New-NewDir (Join-Path $rr 'nometa')
    New-NewDir (Join-Path $rr 'sub')
    New-Sentinel 'copyfrom\sub\deep\meta.json'
    Case 'CopyFrom naming a finished warm-fresh of the same corpus and exe is accepted' { -not (Get-CopyFromProblem $rr 'good' $corpus $sha) }
    Case 'CopyFrom outside RunRoot is refused' { [bool](Get-CopyFromProblem $rr (Join-Path $t 'outside') $corpus $sha) }
    Case 'CopyFrom naming a nested path is refused' { [bool](Get-CopyFromProblem $rr 'sub\deep' $corpus $sha) }
    Case 'CopyFrom naming .. is refused' { [bool](Get-CopyFromProblem $rr '..' $corpus $sha) }
    Case 'CopyFrom naming a warm-noop attempt is refused' { [bool](Get-CopyFromProblem $rr 'noop' $corpus $sha) }
    Case 'CopyFrom naming a contaminated attempt is refused' { [bool](Get-CopyFromProblem $rr 'dirty' $corpus $sha) }
    Case 'CopyFrom naming an attempt of another corpus is refused' { [bool](Get-CopyFromProblem $rr 'othercorpus' $corpus $sha) }
    Case 'CopyFrom naming an attempt of another exe is refused' { [bool](Get-CopyFromProblem $rr 'otherexe' $corpus $sha) }
    Case 'CopyFrom naming a failed attempt is refused' { [bool](Get-CopyFromProblem $rr 'failed' $corpus $sha) }
    Case 'CopyFrom naming a folder without meta.json is refused' { [bool](Get-CopyFromProblem $rr 'nometa' $corpus $sha) }
    Case 'CopyFrom naming an unfinished attempt (exit code 0 recorded) is refused' { [bool](Get-CopyFromProblem $rr 'unfinished' $corpus $sha) }

    # free-space figure: never from the corpus (it does not even exist here)
    $eb = Join-Path $t 'expected-bytes'; New-NewDir $eb
    $missingCorpus = Join-Path $t 'no-such-corpus'
    $mkAttempt = {
        param($name, $build, $bytes, $code)
        New-NewDir (Join-Path $eb $name); New-NewDir (Join-Path $eb "$name\out")
        Write-NewFile (Join-Path $eb "$name\out\a.dds") ('x' * $bytes)
        Write-NewJson (Join-Path $eb "$name\meta.json") @{ corpus = $missingCorpus; build = $build; exitCode = $code }
    }
    Case 'the free-space figure needs -ExpectedBytes when no earlier attempt of the corpus and build exists' {
        $null -eq (Get-ExpectedBytesFigure $eb $missingCorpus 'release' 0)
    }
    Case 'the free-space figure is the given -ExpectedBytes for a first attempt, without touching the corpus' {
        (Get-ExpectedBytesFigure $eb $missingCorpus 'release' 4096) -eq 4096
    }
    & $mkAttempt '20260101-000001-a-warmup-1-aaaaaaaa' 'release' 100 0
    & $mkAttempt '20260101-000002-a-warm-fresh-1-bbbbbbbb' 'release' 300 0
    & $mkAttempt '20260101-000003-a-warm-fresh-2-cccccccc' 'base' 900 0
    & $mkAttempt '20260101-000004-a-warm-fresh-3-dddddddd' 'release' 700 1
    Case 'the free-space figure is the newest finished attempt of the same corpus and build (other build and failed ones ignored)' {
        (Get-ExpectedBytesFigure $eb $missingCorpus 'release' 4096) -eq 300
    }
    Case 'the attempt path never walks the corpus: no Get-DirBytes call takes the corpus' {
        $text = [IO.File]::ReadAllText($PSCommandPath)
        -not [regex]::IsMatch($text, 'Get-DirBytes\s+\(?\$A?\.?[Cc]orpus')
    }
    Case 'the next attempt number counts only the same label and cohort' {
        $d = Join-Path $t 'k'; New-NewDir $d
        foreach ($n in '20260101-000001-a-warm-fresh-1-aaaaaaaa', '20260101-000002-b-a-warm-fresh-1-bbbbbbbb', '20260101-000003-a-warm-fresh-2-cccccccc') { New-NewDir (Join-Path $d $n) }
        (Get-NextK $d 'a' 'warm-fresh') -eq 3
    }

    # plan stream parser (EP-T4-2b): canned events, StrictMode-safe on missing fields
    $canned = @(
        'not json at all',
        '{"kind":"file_planned","job_key":"textures/a.dds","job":{"status":{"state":"ready"}}}',
        '{"kind":"file_planned","job_key":"textures/b.dds","job":{"status":{"state":"blocked","codes":["X-1"]}}}',
        '{"kind":"file_skipped","path":"C:/c/textures/c_ao.png","job_key":"textures/c.dds","skip":"packed_source","code":null,"reason":"packed into x"}',
        '{"kind":"file_skipped","path":"C:/c/textures/d.png","skip":"other"}',
        '{"kind":"file_failed","path":"C:/c/textures/e.png"}',
        '{"kind":"file_planned","job_key":"textures/f.dds"}'
    )
    $parsed = ConvertFrom-PlanStream $canned 'C:\c'
    Case 'plan stream: only ready planned jobs are expected keys (a job without a status is not)' {
        (@($parsed.Ready) -join ',') -eq 'textures/a.dds'
    }
    Case 'plan stream: planned-not-written holds planned jobs only, with their status' {
        $nw = @($parsed.NotWritten)
        $nw.Count -eq 2 -and $nw[0].StartsWith("textures/b.dds`tblocked`t") -and $nw[1].StartsWith("textures/f.dds`t")
    }
    Case 'plan stream: skip events go to skipped-inputs with their input path, never into planned-not-written' {
        $sk = @($parsed.Skipped)
        $sk.Count -eq 2 -and $sk[0].StartsWith("textures/c_ao.png`tpacked_source") -and $sk[1].StartsWith("textures/d.png`tother") -and -not (@($parsed.NotWritten) -match 'c_ao')
    }
    Case 'plan stream: skip and failure events are listed with reasons, missing fields tolerated' {
        $ev = @($parsed.Events)
        $ev.Count -eq 3 -and $ev[2].StartsWith("failed`ttextures/e.png")
    }

    # contention: gaps in the counter stream are flagged (Minor 1)
    $mkRow = { param($ctr) [pscustomobject]@{ t_ms = 0; ctr_ms = $ctr; total = 0; exempt = 0; other_cpu = 0; other_io = 0; sys_cpu = 0; sys_io = 0; banned = '' } }
    $base0 = @{ otherCpuCores = 0.0; otherIoBytesPerSec = 0.0 }
    Case 'contention: a gap over 1.5 s between counter samples makes the verdict contaminated' {
        $v = Get-ContentionVerdict @((& $mkRow 1000), (& $mkRow 2000), (& $mkRow 4000)) $base0 $false
        $v.contaminated -and $v.sampleGaps -eq 1
    }
    Case 'contention: one-second samples are clean' {
        $v = Get-ContentionVerdict @((& $mkRow 1000), (& $mkRow 2000), (& $mkRow 3010)) $base0 $false
        (-not $v.contaminated) -and $v.sampleGaps -eq 0
    }
    Case 'hashes and list files: no rows give no blank line' {
        (Join-Lines @()) -eq '' -and (Join-Lines @('a', 'b')) -eq "a`nb`n"
    }

    # cold stamps
    $now = Get-Date
    $stampName = { param($min) 'purge-' + $now.AddMinutes(-$min).ToString('yyyyMMdd-HHmmss') + '.json' }
    $c0 = Join-Path $t 'cold-none'; New-NewDir $c0
    $c1 = Join-Path $t 'cold-stale'; New-NewDir $c1; Write-NewJson (Join-Path $c1 (& $stampName 20)) @{ x = 1 }
    $c2 = Join-Path $t 'cold-used'; New-NewDir $c2
    $used = & $stampName 1
    Write-NewJson (Join-Path $c2 $used) @{ x = 1 }
    New-NewDir (Join-Path $c2 'earlier-attempt'); Write-NewJson (Join-Path $c2 'earlier-attempt\meta.json') @{ purgeStamp = $used }
    $c3 = Join-Path $t 'cold-ok'; New-NewDir $c3
    $okStamp = & $stampName 2
    Write-NewJson (Join-Path $c3 $okStamp) @{ x = 1 }
    Case 'a cold attempt without a stamp is refused' { -not (Get-PurgeStamp $c0 $now) }
    Case 'a cold attempt with a stale stamp is refused' { -not (Get-PurgeStamp $c1 $now) }
    Case 'a cold attempt with a stamp an earlier attempt names is refused' { -not (Get-PurgeStamp $c2 $now) }
    Case 'a cold attempt with a fresh unused stamp is accepted' { (Get-PurgeStamp $c3 $now) -eq $okStamp }

    # warm predecessors
    function New-LastRun([string]$dir, [double]$minAgo, [string]$corp, [bool]$killed) {
        $d = Join-Path $t $dir; New-NewDir $d
        $ms = [DateTimeOffset]::new($now.AddMinutes(-$minAgo)).ToUnixTimeMilliseconds()
        Write-NewJson (Join-Path $d 'last-run-20260101-000000-x-warmup-1-aaaaaaaa.json') @{ finished = $true; killed = $killed; corpus = $corp; endedUnixMs = $ms }
        $d
    }
    $w0 = Join-Path $t 'warm-none'; New-NewDir $w0
    $w1 = New-LastRun 'warm-recent' 2 $corpus $false
    $w2 = New-LastRun 'warm-old' 6 $corpus $false
    $w3 = New-LastRun 'warm-killed' 1 $corpus $true
    $w4 = New-LastRun 'warm-other' 1 'F:\corpus\two' $false
    Case 'a warm attempt without a recent last-run file is refused' { [bool](Get-PredecessorProblem $w0 $corpus $now) }
    Case 'a warm attempt after a run that ended 2 minutes ago is accepted' { -not (Get-PredecessorProblem $w1 $corpus $now) }
    Case 'a warm attempt after a run that ended 6 minutes ago is refused' { [bool](Get-PredecessorProblem $w2 $corpus $now) }
    Case 'a warm attempt after a killed run is refused' { [bool](Get-PredecessorProblem $w3 $corpus $now) }
    Case 'a warm attempt after a run of another corpus is refused' { [bool](Get-PredecessorProblem $w4 $corpus $now) }

    Case 'attempt names have the format and two in one second differ' {
        $a = Get-AttemptName 'fx' 'warm-fresh' 1
        $b = Get-AttemptName 'fx' 'warm-fresh' 1
        ($a -match '^\d{8}-\d{6}-fx-warm-fresh-1-[0-9a-f]{8}$') -and ($b -match '^\d{8}-\d{6}-fx-warm-fresh-1-[0-9a-f]{8}$') -and ($a -ne $b)
    }
    Case 'the script text has no delete, move, clear, mirror, purge or forced folder creation' {
        $text = [IO.File]::ReadAllText($PSCommandPath) + "`n" + [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'profile-paths.ps1'))
        $words = @(('Remove' + '-Item'), ('Move' + '-Item'), ('Clear' + '-Content'), ('/M' + 'IR'), ('/PUR' + 'GE'), ('/M' + 'OV'))
        $hit = @($words | Where-Object { $text.IndexOf($_, [StringComparison]::OrdinalIgnoreCase) -ge 0 })
        $newForce = @($text -split "`n" | Where-Object { $_.Contains('New' + '-Item') -and $_.Contains('-Fo' + 'rce') })
        if ($hit.Count -or $newForce.Count) { throw ('found: ' + ($hit -join ', ') + " $($newForce.Count) New-Item line(s) with force") }
        $true
    }
    Case 'every sentinel is unchanged and present' {
        $bad = @($sentinels.Keys | Where-Object { -not (Test-Path -LiteralPath $_) -or (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash -ne $sentinels[$_] })
        if ($bad.Count) { throw ('changed or missing: ' + ($bad -join ', ')) }
        $true
    }
    $failed = @($results | Where-Object { -not $_.Ok })
    "selftest folder: $t"
    "{0} cases, {1} failed" -f $results.Count, $failed.Count
    if ($failed.Count) { exit 1 }
    exit 0
}

# ---------------------------------------------------------------- machine state, quiet check, contention

$BannedNames = @('cargo', 'rustc', 'link', 'cl', 'clippy-driver', 'tessalith', 'sdc', 'samply', 'wpr')
$SystemNames = @('system', 'msmpeng', 'registry', 'memcompression')
$TargetPathRx = '(?i)\\(cargo-)?target[^\\]*\\(.+\\)?(debug|release|profiling)\\'
$DetectionLimit = 'Detection limit: a process that starts and exits between two one-second samples is not seen.'

function Get-ExeSha([string]$exe) { (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash }

function Sort-Ordinal($items) {
    $a = [string[]]@($items | Where-Object { $null -ne $_ })
    [Array]::Sort($a, [StringComparer]::Ordinal)
    , $a
}

# One line per item, each ended by a newline; nothing at all for no items (no blank line).
function Join-Lines($items) {
    $a = @($items | Where-Object { $null -ne $_ })
    if ($a.Count -eq 0) { return '' }
    ($a -join "`n") + "`n"
}

function Say([string]$m) { Write-Host ('[{0}] {1}' -f (Get-Date).ToString('HH:mm:ss'), $m) }

function Get-RunnerTree {
    $all = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId)
    $set = New-Object 'System.Collections.Generic.HashSet[int]'
    [void]$set.Add($PID)
    do {
        $grew = $false
        foreach ($p in $all) {
            if ($set.Contains([int]$p.ParentProcessId) -and $set.Add([int]$p.ProcessId)) { $grew = $true }
        }
    } while ($grew)
    , $set
}

function Get-BannedRunning($exempt) {
    $found = @()
    foreach ($p in Get-Process) {
        if ($exempt.Contains([int]$p.Id)) { continue }
        $path = $null
        try { $path = $p.Path } catch { }
        if ($BannedNames -contains $p.ProcessName.ToLowerInvariant() -or ($path -and $path -match $TargetPathRx)) {
            $found += ('{0}({1})' -f $p.ProcessName, $p.Id)
        }
    }
    $found
}

function Get-DirBytes([string]$dir) {
    $n = [int64]0
    foreach ($f in [IO.Directory]::EnumerateFiles($dir, '*', [IO.SearchOption]::AllDirectories)) { $n += ([IO.FileInfo]$f).Length }
    $n
}

# Free-space basis: the newest finished attempt's out size for the same corpus and build, read from our own
# folders; otherwise the caller's -ExpectedBytes; otherwise $null. Nothing reads the corpus outside the command.
# shortcut: the first attempt of a corpus and build needs -ExpectedBytes by hand, upgrade when -Expected records planned output sizes.
function Get-ExpectedBytesFigure([string]$runRoot, [string]$corpus, [string]$build, [int64]$given) {
    foreach ($d in Get-ChildItem -LiteralPath $runRoot -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending) {
        $mp = Join-Path $d.FullName 'meta.json'
        $out = Join-Path $d.FullName 'out'
        if (-not (Test-Path -LiteralPath $mp) -or -not (Test-Path -LiteralPath $out)) { continue }
        $m = Read-Json $mp
        if ([string]::Equals((Get-Prop $m 'corpus'), $corpus, 'OrdinalIgnoreCase') -and (Get-Prop $m 'build') -eq $build -and (Get-Prop $m 'exitCode') -eq 0) {
            return (Get-DirBytes $out)
        }
    }
    if ($given -gt 0) { return $given }
    $null
}

# The next repeat number for a label and cohort: exact name match, so label "a" does not count "b-a".
function Get-NextK([string]$runRoot, [string]$label, [string]$cohort) {
    $rx = '^\d{8}-\d{6}-' + [regex]::Escape($label) + '-' + [regex]::Escape($cohort) + '-\d+-[0-9a-f]{8}$'
    @(Get-ChildItem -LiteralPath $runRoot -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -match $rx }).Count + 1
}

# Parses the dry run's JSONL (spec 7.3). Missing fields are tolerated (StrictMode).
function ConvertFrom-PlanStream([string[]]$lines, [string]$corpusDir) {
    $corpusRoot = ([IO.Path]::GetFullPath($corpusDir).TrimEnd('\') + '\').Replace('\', '/')
    $rel = { param($p) $s = [string]$p; $n = $s.Replace('\', '/'); if ($n.StartsWith($corpusRoot, 'OrdinalIgnoreCase')) { $n.Substring($corpusRoot.Length) } else { $s } }
    $ready = New-Object System.Collections.Generic.List[string]
    $notWritten = New-Object System.Collections.Generic.List[string]
    $skipped = New-Object System.Collections.Generic.List[string]
    $events = New-Object System.Collections.Generic.List[string]
    foreach ($line in $lines) {
        if (-not $line.Trim().StartsWith('{')) { continue }
        $e = $line | ConvertFrom-Json
        $path = & $rel (Get-Prop $e 'path')
        switch (Get-Prop $e 'kind') {
            'file_planned' {
                $status = Get-Prop (Get-Prop $e 'job') 'status'
                $state = Get-Prop $status 'state'
                $key = [string](Get-Prop $e 'job_key')
                if ($state -eq 'ready') { $ready.Add($key) }
                else { $notWritten.Add(("{0}`t{1}`t{2}" -f $key, $(if ($state) { $state } else { '(none)' }), $(if ($status) { $status | ConvertTo-Json -Compress -Depth 5 } else { '' }))) }
            }
            'file_skipped' {
                $kind = Get-Prop $e 'skip'; $code = Get-Prop $e 'code'; $reason = Get-Prop $e 'reason'
                $events.Add(("skipped`t{0}`t{1}`t{2}`t{3}" -f $path, $kind, $code, $reason))
                $skipped.Add(("{0}`t{1}`t{2}`t{3}`t{4}" -f $path, $kind, $code, (Get-Prop $e 'job_key'), $reason))
            }
            'file_failed' { $events.Add(("failed`t{0}`t{1}" -f $path, (Get-Prop $e 'error'))) }
        }
    }
    [pscustomobject]@{ Ready = $ready.ToArray(); NotWritten = $notWritten.ToArray(); Skipped = $skipped.ToArray(); Events = $events.ToArray() }
}
# One quiet check (spec 7.6): banned processes, 10 s of CPU under 5 %, free space.
function Test-Quiet([string]$runRoot, [int64]$neededBytes) {
    $reasons = @()
    $banned = @(Get-BannedRunning (Get-RunnerTree))
    if ($banned.Count) { $reasons += ('banned processes running: ' + ($banned -join ', ')) }
    $cpu = (Get-Counter '\Processor(_Total)\% Processor Time' -SampleInterval 1 -MaxSamples 10).CounterSamples | Measure-Object CookedValue -Average
    $cpuAvg = [math]::Round($cpu.Average, 2)
    if ($cpuAvg -ge 5) { $reasons += "CPU averaged $cpuAvg % over 10 s (limit 5 %)" }
    $root = [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($runRoot))
    $free = (New-Object IO.DriveInfo($root)).AvailableFreeSpace
    $freeOk = $free -ge $neededBytes
    $freeReason = $null
    if (-not $freeOk) { $freeReason = ('free space {0:N1} GB on {1} is under the needed {2:N1} GB' -f ($free / 1GB), $root, ($neededBytes / 1GB)); $reasons += $freeReason }
    [pscustomobject]@{ Ok = ($reasons.Count -eq 0); Reasons = $reasons; CpuAvgPct = $cpuAvg; FreeBytes = $free; NeededBytes = $neededBytes; FreeOk = $freeOk; Banned = $banned }
}

# Retries every 5 minutes up to 2 hours; a free-space failure stops at once.
function Wait-Quiet([string]$runRoot, [int64]$neededBytes) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $tries = 0
    while ($true) {
        $tries++
        $r = Test-Quiet $runRoot $neededBytes
        if ($r.Ok) { return [pscustomobject]@{ Result = $r; Tries = $tries; PassedFirstTry = ($tries -eq 1); WaitedSec = [int]$sw.Elapsed.TotalSeconds } }
        Say ('quiet check failed (try {0}): {1}' -f $tries, ($r.Reasons -join '; '))
        if (-not $r.FreeOk) { throw "stopping: $($r.Reasons -join '; ')" }
        if ($sw.Elapsed.TotalHours -ge 2) { throw 'stopping: the machine did not become quiet within 2 hours' }
        Start-Sleep -Seconds 300
    }
}

function Get-DiskRaw {
    @(Get-CimInstance Win32_PerfRawData_PerfDisk_PhysicalDisk | Where-Object { $_.Name -ne '_Total' } |
        ForEach-Object { [pscustomobject]@{ name = $_.Name; read = [uint64]$_.DiskReadBytesPersec; write = [uint64]$_.DiskWriteBytesPersec } })
}

function Get-DiskFigure($raw, [string]$path) {
    $q = [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($path)).TrimEnd('\')
    $hit = @($raw | Where-Object { $_.name -like "*$q*" }) | Select-Object -First 1
    if ($hit) { $hit } else { [pscustomobject]@{ name = "n/a ($q has no physical-disk counter)"; read = 0; write = 0 } }
}

function Get-FileList([string]$dir) {
    $root = [IO.Path]::GetFullPath($dir).TrimEnd('\') + '\'
    foreach ($f in [IO.Directory]::EnumerateFiles($dir, '*', [IO.SearchOption]::AllDirectories) | Sort-Object) {
        $fi = [IO.FileInfo]$f
        '{0},{1},{2}' -f $f.Substring($root.Length).Replace('\', '/').ToLowerInvariant(), $fi.Length, $fi.LastWriteTimeUtc.Ticks
    }
}

# The per-second contention stream (spec 7.6): one Get-Counter -Continuous, started before the command.
function Start-ContentionStream($shared) {
    Start-ThreadJob -ArgumentList $shared -ScriptBlock {
        param($sh)
        $banned = $sh.Banned; $system = $sh.System; $rx = $sh.TargetRx
        $counters = '\Processor(_Total)\% Processor Time', '\Process(*)\ID Process', '\Process(*)\% Processor Time', '\Process(*)\IO Read Bytes/sec', '\Process(*)\IO Write Bytes/sec'
        Get-Counter -Counter $counters -SampleInterval 1 -Continuous | ForEach-Object {
            $total = 0.0; $by = @{}
            foreach ($c in $_.CounterSamples) {
                if ($c.Path -match '\\processor\(_total\)\\') { $total = $c.CookedValue; continue }
                if ($c.Path -match '\\process\((.+)\)\\([^\\]+)$') {
                    if (-not $by.ContainsKey($Matches[1])) { $by[$Matches[1]] = @{} }
                    $by[$Matches[1]][$Matches[2]] = $c.CookedValue
                }
            }
            $tree = New-Object 'System.Collections.Generic.HashSet[int]'
            [void]$tree.Add($sh.RunnerPid)
            $all = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId)
            do { $grew = $false; foreach ($p in $all) { if ($tree.Contains([int]$p.ParentProcessId) -and $tree.Add([int]$p.ProcessId)) { $grew = $true } } } while ($grew)
            $exemptCpu = 0.0; $sysCpu = 0.0; $otherIo = 0.0; $sysIo = 0.0; $found = @()
            foreach ($k in $by.Keys) {
                $name = ($k -replace '#\d+$', '')
                if ($name -eq '_total' -or $name -eq 'idle') { continue }
                $m = $by[$k]
                $procId = [int]$m['id process']
                $cpu = [double]$m['% processor time'] / 100.0
                $io = [double]$m['io read bytes/sec'] + [double]$m['io write bytes/sec']
                if ($tree.Contains($procId) -or $sh.Exempt.ContainsKey($procId)) { $exemptCpu += $cpu; continue }
                if ($system -contains $name) { $sysCpu += $cpu; $sysIo += $io; continue }
                $otherIo += $io
                if ($banned -contains $name) { $found += ('{0}({1})' -f $name, $procId) }
            }
            foreach ($p in Get-Process) {
                if ($tree.Contains([int]$p.Id) -or $sh.Exempt.ContainsKey([int]$p.Id)) { continue }
                $path = $null; try { $path = $p.Path } catch { }
                $tag = '{0}({1})' -f $p.ProcessName, $p.Id
                if ($path -and $path -match $rx -and $found -notcontains $tag) { $found += $tag }
            }
            $totalCores = $total / 100.0 * $sh.Logical
            $sh.Queue.Enqueue([pscustomobject]@{
                    t_ms = $sh.Sw.ElapsedMilliseconds; ctr_ms = ([DateTimeOffset]$_.Timestamp).ToUnixTimeMilliseconds(); total = [math]::Round($totalCores, 3); exempt = [math]::Round($exemptCpu, 3)
                    other_cpu = [math]::Round([math]::Max(0.0, $totalCores - $exemptCpu - $sysCpu), 3); other_io = [math]::Round($otherIo, 0)
                    sys_cpu = [math]::Round($sysCpu, 3); sys_io = [math]::Round($sysIo, 0); banned = ($found -join ';')
                })
        }
    }
}

# Contamination per spec 7.6 against the baseline (or the calibration figures themselves).
function Get-ContentionVerdict($rows, $baseline, [bool]$calibrating) {
    $n = @($rows).Count
    $cpuMean = if ($n) { ($rows | Measure-Object other_cpu -Average).Average } else { 0.0 }
    $ioMean = if ($n) { ($rows | Measure-Object other_io -Average).Average } else { 0.0 }
    $ioMax = if ($n) { ($rows | Measure-Object other_io -Maximum).Maximum } else { 0.0 }
    $sysCpuMean = if ($n) { ($rows | Measure-Object sys_cpu -Average).Average } else { 0.0 }
    $sysIoMean = if ($n) { ($rows | Measure-Object sys_io -Average).Average } else { 0.0 }
    $bannedSeen = @($rows | Where-Object { $_.banned } | ForEach-Object { $_.banned } | ForEach-Object { $_ -split ';' } | Sort-Object -Unique)
    $reasons = @()
    # EP-R4: every second. Counter timestamps further apart than 1.5 s mean seconds were not sampled.
    $stamps = @($rows | ForEach-Object { $_.ctr_ms } | Sort-Object)
    $gaps = 0
    for ($i = 1; $i -lt $stamps.Count; $i++) { if ($stamps[$i] - $stamps[$i - 1] -gt 1500) { $gaps++ } }
    if ($gaps) { $reasons += "$gaps gap(s) over 1.5 s between contention samples" }
    if ($n -eq 0) { $reasons += 'no contention samples were recorded' }
    if ($bannedSeen.Count) { $reasons += ('banned process appeared: ' + ($bannedSeen -join ', ')) }
    if (-not $calibrating -and $baseline) {
        if ($cpuMean -gt $baseline.otherCpuCores + 0.5) { $reasons += ('other CPU averaged {0:N2} cores, baseline {1:N2} + 0.5' -f $cpuMean, $baseline.otherCpuCores) }
        if ($ioMean -gt $baseline.otherIoBytesPerSec + 20MB) { $reasons += ('other I/O averaged {0:N1} MB/s, baseline {1:N1} + 20' -f ($ioMean / 1MB), ($baseline.otherIoBytesPerSec / 1MB)) }
        $spike = @($rows | Where-Object { $_.other_io -gt $baseline.otherIoBytesPerSec + 200MB }).Count
        if ($spike) { $reasons += "other I/O exceeded the baseline by 200 MB/s in $spike second(s)" }
    }
    [ordered]@{
        contaminated = ($reasons.Count -gt 0); reasons = $reasons; samples = $n; sampleGaps = $gaps; calibrating = $calibrating
        otherCpuCoresMean = [math]::Round($cpuMean, 3); otherIoBytesPerSecMean = [math]::Round($ioMean, 0); otherIoBytesPerSecMax = [math]::Round($ioMax, 0)
        systemLineCpuCoresMean = [math]::Round($sysCpuMean, 3); systemLineIoBytesPerSecMean = [math]::Round($sysIoMean, 0)
        baseline = $baseline; detectionLimit = $DetectionLimit
    }
}

# Starts a child with redirected output copied into new files. Env: the frozen settings of 7.3.
function Start-Child([string]$exe, [string[]]$arguments, [string]$workDir, [hashtable]$env, [string]$outPath, [string]$errPath) {
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    foreach ($a in $arguments) { $psi.ArgumentList.Add($a) }
    $psi.WorkingDirectory = $workDir
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    foreach ($k in $env.Keys) {
        if ($null -eq $env[$k]) { [void]$psi.Environment.Remove($k) } else { $psi.Environment[$k] = [string]$env[$k] }
    }
    $outFs = [IO.File]::Open($outPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    $errFs = [IO.File]::Open($errPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    $p = [Diagnostics.Process]::Start($psi)
    [pscustomobject]@{
        Process = $p; OutFs = $outFs; ErrFs = $errFs
        OutTask = $p.StandardOutput.BaseStream.CopyToAsync($outFs)
        ErrTask = $p.StandardError.BaseStream.CopyToAsync($errFs)
    }
}

function Stop-Child($c) {
    $c.Process.WaitForExit()
    $c.OutTask.Wait(); $c.ErrTask.Wait()
    $c.OutFs.Dispose(); $c.ErrFs.Dispose()
}

function Get-ConvertArgs([string]$corpus, [string]$out, [string[]]$sets, [string[]]$extra) {
    $a = @('convert', $corpus, '-o', $out, '--recursive', '--accept-suggestions', 'high', '--jobs', '0')
    foreach ($s in $sets) { $a += @('--set', $s) }
    $a + $extra
}

# ---------------------------------------------------------------- Expected (spec 7.3)

function Invoke-Expected([hashtable]$A) {
    $dir = Join-Path $A.RunRoot ('expected-{0}-run-{1}-{2}' -f $A.Label, (Get-Date).ToString('yyyyMMdd-HHmmss'), [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-NewDir $dir
    New-NewDir (Join-Path $dir 'out')
    New-NewDir (Join-Path $dir 'appdata')
    $cargs = @('convert', $A.Corpus, '-o', (Join-Path $dir 'out'), '--recursive', '--accept-suggestions', 'high', '--dry-run', '--json')
    foreach ($s in $A.Set) { $cargs += @('--set', $s) }
    $c = Start-Child $A.Exe $cargs $dir @{ APPDATA = (Join-Path $dir 'appdata'); SDC_PROFILE = $null } (Join-Path $dir 'stdout.jsonl') (Join-Path $dir 'stderr.log')
    Stop-Child $c
    if ($c.Process.ExitCode -ne 0) { throw "the dry run exited with $($c.Process.ExitCode); see $dir\stderr.log" }
    $plan = ConvertFrom-PlanStream @([IO.File]::ReadAllLines((Join-Path $dir 'stdout.jsonl'))) $A.Corpus
    $sorted = Sort-Ordinal $plan.Ready
    Write-NewFile (Join-Path $A.RunRoot "expected-$($A.Label).txt") (Join-Lines $sorted)
    Write-NewFile (Join-Path $A.RunRoot "planned-not-written-$($A.Label).txt") (Join-Lines (Sort-Ordinal $plan.NotWritten))
    Write-NewFile (Join-Path $A.RunRoot "skipped-inputs-$($A.Label).txt") (Join-Lines (Sort-Ordinal $plan.Skipped))
    Write-NewFile (Join-Path $A.RunRoot "expected-$($A.Label)-events.txt") (Join-Lines (Sort-Ordinal $plan.Events))
    Say ('expected {0} key(s); {1} planned but not written; {2} skipped input(s); {3} skip/failure event(s); dry-run folder {4}' -f $sorted.Count, @($plan.NotWritten).Count, @($plan.Skipped).Count, @($plan.Events).Count, $dir)
}

# ---------------------------------------------------------------- one attempt (spec 7.3 to 7.7)

$WarmCohorts = @('warm-fresh', 'warm-noop', 'warm-stale', 'overhead', 'parity')

function Invoke-Attempt([hashtable]$A, [bool]$dry) {
    $exeSha = Get-ExeSha $A.Exe
    $cohort = $A.Cohort
    $problems = @()
    $baselinePath = Join-Path $A.RunRoot 'baseline.json'
    if ($A.Calibrate) { if (Test-Path -LiteralPath $baselinePath) { $problems += 'baseline.json already exists in this RunRoot (it is never overwritten)' } }
    elseif (-not (Test-Path -LiteralPath $baselinePath)) { $problems += 'no baseline.json: run a -Calibrate attempt on a fixture project first' }
    $copies = $cohort -in 'warm-noop', 'warm-stale'
    if ($copies) {
        if (-not $A.CopyFrom) { $problems += "-CopyFrom is required for $cohort" }
        else { $cp = Get-CopyFromProblem $A.RunRoot $A.CopyFrom $A.Corpus $exeSha; if ($cp) { $problems += "-CopyFrom $($A.CopyFrom): $cp" } }
    } elseif ($A.CopyFrom) { $problems += "-CopyFrom is refused for $cohort" }
    if ($A.Samply -and -not (Test-Path -LiteralPath $A.Samply -PathType Leaf)) { $problems += "samply not found: $($A.Samply)" }
    # refuse at once what no amount of waiting for a quiet machine can fix (a missing stamp or predecessor)
    if ($cohort -eq 'cold-fresh' -and -not (Get-PurgeStamp $A.RunRoot (Get-Date))) { $problems += 'no unused purge-*.json under 15 minutes old in RunRoot (the owner runs tools\profile-purge.ps1 elevated)' }
    if ($cohort -in $WarmCohorts) { $pp = Get-PredecessorProblem $A.RunRoot $A.Corpus (Get-Date); if ($pp) { $problems += $pp } }
    $figure = Get-ExpectedBytesFigure $A.RunRoot $A.Corpus $A.Build ([int64]$A.ExpectedBytes)
    if ($null -eq $figure) { $problems += 'no earlier attempt of this corpus and build in RunRoot to size the free-space check from: pass -ExpectedBytes (the corpus is never read outside the command)' }

    $extra = @()
    if ($cohort -eq 'warm-stale') { $extra = @('--set', "mips.edge=$($A.StaleEdge)") }
    $name = Get-AttemptName $A.Label $cohort (Get-NextK $A.RunRoot $A.Label $cohort)
    $attempt = Join-Path $A.RunRoot $name
    $out = Join-Path $attempt 'out'
    $cmdArgs = Get-ConvertArgs $A.Corpus $out $A.Set $extra
    $useProfile = $A.Build -in 'release', 'profiling'
    $profileDir = Join-Path $attempt 'profile'
    $exeToRun = $A.Exe
    $fullArgs = $cmdArgs
    if ($A.Samply) { $exeToRun = $A.Samply; $fullArgs = @('record', '--save-only', '--unstable-presymbolicate', '-o', (Join-Path $attempt 'samply.json.gz'), '--', $A.Exe) + $cmdArgs }
    $needed = [int64](2.5 * [int64]$figure + 20GB)

    if ($dry) {
        Say "DRY RUN (nothing is created or run)"
        Say "attempt folder: $attempt"
        Say "out folder:     $out"
        Say "APPDATA:        $(Join-Path $attempt 'appdata')   SDC_PROFILE: $(if ($useProfile) { $profileDir } else { '(unset)' })"
        Say "command:        $exeToRun $($fullArgs -join ' ')"
        if ($copies) { Say "copy:           robocopy <$($A.RunRoot)\$($A.CopyFrom)\out> $out /E /COPY:DAT /DCOPY:T" }
        $q = Test-Quiet $A.RunRoot $needed
        Say ('quiet check: {0}; CPU {1} %; free {2:N1} GB, needed {3:N1} GB{4}' -f $(if ($q.Ok) { 'PASS' } else { 'FAIL' }), $q.CpuAvgPct, ($q.FreeBytes / 1GB), ($q.NeededBytes / 1GB), $(if ($q.Reasons) { '; ' + ($q.Reasons -join '; ') } else { '' }))
        foreach ($p in $problems) { Say "would refuse: $p" }
        Say $DetectionLimit
        return 0
    }
    if ($problems) { throw ('refused: ' + ($problems -join '; ')) }

    # quiet check; a warm chain whose check needed a retry gets a warmup first (spec 7.3)
    $qs = Get-Date
    $quiet = Wait-Quiet $A.RunRoot $needed
    if (-not $quiet.PassedFirstTry -and ($cohort -in $WarmCohorts)) {
        Say 'the quiet check needed a retry: running a warmup first'
        $w = $A.Clone(); $w.Cohort = 'warmup'; $w.CopyFrom = $null; $w.Calibrate = $false; $w.Samply = $null
        $wr = Invoke-Attempt $w $false
        if ($wr -ne 0) { throw "the warmup ended with code $wr; stopping" }
        $qs = Get-Date
        $quiet = Wait-Quiet $A.RunRoot $needed
        if (-not $quiet.PassedFirstTry) { throw 'stopping: the quiet check after the warmup did not pass at its first try' }
    }
    if ($cohort -in $WarmCohorts -and -not $copies) {
        $pp = Get-PredecessorProblem $A.RunRoot $A.Corpus $qs
        if ($pp) { throw "refused: $pp" }
    }
    $purge = $null
    if ($cohort -eq 'cold-fresh') {
        $purge = Get-PurgeStamp $A.RunRoot (Get-Date)
        if (-not $purge) { throw 'refused: no unused purge-*.json under 15 minutes old in RunRoot (the owner runs tools\profile-purge.ps1 elevated)' }
    }
    $baseline = $null
    if (-not $A.Calibrate) { $baseline = Read-Json $baselinePath }

    # folders, each created new
    New-NewDir $attempt
    New-NewDir $out
    New-NewDir (Join-Path $attempt 'appdata')
    $meta = [ordered]@{
        attempt = $name; label = $A.Label; cohort = $cohort; build = $A.Build; corpus = $A.Corpus; exe = $A.Exe; exeSha256 = $exeSha
        commit = (& git -C $PSScriptRoot rev-parse HEAD 2>$null); command = ($exeToRun + ' ' + ($fullArgs -join ' ')); finished = $false
        exitCode = $null; externalWallMs = $null; purgeStamp = $purge; calibrate = [bool]$A.Calibrate; samply = $null
        quiet = [ordered]@{ passedFirstTry = $quiet.PassedFirstTry; tries = $quiet.Tries; waitedSec = $quiet.WaitedSec; cpuAvgPct = $quiet.Result.CpuAvgPct; freeGB = [math]::Round($quiet.Result.FreeBytes / 1GB, 1); neededGB = [math]::Round($needed / 1GB, 1) }
        logicalCores = [Environment]::ProcessorCount
    }
    $code = 1
    try {
        if ($copies) {
            $src = Join-Path (Join-Path $A.RunRoot $A.CopyFrom) 'out'
            $pp = Get-PredecessorProblem $A.RunRoot $A.Corpus (Get-Date)
            if ($pp) { throw "refused: $pp" }
            $bytes = Get-DirBytes $src
            $csw = [Diagnostics.Stopwatch]::StartNew()
            & robocopy $src $out /E /COPY:DAT /DCOPY:T /R:1 /W:1 /NP /NFL /NDL /NJH | Out-Null
            $rc = $LASTEXITCODE
            $csw.Stop()
            $meta['copy'] = [ordered]@{ from = $A.CopyFrom; bytes = $bytes; ms = $csw.ElapsedMilliseconds; robocopyExit = $rc }
            if ($rc -ge 8) { throw "robocopy failed with exit code $rc" }
        }
        if ($cohort -eq 'warm-noop') { Write-NewFile (Join-Path $attempt 'files-before.csv') ("path,size,mtime_utc_ticks`n" + (Join-Lines (Get-FileList $out))) }

        $meta['powerPlan'] = ((& powercfg /getactivescheme) -join ' ')
        $meta['defender'] = try { (Get-MpComputerStatus).RealTimeProtectionEnabled } catch { "unavailable: $($_.Exception.Message)" }
        $meta['drives'] = @(Get-PhysicalDisk | ForEach-Object { [ordered]@{ name = $_.FriendlyName; media = [string]$_.MediaType } })
        $meta['cpu'] = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
        $meta['ramGB'] = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 1)

        $shared = [hashtable]::Synchronized(@{
                Queue = (New-Object 'System.Collections.Concurrent.ConcurrentQueue[object]'); Exempt = (New-Object 'System.Collections.Concurrent.ConcurrentDictionary[int,bool]')
                Sw = [Diagnostics.Stopwatch]::StartNew(); RunnerPid = $PID; Logical = [Environment]::ProcessorCount
                Banned = $BannedNames; System = $SystemNames; TargetRx = $TargetPathRx
            })
        $job = Start-ContentionStream $shared
        $contWriter = New-Object IO.StreamWriter([IO.File]::Open((Join-Path $attempt 'contention.csv'), [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read))
        $sampWriter = New-Object IO.StreamWriter([IO.File]::Open((Join-Path $attempt 'sampler.csv'), [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read))
        $contWriter.WriteLine('t_ms,counter_unix_ms,total_cpu_cores,exempt_cpu_cores,other_cpu_cores,other_io_bytes_per_s,system_cpu_cores,system_io_bytes_per_s,banned')
        $sampWriter.WriteLine('t_ms,pid,working_set,private_bytes,threads,cpu_ms')
        $rows = New-Object System.Collections.Generic.List[object]
        $drain = {
            $r = $null
            while ($shared.Queue.TryDequeue([ref]$r)) {
                $contWriter.WriteLine(('{0},{1},{2},{3},{4},{5},{6},{7},{8}' -f $r.t_ms, $r.ctr_ms, $r.total, $r.exempt, $r.other_cpu, $r.other_io, $r.sys_cpu, $r.sys_io, $r.banned))
                $rows.Add($r)
            }
        }
        try {
            $wait = [Diagnostics.Stopwatch]::StartNew()
            while ($shared.Queue.IsEmpty -and $wait.Elapsed.TotalSeconds -lt 15) { Start-Sleep -Milliseconds 200 }
            & $drain
            $rows.Clear()   # the stream's first sample is a warm-up, not part of the timed run

            $diskStart = Get-DiskRaw
            $envMap = @{ APPDATA = (Join-Path $attempt 'appdata'); SDC_PROFILE = $(if ($useProfile) { $profileDir } else { $null }) }
            $wall = [Diagnostics.Stopwatch]::StartNew()
            $startMs = $shared.Sw.ElapsedMilliseconds
            $child = Start-Child $exeToRun $fullArgs $attempt $envMap (Join-Path $attempt 'stdout.log') (Join-Path $attempt 'stderr.log')
            $proc = $child.Process
            $shared.Exempt[[int]$proc.Id] = $true
            $target = $null
            if (-not $A.Samply) { $target = $proc }
            $next = 0
            while (-not $proc.HasExited) {
                $now = $shared.Sw.ElapsedMilliseconds
                if (-not $target) {
                    $kid = Get-CimInstance Win32_Process -Filter "ParentProcessId=$($proc.Id)" | Where-Object { $_.Name -ieq [IO.Path]::GetFileName($A.Exe) } | Select-Object -First 1
                    if ($kid) { try { $target = [Diagnostics.Process]::GetProcessById([int]$kid.ProcessId); $shared.Exempt[[int]$kid.ProcessId] = $true } catch { } }
                }
                if ($target) {
                    try {
                        $target.Refresh()
                        $sampWriter.WriteLine(('{0},{1},{2},{3},{4},{5}' -f ($now - $startMs), $target.Id, $target.WorkingSet64, $target.PrivateMemorySize64, $target.Threads.Count, [int64]$target.TotalProcessorTime.TotalMilliseconds))
                    } catch { }
                }
                & $drain
                $next += 250
                $sleep = ($startMs + $next) - $shared.Sw.ElapsedMilliseconds
                if ($sleep -gt 0) { Start-Sleep -Milliseconds $sleep }
            }
            Stop-Child $child
            $wall.Stop()
            $endMs = $shared.Sw.ElapsedMilliseconds
            $diskEnd = Get-DiskRaw
            Start-Sleep -Milliseconds 1100   # let the stream deliver the last second
            & $drain
        } finally {
            if ($job) { Stop-Job $job; Remove-Job $job -Force }
            $contWriter.Dispose(); $sampWriter.Dispose()
        }
        $code = $proc.ExitCode
        $meta['exitCode'] = $code
        $meta['externalWallMs'] = $wall.ElapsedMilliseconds
        $meta['finished'] = $true
        $meta['disk'] = [ordered]@{
            start = [ordered]@{ corpus = (Get-DiskFigure $diskStart $A.Corpus); output = (Get-DiskFigure $diskStart $out) }
            end = [ordered]@{ corpus = (Get-DiskFigure $diskEnd $A.Corpus); output = (Get-DiskFigure $diskEnd $out) }
        }
        $timed = @($rows | Where-Object { $_.t_ms -ge $startMs -and $_.t_ms -le $endMs + 1200 })
        $verdict = Get-ContentionVerdict $timed $baseline ([bool]$A.Calibrate)
        $meta['contention'] = $verdict
        if ($A.Calibrate) {
            if ($verdict.samples -eq 0 -or $verdict.reasons.Count) { throw ('calibration unusable, baseline.json not written: ' + ($verdict.reasons -join '; ')) }
            Write-NewJson $baselinePath ([ordered]@{ otherCpuCores = $verdict.otherCpuCoresMean; otherIoBytesPerSec = $verdict.otherIoBytesPerSecMean; attempt = $name; samples = $verdict.samples; logicalCores = [Environment]::ProcessorCount })
        }
        $errText = [IO.File]::ReadAllText((Join-Path $attempt 'stderr.log'))
        if ($A.Samply -and $code -ne 0 -and $errText -match '(?i)elevat|administrator|access (is )?denied|privilege') {
            $meta['samply'] = 'needs elevation'
            Say 'samply needs elevation: the owner runs this command from an elevated shell; stopping'
            return 5
        }
        if ($A.Samply) { $meta['samply'] = 'recorded' }

        # outputs: hashes in output_key form, and for noop the file lists after
        $outRoot = [IO.Path]::GetFullPath($out).TrimEnd('\') + '\'
        $files = @([IO.Directory]::EnumerateFiles($out, '*.dds', [IO.SearchOption]::AllDirectories))
        $hashRows = $files | ForEach-Object -ThrottleLimit 4 -Parallel {
            $prefix = $using:outRoot
            $key = $_.Substring($prefix.Length).Replace('\', '/').ToLowerInvariant()
            '{0},{1}' -f $key, (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
        }
        Write-NewFile (Join-Path $attempt 'hashes.csv') ("path,sha256`n" + (Join-Lines (Sort-Ordinal $hashRows)))
        $meta['ddsFiles'] = $files.Count
        if ($cohort -eq 'warm-noop') { Write-NewFile (Join-Path $attempt 'files-after.csv') ("path,size,mtime_utc_ticks`n" + (Join-Lines (Get-FileList $out))) }

        Write-NewJson (Join-Path $A.RunRoot "last-run-$name.json") ([ordered]@{
                attempt = $name; corpus = $A.Corpus; cohort = $cohort; finished = $true; killed = $false; exitCode = $code
                endedUnixMs = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
            })
        Say ('exit {0}, wall {1} ms, contention: {2}' -f $code, $wall.ElapsedMilliseconds, $(if ($verdict.contaminated) { 'CONTAMINATED ' + ($verdict.reasons -join '; ') } else { 'clean' }))
        Say $DetectionLimit
        if ($code -ne 0) { return $code }
        if ($verdict.contaminated) { return 3 }
        return 0
    } catch {
        $meta['error'] = $_.Exception.Message
        Say "attempt stopped: $($_.Exception.Message)"
        return 1
    } finally {
        Write-NewJson (Join-Path $attempt 'meta.json') $meta
    }
}

# ---------------------------------------------------------------- entry

if ($SelfTest) { Invoke-SelfTest }

if (-not (Test-Label $Label)) { throw "refused: -Label must match ^[A-Za-z0-9][A-Za-z0-9-]{0,31}`$ (got '$Label')" }
if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) { throw "refused: exe not found: $Exe" }
if (-not (Test-Path -LiteralPath $Corpus -PathType Container)) { throw "refused: corpus folder not found: $Corpus" }
$Exe = [IO.Path]::GetFullPath($Exe)
$Corpus = [IO.Path]::GetFullPath($Corpus).TrimEnd('\')
if (-not $RunRoot) { $RunRoot = Join-Path 'F:\ai\dds\bench\profile\runs' ((Get-Date).ToString('yyyyMMdd') + '-' + $Label) }
$RunRoot = [IO.Path]::GetFullPath($RunRoot).TrimEnd('\')
$why = Get-RunRootProblem $RunRoot (Get-ProtectedList)
if ($why) { throw "refused: RunRoot $RunRoot $why" }

$dry = ($PSCmdlet.ParameterSetName -eq 'Run') -and $DryRun
if (-not $dry -and -not (Test-Path -LiteralPath $RunRoot)) { New-Item -ItemType Directory -Path $RunRoot | Out-Null }

if ($PSCmdlet.ParameterSetName -eq 'Expected') {
    Invoke-Expected @{ Exe = $Exe; Corpus = $Corpus; Label = $Label; RunRoot = $RunRoot; Set = $Set }
    exit 0
}
$code = Invoke-Attempt @{ Exe = $Exe; Corpus = $Corpus; Label = $Label; Cohort = $Cohort; Build = $Build; RunRoot = $RunRoot; Set = $Set; CopyFrom = $CopyFrom; StaleEdge = $StaleEdge; Samply = $Samply; Calibrate = [bool]$Calibrate; ExpectedBytes = $ExpectedBytes } $dry
exit $code

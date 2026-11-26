<#
.SYNOPSIS
  Engine profiling runner (plan 2026-11-21-engine-profile, Task 4). One attempt per call. Nothing in this script
  deletes, empties, moves or overwrites a file or folder (EP-R3), with ONE exception: -PruneOutputs (EP-O5,
  which amends EP-R3) deletes the .dds outputs an accepted attempt hashed, and replaces that attempt's meta.json
  with a copy that records the prune (the original is kept as meta.before-prune.json, created new). Every folder
  is created new (the create fails when it exists), and every file it writes is created new, except that one
  meta.json replace. Old runs are removed by hand.

.DESCRIPTION
  Four parameter sets, each with only its own parameters (PowerShell rejects a mix).

  Run:       -Exe <sdc.exe> -Corpus <folder> -Label <name>
             -Cohort <warmup|warm-fresh|warm-noop|warm-stale|cold-fresh|overhead|parity>
             -Build <release|base|release-off|profiling> [-RunRoot <folder>] [-Set <string[]>]
             [-CopyFrom <attempt name>] [-StaleEdge <clamp|wrap>] [-Samply <samply.exe>] [-Calibrate] [-DryRun]
             [-Validate]  adds --validate to the command and records validate: true in meta.json; fresh runs
             only (EP-C8): refused with -CopyFrom and for warm-noop and warm-stale.
             [-ExpectedBytes <n>]  size of the expected output; required for the first attempt of a corpus and
             build in a RunRoot, later ones size the free-space check from the newest earlier attempt's out.
             Nothing reads the corpus outside the command.
  Expected:  -Expected -Exe <base sdc.exe> -Corpus <folder> -Label <name> -RunRoot <folder> [-Set <string[]>]
             Dry run of the base build into a new empty folder under RunRoot with frozen APPDATA. Writes, each
             created new: expected-<Label>.txt, planned-not-written-<Label>.txt, skipped-inputs-<Label>.txt,
             expected-<Label>-events.txt. Exit 1 (unresolved or blocked files) counts as finished when batch_done
             shows nothing failed or cancelled (EP-C11); any other non-zero exit throws with the reason.
  Finished:  an attempt finished when the child ran to its end and exited 0, or exited 1 with only the known
             skips (EP-C11): its stderr done lines show nothing failed, cancelled or failing validation, and its
             skipped set equals the dry run's planned-not-written-<Label>.txt (the -Expected run of the same
             label in RunRoot). meta.json keeps the child's exitCode and the verdict in a knownSkips block.
             -CopyFrom, the free-space figure, the corpus reference and the prune read this, not exit 0 alone.
             The runner exits with the child's codes other than 0 and 1 first, then 3 (contaminated), then 4 (not
             cold), then the child's 0 or 1: 3 and 4 take precedence over the child's 1.
  Prune:     -PruneOutputs -RunRoot <folder> -Attempt <attempt name> -Reference <attempt name> [-Keep <names>]
             EP-O5: deletes the attempt's .dds files under <attempt>\out, and nothing else, once its hashes.csv
             equals the reference attempt's. The reference must be a finished base-build attempt of the same
             corpus. Refused for the corpus reference (the oldest finished base-build
             attempt of the attempt's corpus in RunRoot, EP-R4; kept whole), the -Reference itself, a name in
             -Keep, an attempt that did not finish, one already pruned, an
             empty reference, a .dds on disk that hashes.csv lacks (or the reverse), and an out tree holding a
             junction or link. Before deleting, meta.json is copied to meta.before-prune.json (created new) and
             replaced by a copy with a "prune" block (reference, files, bytes, time).
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
  The script never elevates, downloads or installs anything. Cold attempts need a cold stamp under 15 minutes
  old that no earlier attempt used: an owner-run tools\profile-purge.ps1 stamp (purge-*.json), or a
  tools\profile-evict.ps1 stamp (evict-*.json, EP-C5) of a folder corpus equal to -Corpus. meta.json records the
  stamp (purgeStamp, either kind) and coldMethod. After an evict stamp the attempt's disk read bytes on the corpus
  disk must reach 90 % of the stamp's corpus bytes, else meta.json says cold = false and the exit code is 4. A
  purge stamp carries no byte count (the runner never reads the corpus), so that check is not applied to it.
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
    [Parameter(ParameterSetName = 'Prune', Mandatory)]
    [string]$RunRoot,
    [Parameter(ParameterSetName = 'Prune', Mandatory)]
    [switch]$PruneOutputs,
    [Parameter(ParameterSetName = 'Prune', Mandatory)]
    [string]$Attempt,
    [Parameter(ParameterSetName = 'Prune', Mandatory)]
    [string]$Reference,
    [Parameter(ParameterSetName = 'Prune')]
    [string[]]$Keep = @(),
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
    [Parameter(ParameterSetName = 'Run')]
    [switch]$Validate,
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

# A property of a read-back JSON object, or a key of a dictionary (meta.json before it is written), or $null.
function Get-Prop($obj, [string]$name) {
    if ($obj -is [Collections.IDictionary]) { if ($obj.Contains($name)) { return $obj[$name] } else { return $null } }
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
    if (-not (Test-AttemptFinished $m)) { return 'did not finish (exit 0, or exit 1 with only the known skips, EP-C11)' }
    if ((Get-Prop (Get-Prop $m 'contention') 'contaminated') -ne $false) { return 'is contaminated or has no contention verdict' }
    if (-not [string]::Equals((Get-Prop $m 'corpus'), $corpus, 'OrdinalIgnoreCase')) { return 'is of another corpus' }
    if ((Get-Prop $m 'exeSha256') -ne $exeSha) { return 'was run with another exe' }
    if (Get-Prop $m 'prune') { return 'was pruned: its DDS outputs are deleted (EP-O5)' }
    $null
}

# Newest unused cold stamp under 15 minutes old, or $null (spec 7.4, EP-C5): a purge-*.json (standby list), or an
# evict-*.json of a folder corpus equal to $corpus (per-file eviction; a list-file stamp never matches a run).
# meta.json names the stamp in purgeStamp, whichever kind, so a stamp is used once.
function Get-ColdStamp([string]$runRoot, [datetime]$now, [string]$corpus) {
    $used = @{}
    foreach ($m in Get-ChildItem -LiteralPath $runRoot -Directory -ErrorAction SilentlyContinue) {
        $mp = Join-Path $m.FullName 'meta.json'
        if (Test-Path -LiteralPath $mp) {
            $s = Get-Prop (Read-Json $mp) 'purgeStamp'
            if ($s) { $used[$s] = $true }
        }
    }
    $best = $null
    $bestT = [datetime]::MinValue
    foreach ($f in Get-ChildItem -LiteralPath $runRoot -File -ErrorAction SilentlyContinue | Sort-Object Name) {
        if ($f.Name -notmatch '^(purge|evict)-(\d{8}-\d{6})\.json$') { continue }
        $kind = $Matches[1]
        $t = [datetime]::ParseExact($Matches[2], 'yyyyMMdd-HHmmss', $null)
        $age = ($now - $t).TotalMinutes
        if ($age -lt 0 -or $age -ge 15 -or $used.ContainsKey($f.Name)) { continue }
        if ($kind -eq 'evict') {
            try { $s = Read-Json $f.FullName }
            catch { Write-Warning "skipped the unreadable cold stamp $($f.Name): $($_.Exception.Message)"; continue }
            if ((Get-Prop $s 'method') -ne 'per-file-no-buffering' -or (Get-Prop $s 'corpusKind') -ne 'folder') { continue }
            if (-not [string]::Equals(([string](Get-Prop $s 'corpus')).TrimEnd('\'), $corpus.TrimEnd('\'), 'OrdinalIgnoreCase')) { continue }
        }
        if ($t -ge $bestT) { $best = $f.Name; $bestT = $t }
    }
    $best
}

# The cold verdict of a finished cold attempt (EP-C5): after an evict stamp the corpus disk's read bytes during
# the run must reach 90 % of the stamp's corpus bytes. A purge stamp has no byte count: the check is not applied.
function Get-ColdVerdict([string]$runRoot, [string]$stamp, $readStart, $readEnd) {
    if ($stamp -notlike 'evict-*') {
        return [ordered]@{ method = 'standby-purge'; check = 'not applied: a purge stamp carries no corpus byte count (the runner never reads the corpus)' }
    }
    $s = Read-Json (Join-Path $runRoot $stamp)
    $corpusBytes = [int64](Get-Prop $s 'bytes')
    $v = [ordered]@{ method = 'per-file-no-buffering'; corpusBytes = $corpusBytes; diskReadBytes = $null; ratio = $null; cold = $false; reason = $null }
    if ($readStart.name -like 'n/a*' -or $readEnd.name -like 'n/a*') {
        $v.reason = "the corpus drive has no physical-disk counter ($($readStart.name))"
        return $v
    }
    $read = [int64]([uint64]$readEnd.read - [uint64]$readStart.read)
    $v.diskReadBytes = $read
    if ($corpusBytes -le 0) { $v.reason = 'the stamp records no corpus bytes'; return $v }
    $v.ratio = [math]::Round($read / $corpusBytes, 4)
    $v.cold = ($read -ge 0.9 * $corpusBytes)
    if (-not $v.cold) { $v.reason = ('the run read {0:N0} bytes from the corpus disk, under 90 % of the {1:N0} corpus bytes' -f $read, $corpusBytes) }
    $v
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
    $ksOk = @{ ok = $true; reason = $null }; $ksBad = @{ ok = $false; reason = '1 planned-not-written key(s) not skipped' }
    $exit1 = { param($ks) $c = $good.Clone(); $c['exitCode'] = 1; $c['knownSkips'] = $ks; $c }
    New-Meta 'exit1-known' (& $exit1 $ksOk)
    New-Meta 'exit1-other' (& $exit1 $ksBad)
    Case 'CopyFrom naming an exit-1 attempt with only the known skips is accepted (EP-C11)' { -not (Get-CopyFromProblem $rr 'exit1-known' $corpus $sha) }
    Case 'CopyFrom naming an exit-1 attempt whose skips differ is refused' { (Get-CopyFromProblem $rr 'exit1-other' $corpus $sha) -like '*did not finish*' }

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
    Case 'the free-space figure takes an exit-1 attempt with only the known skips, not one whose skips differ (EP-C11)' {
        $e2 = Join-Path $t 'expected-bytes-exit1'; New-NewDir $e2
        foreach ($a in @(@('20260101-000001-a-warm-fresh-1-aaaaaaaa', 200, $true), @('20260101-000002-a-warm-fresh-2-bbbbbbbb', 500, $false))) {
            New-NewDir (Join-Path $e2 $a[0]); New-NewDir (Join-Path $e2 "$($a[0])\out")
            Write-NewFile (Join-Path $e2 "$($a[0])\out\a.dds") ('x' * $a[1])
            Write-NewJson (Join-Path $e2 "$($a[0])\meta.json") @{ corpus = $missingCorpus; build = 'release'; exitCode = 1; knownSkips = @{ ok = $a[2] } }
        }
        (Get-ExpectedBytesFigure $e2 $missingCorpus 'release' 4096) -eq 200
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

    # EP-C11: exit 1 with only the known skips counts as finished
    $doneJson = { param($failed, $cancelled) '{"kind":"batch_done","converted":0,"up_to_date":0,"skipped":2,"skipped_intentional":0,"failed":' + $failed + ',"validation_failed":0,"checked":0,"checked_failed":0,"cancelled":' + $cancelled + '}' }
    $known = @(
        '{"kind":"file_planned","path":"C:/c/textures/a.png","job_key":"textures/a.dds","job":{"status":{"state":"ready"}}}',
        '{"kind":"file_planned","path":"C:/c/textures/u.png","job_key":"textures/u.dds","job":{"status":{"state":"unresolved","needs":["subtype"]}}}',
        '{"kind":"file_planned","path":"C:/c/textures/b.png","job_key":"textures/b.dds","job":{"status":{"state":"blocked"}}}',
        '{"kind":"file_skipped","path":"C:/c/textures/u.png","job_key":"textures/u.dds","skip":"unresolved","code":"SUG-LOW-CONFIDENCE","reason":"unknown suffix"}',
        '{"kind":"file_skipped","path":"C:/c/textures/b.png","job_key":"textures/b.dds","skip":"blocked","code":"LOSS-ALPHA-DATA","reason":"blocked by LOSS-ALPHA-DATA"}',
        '{"kind":"file_skipped","path":"C:/c/textures/a_ao.png","job_key":"textures/a.dds","skip":"packed_source","code":null,"reason":"packed into a"}',
        (& $doneJson 0 '[]')
    )
    $kplan = ConvertFrom-PlanStream $known 'C:\c'
    $kr = Join-Path $t 'known-skips'; New-NewDir $kr
    [void](Write-ExpectedLists $kr 'fx' $kplan)
    $okLog = @(
        'warning C:/c/textures/u.png: SUG-LOW-CONFIDENCE unknown suffix',
        'ok    C:/c/textures/a.png -> C:/o/textures/a.dds',
        'skip  C:/c/textures/u.png: SUG-LOW-CONFIDENCE unknown suffix',
        'skip  C:/c/textures/b.png: LOSS-ALPHA-DATA blocked by LOSS-ALPHA-DATA; acknowledge with --allow <code> or change the setting',
        'done: 1 converted, 0 up to date, 2 skipped (0 on purpose), 0 failed, validation off, 0 cancelled'
    )
    $ksv = { param($lines, $label = 'fx') Get-KnownSkipsVerdict $kr $label $lines 'C:\c' }
    $withDone = { param($done) @($okLog | Where-Object { -not $_.StartsWith('done: ') }) + $done }
    Case 'expected exit: 0 passes, and 1 passes when batch_done shows nothing failed or cancelled' {
        $null -eq (Get-ExpectedExitProblem 0 @()) -and $null -eq (Get-ExpectedExitProblem 1 $kplan.Done) -and @($kplan.Done).Count -eq 1 -and $kplan.Done[0].skipped -eq 2
    }
    Case 'expected exit: 1 with a failed file, a cancelled file or no batch_done throws a reason; 2 is refused' {
        $f = (ConvertFrom-PlanStream @((& $doneJson 1 '[]')) 'C:\c').Done
        $c = (ConvertFrom-PlanStream @((& $doneJson 0 '["C:/c/x.png"]')) 'C:\c').Done
        (Get-ExpectedExitProblem 1 $f) -like '*failed = 1*' -and (Get-ExpectedExitProblem 1 $c) -like '*cancelled = 1*' -and
        (Get-ExpectedExitProblem 1 @()) -like '*no batch_done*' -and (Get-ExpectedExitProblem 2 $kplan.Done) -like 'exit 2*'
    }
    Case 'known skips: exit 1 with the dry run''s skip set is finished' {
        $v = & $ksv $okLog
        $m = @{ finished = $true; exitCode = 1; knownSkips = $v }
        $v.ok -and $v.expected -eq 2 -and $v.skipped -eq 2 -and $v.doneSkipped -eq 2 -and (Test-AttemptFinished $m) -and (Test-ExitAccepted $m)
    }
    Case 'known skips: up-to-date and kept lines of a noop run are written outputs, not skips' {
        $v = & $ksv (@('skip  C:/c/textures/a.png: up to date', 'skip  C:/c/textures/a.png: kept: written by an older encode version (--ev re-encodes it)') +
            (& $withDone 'done: 0 converted, 1 up to date, 2 skipped (0 on purpose), 0 failed, validation off, 0 cancelled'))
        $v.ok
    }
    Case 'known skips: a different skip set (one missing, one the dry run did not skip) is refused' {
        $less = & $ksv (@($okLog | Where-Object { $_ -notlike 'skip  C:/c/textures/b.png*' -and -not $_.StartsWith('done: ') }) + 'done: 1 converted, 0 up to date, 1 skipped (0 on purpose), 0 failed, validation off, 0 cancelled')
        $more = & $ksv (@('skip  C:/c/textures/z.png: SUG-LOW-CONFIDENCE unknown suffix') + (& $withDone 'done: 1 converted, 0 up to date, 3 skipped (0 on purpose), 0 failed, validation off, 0 cancelled'))
        $m = @{ finished = $true; exitCode = 1; knownSkips = $less }
        (-not $less.ok) -and $less.missing -eq 1 -and $less.reason -like '*not skipped*' -and ($less.examples -contains 'textures/b.dds') -and
        (-not $more.ok) -and $more.unmapped -eq 1 -and $more.reason -like '*did not skip*' -and -not (Test-AttemptFinished $m)
    }
    Case 'known skips: a failed file is refused (done count and FAIL line)' {
        $v = & $ksv (@('FAIL  C:/c/textures/a.png at Encode: boom') + (& $withDone 'done: 0 converted, 0 up to date, 2 skipped (0 on purpose), 1 failed, validation off, 0 cancelled'))
        (-not $v.ok) -and $v.reason -like '*failed = 1*' -and $v.reason -like '*1 FAIL line*'
    }
    Case 'known skips: a cancelled file, a failed validation or a failed check in place is refused' {
        $c = & $ksv (& $withDone 'done: 1 converted, 0 up to date, 2 skipped (0 on purpose), 0 failed, validation off, 2 cancelled')
        $vf = & $ksv (& $withDone 'done: 1 converted, 0 up to date, 2 skipped (0 on purpose), 0 failed, 1 failed validation, 0 cancelled')
        $cf = & $ksv (& $withDone 'done: 0 converted, 1 up to date, 2 skipped (0 on purpose), 0 failed, validation off, 1 checked in place (1 failed), 0 cancelled')
        $c0 = & $ksv (& $withDone 'done: 0 converted, 1 up to date, 2 skipped (0 on purpose), 0 failed, 0 failed validation, 1 checked in place (0 failed), 0 cancelled, 1 kept at a higher target')
        (-not $c.ok) -and $c.reason -like '*cancelled = 2*' -and (-not $vf.ok) -and $vf.reason -like '*validationFailed = 1*' -and
        (-not $cf.ok) -and $cf.reason -like '*checkedFailed = 1*' -and $c0.ok
    }
    Case 'known skips: no done line, an unparsed done line, or a done count unequal to the skip lines is refused' {
        $none = & $ksv @($okLog | Where-Object { -not $_.StartsWith('done: ') })
        $bad = & $ksv (& $withDone 'done: something else')
        $count = & $ksv (& $withDone 'done: 1 converted, 0 up to date, 3 skipped (0 on purpose), 0 failed, validation off, 0 cancelled')
        (-not $none.ok) -and $none.reason -like '*no batch_done*' -and (-not $bad.ok) -and $bad.reason -like '*did not parse*' -and
        (-not $count.ok) -and $count.reason -like '*count 3 skipped*'
    }
    Case 'known skips: without the dry run''s lists for the label, exit 1 is refused' {
        $v = & $ksv $okLog 'other'
        (-not $v.ok) -and $v.reason -like '*run -Expected with label other*'
    }
    Case 'runner exit: 3 and 4 take precedence over the child''s 1; other child codes come first' {
        ((Get-RunnerExit 1 $true $false) -eq 3) -and ((Get-RunnerExit 1 $false $true) -eq 4) -and ((Get-RunnerExit 1 $true $true) -eq 3) -and
        ((Get-RunnerExit 1 $false $false) -eq 1) -and ((Get-RunnerExit 0 $false $false) -eq 0) -and ((Get-RunnerExit 0 $true $false) -eq 3) -and
        ((Get-RunnerExit 2 $true $true) -eq 2)
    }
    Case 'runner exit: a warmup is clean at 0, or at 1 when the child''s 1 counts; never at 3 or 4' {
        (Test-CleanRunnerExit 0 $false) -and (Test-CleanRunnerExit 1 $true) -and -not (Test-CleanRunnerExit 1 $false) -and
        -not (Test-CleanRunnerExit 3 $true) -and -not (Test-CleanRunnerExit 4 $true)
    }
    Case 'samply elevation: an accepted exit 1 whose log names an "elevation" path is not read as needing elevation' {
        $log = 'skip  C:/c/textures/elevation_map.png: SUG-LOW-CONFIDENCE unknown suffix'
        -not (Test-SamplyNeedsElevation $true $true 1 $log) -and (Test-SamplyNeedsElevation $true $false 1 'Access is denied') -and
        -not (Test-SamplyNeedsElevation $false $false 1 'Access is denied') -and -not (Test-SamplyNeedsElevation $true $false 0 'Access is denied')
    }
    Case 'the attempt path records knownSkips for a child exit 1, returns through Get-RunnerExit and checks its warmup with the same verdict' {
        $text = [IO.File]::ReadAllText($PSCommandPath)
        $text.Contains("`$meta['knownSkips'] = Get-KnownSkipsVerdict") -and $text.Contains('return (Get-RunnerExit $code') -and
        $text.Contains('Test-CleanRunnerExit $wr $script:LastChildAccepted') -and $text.Contains('$why = Get-ExpectedExitProblem $ec $plan.Done') -and
        @([regex]::Matches($text, '-eq 0\) \{ return \$code \}|-ne 0\) \{ return \$code \}')).Count -eq 0
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
    Case 'a cold attempt without a stamp is refused' { -not (Get-ColdStamp $c0 $now $corpus) }
    Case 'a cold attempt with a stale stamp is refused' { -not (Get-ColdStamp $c1 $now $corpus) }
    Case 'a cold attempt with a stamp an earlier attempt names is refused' { -not (Get-ColdStamp $c2 $now $corpus) }
    Case 'a cold attempt with a fresh unused stamp is accepted' { (Get-ColdStamp $c3 $now $corpus) -eq $okStamp }

    # evict stamps (EP-C5): accepted like a purge stamp, for a folder corpus equal to -Corpus only
    $evictName = { param($min) 'evict-' + $now.AddMinutes(-$min).ToString('yyyyMMdd-HHmmss') + '.json' }
    $evictBody = { param($corp, $kind) [ordered]@{ stamp = 'x'; method = 'per-file-no-buffering'; corpus = $corp; corpusKind = $kind; files = 2; bytes = 1000 } }
    function New-EvictDir([string]$name, [double]$min, [string]$corp, [string]$kind) {
        $d = Join-Path $t $name; New-NewDir $d
        $n = & $evictName $min
        Write-NewJson (Join-Path $d $n) (& $evictBody $corp $kind)
        [pscustomobject]@{ Dir = $d; Stamp = $n }
    }
    $e1 = New-EvictDir 'evict-ok' 2 $corpus 'folder'
    $e2 = New-EvictDir 'evict-other' 2 'F:\corpus\two' 'folder'
    $e3 = New-EvictDir 'evict-list' 2 $corpus 'list'
    $e4 = New-EvictDir 'evict-stale' 20 $corpus 'folder'
    $e5 = New-EvictDir 'evict-used' 2 $corpus 'folder'
    New-NewDir (Join-Path $e5.Dir 'earlier-attempt'); Write-NewJson (Join-Path $e5.Dir 'earlier-attempt\meta.json') @{ purgeStamp = $e5.Stamp }
    $e6 = New-EvictDir 'evict-newest' 1 $corpus 'folder'
    $olderPurge = & $stampName 3
    Write-NewJson (Join-Path $e6.Dir $olderPurge) @{ x = 1 }
    Case 'a cold attempt with a fresh unused evict stamp of its corpus is accepted' { (Get-ColdStamp $e1.Dir $now $corpus) -eq $e1.Stamp }
    Case 'an evict stamp matches the corpus with or without a trailing backslash, in any case' { (Get-ColdStamp $e1.Dir $now ($corpus.ToUpperInvariant() + '\')) -eq $e1.Stamp }
    Case 'an evict stamp of another corpus is refused' { -not (Get-ColdStamp $e2.Dir $now $corpus) }
    Case 'an evict stamp of a list-file corpus is refused' { -not (Get-ColdStamp $e3.Dir $now $corpus) }
    Case 'a stale evict stamp is refused' { -not (Get-ColdStamp $e4.Dir $now $corpus) }
    Case 'an evict stamp an earlier attempt names is refused' { -not (Get-ColdStamp $e5.Dir $now $corpus) }
    Case 'the newest usable stamp wins across purge and evict stamps' { (Get-ColdStamp $e6.Dir $now $corpus) -eq $e6.Stamp }

    # the cold verdict: disk read bytes on the corpus disk against the evict stamp's corpus bytes
    $disk = { param($name, $read) [pscustomobject]@{ name = $name; read = [uint64]$read; write = [uint64]0 } }
    Case 'cold verdict: reading 95 % of the corpus bytes from disk is cold' {
        $v = Get-ColdVerdict $e1.Dir $e1.Stamp (& $disk '1 F:' 5000) (& $disk '1 F:' 5950)
        $v.cold -and $v.diskReadBytes -eq 950 -and $v.corpusBytes -eq 1000 -and $v.method -eq 'per-file-no-buffering'
    }
    Case 'cold verdict: exactly 90 % is cold' { (Get-ColdVerdict $e1.Dir $e1.Stamp (& $disk '1 F:' 0) (& $disk '1 F:' 900)).cold }
    Case 'cold verdict: reading 80 % is not cold, with a reason' {
        $v = Get-ColdVerdict $e1.Dir $e1.Stamp (& $disk '1 F:' 0) (& $disk '1 F:' 800)
        (-not $v.cold) -and $v.reason -like '*under 90 %*'
    }
    Case 'cold verdict: a corpus drive without a disk counter is not cold' {
        $v = Get-ColdVerdict $e1.Dir $e1.Stamp (& $disk 'n/a (Z: has no physical-disk counter)' 0) (& $disk 'n/a (Z: has no physical-disk counter)' 0)
        (-not $v.cold) -and $v.reason -like '*no physical-disk counter*'
    }
    Case 'cold verdict: Get-DiskFigure''s own objects, a drive with and without a counter' {
        $raw0 = @([pscustomobject]@{ name = '2 F:'; read = [uint64]1000; write = [uint64]0 })
        $raw1 = @([pscustomobject]@{ name = '2 F:'; read = [uint64]1950; write = [uint64]0 })
        $yes = Get-ColdVerdict $e1.Dir $e1.Stamp (Get-DiskFigure $raw0 'F:\corpus\one') (Get-DiskFigure $raw1 'F:\corpus\one')
        $no = Get-ColdVerdict $e1.Dir $e1.Stamp (Get-DiskFigure $raw0 'Q:\corpus') (Get-DiskFigure $raw1 'Q:\corpus')
        $yes.cold -and $yes.diskReadBytes -eq 950 -and (-not $no.cold) -and $no.reason -like '*no physical-disk counter*'
    }
    Case 'a malformed evict stamp is skipped with a warning; an older good stamp is still found' {
        $bad = New-EvictDir 'evict-malformed' 2 $corpus 'folder'
        Write-NewFile (Join-Path $bad.Dir (& $evictName 1)) '{ not json'
        $o = @(Get-ColdStamp $bad.Dir $now $corpus 3>&1)
        $warn = @($o | Where-Object { $_ -is [Management.Automation.WarningRecord] })
        $got = @($o | Where-Object { $_ -is [string] })
        $got.Count -eq 1 -and $got[0] -eq $bad.Stamp -and $warn.Count -eq 1 -and "$($warn[0])" -like '*unreadable cold stamp*'
    }
    Case 'cold verdict: a purge stamp records the method and no cold flag (the byte check is not applied)' {
        $v = Get-ColdVerdict $c3 $okStamp (& $disk '1 F:' 0) (& $disk '1 F:' 0)
        $v.method -eq 'standby-purge' -and -not $v.Contains('cold') -and $v.check -like 'not applied*'
    }

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
    # -Validate (EP-C8): fresh runs only; the command gains --validate once
    foreach ($c in 'warm-noop', 'warm-stale') {
        Case "-Validate is refused for $c" { (Get-ValidateProblem $true $c 'src') -like "*refused for $c*" -and (Get-ValidateProblem $true $c $null) -like "*refused for $c*" }
    }
    Case '-Validate is refused with -CopyFrom whatever the cohort' { (Get-ValidateProblem $true 'warm-fresh' 'src') -like '*with -CopyFrom*' }
    Case '-Validate is accepted for fresh cohorts without -CopyFrom' {
        -not @('warmup', 'warm-fresh', 'cold-fresh', 'overhead', 'parity' | Where-Object { Get-ValidateProblem $true $_ $null }).Count
    }
    Case 'without -Validate nothing is refused (noop with -CopyFrom stays allowed here)' { -not (Get-ValidateProblem $false 'warm-noop' 'src') }
    Case '-Validate adds --validate once to the convert command' {
        $a = Get-ConvertArgs 'C:\c' 'C:\o' @('reencode=true') (Get-ExtraArgs 'warm-fresh' 'clamp' $true)
        $b = Get-ConvertArgs 'C:\c' 'C:\o' @() (Get-ExtraArgs 'warm-fresh' 'clamp' $false)
        (@($a | Where-Object { $_ -eq '--validate' }).Count -eq 1) -and ($a -contains 'reencode=true') -and ($b -notcontains '--validate')
    }
    Case 'warm-stale keeps its edge setting and gets no --validate' {
        $s = Get-ExtraArgs 'warm-stale' 'wrap' $false
        ($s -join ' ') -eq '--set mips.edge=wrap'
    }
    Case 'meta.json records validate from the switch' {
        $text = [IO.File]::ReadAllText($PSCommandPath)
        $text.Contains('validate = [bool]$A.Validate') -and $text.Contains('Validate = [bool]$Validate')
    }

    # prune (EP-O5): only an accepted attempt's hashed .dds files go, and every refusal deletes nothing
    $pr = Join-Path $t 'prune'; New-NewDir $pr
    $rowsAB = @('textures/a.dds,aaa', 'textures/b.dds,bbb')
    $attMeta = @{ cohort = 'warm-fresh'; finished = $true; exitCode = 0; corpus = $corpus; exeSha256 = $sha; build = 'release'; contention = @{ contaminated = $false } }
    function New-PruneAttempt([string]$root, [string]$name, $meta, $rows, [string[]]$dds) {
        $d = Join-Path $root $name; New-NewDir $d; New-NewDir (Join-Path $d 'out'); New-NewDir (Join-Path $d 'out\_sdc')
        Write-NewJson (Join-Path $d 'meta.json') $meta
        if ($null -ne $rows) { Write-NewFile (Join-Path $d 'hashes.csv') ("path,sha256`n" + (Join-Lines $rows)) }
        Write-NewFile (Join-Path $d 'out\_sdc\manifest.jsonl') "{}`n"
        Write-NewFile (Join-Path $d 'stderr.log') "log`n"
        foreach ($k in $dds) {
            $p = Join-Path $d ('out\' + $k.Replace('/', '\'))
            $pd = Split-Path -Parent $p
            if (-not (Test-Path -LiteralPath $pd)) { New-Item -ItemType Directory -Path $pd | Out-Null }
            Write-NewFile $p "dds $k"
        }
        $d
    }
    $ab = @('textures/a.dds', 'textures/b.dds')
    $baseMeta = $attMeta.Clone(); $baseMeta['build'] = 'base'
    [void](New-PruneAttempt $pr 'ref' $baseMeta $rowsAB $ab)
    [void](New-PruneAttempt $pr 'ref2' $baseMeta $rowsAB $ab)
    [void](New-PruneAttempt $pr 'ref-empty' $baseMeta @() @())
    [void](New-PruneAttempt $pr 'ref-failed' ($baseMeta.Clone() | ForEach-Object { $_['exitCode'] = 1; $_ }) $rowsAB $ab)
    [void](New-PruneAttempt $pr 'ref-other' ($baseMeta.Clone() | ForEach-Object { $_['corpus'] = 'F:\corpus\two'; $_ }) $rowsAB $ab)
    $att = New-PruneAttempt $pr 'att' $attMeta $rowsAB $ab
    [void](New-PruneAttempt $pr 'att-keep' $attMeta $rowsAB $ab)
    [void](New-PruneAttempt $pr 'att-nohash' $attMeta $null $ab)
    [void](New-PruneAttempt $pr 'att-diff' $attMeta @('textures/a.dds,aaa', 'textures/b.dds,ccc') $ab)
    [void](New-PruneAttempt $pr 'att-extra' $attMeta $rowsAB ($ab + 'textures/c.dds'))
    [void](New-PruneAttempt $pr 'att-missing' $attMeta $rowsAB @('textures/a.dds'))
    [void](New-PruneAttempt $pr 'att-failed' ($attMeta.Clone() | ForEach-Object { $_['exitCode'] = 1; $_ }) $rowsAB $ab)
    $junc = New-PruneAttempt $pr 'att-junc' $attMeta $rowsAB $ab
    New-Sentinel 'junc-target\x.dds'
    New-Item -ItemType Junction -Path (Join-Path $junc 'out\link') -Target (Join-Path $t 'junc-target') | Out-Null
    $ddsLeft = { param($name) @([IO.Directory]::EnumerateFiles((Join-Path $pr "$name\out"), '*.dds', [IO.SearchOption]::AllDirectories)).Count }
    $refusedIntact = {
        param($name, $ref, $keep, $like)
        $before = & $ddsLeft $name
        $msg = ''
        try { [void](Invoke-PruneOutputs $pr $name $ref $keep) } catch { $msg = $_.Exception.Message }
        ($msg -like $like) -and ((& $ddsLeft $name) -eq $before) -and $before -gt 0 -and -not (Test-Path -LiteralPath (Join-Path $pr "$name\meta.before-prune.json"))
    }
    Case 'prune: the reference attempt itself is refused' { & $refusedIntact 'ref' 'ref' @() '*is the reference attempt*' }
    Case 'prune: the corpus reference is refused even against a release sibling named as -Reference' {
        & $refusedIntact 'ref' 'att-keep' @() '*is the corpus reference*'
    }
    Case 'prune: the corpus reference (oldest base) is refused even when another base attempt is named as -Reference' {
        & $refusedIntact 'ref' 'ref2' @() '*is the corpus reference*'
    }
    Case 'the corpus reference is the oldest finished base attempt of that corpus' {
        ((Get-CorpusReference $pr $corpus) -eq 'ref') -and ((Get-CorpusReference $pr 'F:\corpus\two\') -eq 'ref-other') -and
        ($null -eq (Get-CorpusReference $pr 'F:\corpus\none'))
    }
    $stamped = { param($k, $cohort) '20260101-00000{0}-t-{1}-{0}-0000000{0}' -f $k, $cohort }
    Case 'prune: a later base attempt (an overhead base) is pruned against the corpus reference' {
        $lr = Join-Path $t 'prune-later-base'; New-NewDir $lr
        $first = & $stamped 1 'overhead'; $later = & $stamped 2 'overhead'
        [void](New-PruneAttempt $lr $first $baseMeta $rowsAB $ab)
        [void](New-PruneAttempt $lr $later $baseMeta $rowsAB $ab)
        $r = Invoke-PruneOutputs $lr $later $first @()
        $r.Files -eq 2 -and @([IO.Directory]::EnumerateFiles((Join-Path $lr "$later\out"), '*.dds', [IO.SearchOption]::AllDirectories)).Count -eq 0 -and
        @([IO.Directory]::EnumerateFiles((Join-Path $lr "$first\out"), '*.dds', [IO.SearchOption]::AllDirectories)).Count -eq 2
    }
    Case 'a failed oldest base attempt is never the corpus reference; the next finished one is, and is refused' {
        $fr2 = Join-Path $t 'prune-failed-oldest'; New-NewDir $fr2
        $failed = & $stamped 1 'parity'; $refN = & $stamped 2 'overhead'; $third = & $stamped 3 'overhead'
        [void](New-PruneAttempt $fr2 $failed ($baseMeta.Clone() | ForEach-Object { $_['exitCode'] = 1; $_ }) $rowsAB $ab)
        [void](New-PruneAttempt $fr2 $refN $baseMeta $rowsAB $ab)
        [void](New-PruneAttempt $fr2 $third $baseMeta $rowsAB $ab)
        $msg = ''
        try { [void](Invoke-PruneOutputs $fr2 $refN $third @()) } catch { $msg = $_.Exception.Message }
        ((Get-CorpusReference $fr2 $corpus) -eq $refN) -and $msg -like '*is the corpus reference*' -and
        @([IO.Directory]::EnumerateFiles((Join-Path $fr2 "$refN\out"), '*.dds', [IO.SearchOption]::AllDirectories)).Count -eq 2
    }
    Case 'exit 1 with only the known skips: the base attempt is the corpus reference, a release attempt is pruned against it (EP-C11)' {
        $xr = Join-Path $t 'prune-exit1'; New-NewDir $xr
        $k1 = { param($m, $ok) $c = $m.Clone(); $c['exitCode'] = 1; $c['knownSkips'] = @{ ok = $ok }; $c }
        $bad = & $stamped 1 'parity'; $refN = & $stamped 2 'overhead'; $relN = & $stamped 3 'warm-fresh'; $relBad = & $stamped 4 'warm-fresh'
        [void](New-PruneAttempt $xr $bad (& $k1 $baseMeta $false) $rowsAB $ab)
        [void](New-PruneAttempt $xr $refN (& $k1 $baseMeta $true) $rowsAB $ab)
        [void](New-PruneAttempt $xr $relN (& $k1 $attMeta $true) $rowsAB $ab)
        [void](New-PruneAttempt $xr $relBad (& $k1 $attMeta $false) $rowsAB $ab)
        $m1 = ''; $m2 = ''
        try { [void](Invoke-PruneOutputs $xr $relBad $refN @()) } catch { $m1 = $_.Exception.Message }
        try { [void](Invoke-PruneOutputs $xr $refN $bad @()) } catch { $m2 = $_.Exception.Message }
        $r = Invoke-PruneOutputs $xr $relN $refN @()
        ((Get-CorpusReference $xr $corpus) -eq $refN) -and $r.Files -eq 2 -and $m1 -like '*did not finish*' -and $m2 -like '*is the corpus reference*' -and
        @([IO.Directory]::EnumerateFiles((Join-Path $xr "$relBad\out"), '*.dds', [IO.SearchOption]::AllDirectories)).Count -eq 2
    }
    Case 'prune: a -Reference that exited 1 with skips that differ is refused' {
        $xr2 = Join-Path $t 'prune-exit1-ref'; New-NewDir $xr2
        $a1 = & $stamped 1 'overhead'; $a2 = & $stamped 2 'warm-fresh'
        $rm = $baseMeta.Clone(); $rm['exitCode'] = 1; $rm['knownSkips'] = @{ ok = $false }
        [void](New-PruneAttempt $xr2 $a1 $rm $rowsAB $ab)
        [void](New-PruneAttempt $xr2 $a2 $attMeta $rowsAB $ab)
        $msg = ''
        try { [void](Invoke-PruneOutputs $xr2 $a2 $a1 @()) } catch { $msg = $_.Exception.Message }
        $msg -like "*reference $a1 did not finish*" -and $null -eq (Get-CorpusReference $xr2 $corpus)
    }
    Case 'prune: a -Reference that is not a base-build attempt is refused' { & $refusedIntact 'att-diff' 'att-keep' @() '*is not a base-build attempt*' }
    Case 'prune: a -Reference that did not finish with exit 0 is refused' { & $refusedIntact 'att-keep' 'ref-failed' @() '*reference ref-failed did not finish*' }
    Case 'prune: a -Reference of another corpus is refused' { & $refusedIntact 'att-keep' 'ref-other' @() '*of another corpus*' }
    Case 'prune: an attempt in -Keep is refused' { & $refusedIntact 'att-keep' 'ref' @('other', 'ATT-KEEP') '*-Keep list*' }
    Case 'prune: an attempt without hashes.csv is refused' { & $refusedIntact 'att-nohash' 'ref' @() '*no hashes.csv*' }
    Case 'prune: hashes that differ from the reference are refused' { & $refusedIntact 'att-diff' 'ref' @() '*differs from the reference*' }
    Case 'prune: a reference without hashes.csv rows is refused' { & $refusedIntact 'att' 'ref-empty' @() '*has no hashes.csv rows*' }
    Case 'prune: a .dds under out that hashes.csv lacks is refused' { & $refusedIntact 'att-extra' 'ref' @() '*not in hashes.csv*' }
    Case 'prune: a hashed file missing from disk is refused' { & $refusedIntact 'att-missing' 'ref' @() '*not on disk*' }
    Case 'prune: an attempt that did not finish with exit 0 is refused' { & $refusedIntact 'att-failed' 'ref' @() '*did not finish*' }
    Case 'prune: an out tree holding a junction is refused and the junction target is untouched' { & $refusedIntact 'att-junc' 'ref' @() '*junction or link*' }
    Case 'prune: names outside RunRoot or nested are refused' {
        $a = ''; $b = ''
        try { [void](Invoke-PruneOutputs $pr '..' 'ref' @()) } catch { $a = $_.Exception.Message }
        try { [void](Invoke-PruneOutputs $pr 'att' 'ref\x' @()) } catch { $b = $_.Exception.Message }
        ($a -like '*directly inside RunRoot*') -and ($b -like '*directly inside RunRoot*') -and ((& $ddsLeft 'att') -eq 2)
    }
    $metaPath = Join-Path $att 'meta.json'
    $origMetaHash = (Get-FileHash -LiteralPath $metaPath -Algorithm SHA256).Hash
    $origMeta = (Read-Json $metaPath) | ConvertTo-Json -Compress -Depth 8
    $ddsBytes = (([IO.FileInfo](Join-Path $att 'out\textures\a.dds')).Length + ([IO.FileInfo](Join-Path $att 'out\textures\b.dds')).Length)
    Case 'prune: an accepted attempt loses exactly its hashed .dds files and nothing else' {
        $r = Invoke-PruneOutputs $pr 'att' 'ref' @('att-keep')
        $r.Files -eq 2 -and $r.Bytes -eq $ddsBytes -and (& $ddsLeft 'att') -eq 0 -and
        (Test-Path -LiteralPath (Join-Path $att 'out\_sdc\manifest.jsonl')) -and (Test-Path -LiteralPath (Join-Path $att 'stderr.log')) -and
        (Test-Path -LiteralPath (Join-Path $att 'hashes.csv')) -and (Test-Path -LiteralPath (Join-Path $att 'out\textures') -PathType Container) -and
        -not (Test-Path -LiteralPath (Join-Path $att 'meta.prune-new.json'))
    }
    Case 'prune: meta.json is the original plus one prune block, and meta.before-prune.json is the original' {
        $m = Read-Json $metaPath
        $p = $m.prune
        $m.PSObject.Properties.Remove('prune')
        ((Get-FileHash -LiteralPath (Join-Path $att 'meta.before-prune.json') -Algorithm SHA256).Hash -eq $origMetaHash) -and
        (($m | ConvertTo-Json -Compress -Depth 8) -eq $origMeta) -and $p.reference -eq 'ref' -and $p.files -eq 2 -and $p.bytes -eq $ddsBytes -and (@($p.keep) -join ',') -eq 'att-keep'
    }
    Case 'prune: a second prune of the same attempt is refused' {
        $msg = ''
        try { [void](Invoke-PruneOutputs $pr 'att' 'ref' @()) } catch { $msg = $_.Exception.Message }
        $msg -like '*already pruned*'
    }
    Case 'prune: the reference and the other refused attempts still hold every .dds' {
        ((& $ddsLeft 'ref') -eq 2) -and ((& $ddsLeft 'att-keep') -eq 2) -and ((& $ddsLeft 'att-diff') -eq 2) -and ((& $ddsLeft 'att-extra') -eq 3)
    }
    Case 'a pruned warm-fresh is refused as a -CopyFrom source' { (Get-CopyFromProblem $pr 'att' $corpus $sha) -like '*pruned*' }
    Case 'the free-space figure of a pruned attempt adds back the pruned bytes' {
        $fr = Join-Path $t 'prune-figure'; New-NewDir $fr
        [void](New-PruneAttempt $fr 'b-ref' ($attMeta.Clone() | ForEach-Object { $_['build'] = 'base'; $_ }) $rowsAB $ab)
        $fa = New-PruneAttempt $fr 'a-att' $attMeta $rowsAB $ab
        $before = Get-DirBytes (Join-Path $fa 'out')
        [void](Invoke-PruneOutputs $fr 'a-att' 'b-ref' @())
        ((Get-DirBytes (Join-Path $fa 'out')) -lt $before) -and ((Get-ExpectedBytesFigure $fr $corpus 'release' 0) -eq $before)
    }
    Case 'prune: hashes.csv written by the real writer (mixed case, nested, a comma) is read back and pruned' {
        $wr = Join-Path $t 'prune-writer'; New-NewDir $wr
        $names = @('Textures\Clutter\Cup.DDS', 'textures\a,b.dds', 'Textures\Sub Dir\deep\X.dds')
        $made = @{}
        foreach ($nm in 'w-ref', 'w-att') {
            $m = if ($nm -eq 'w-ref') { $baseMeta } else { $attMeta }
            $d = New-PruneAttempt $wr $nm $m $null @()
            foreach ($k in $names) {
                $p = Join-Path $d ('out\' + $k)
                $pd = Split-Path -Parent $p
                if (-not (Test-Path -LiteralPath $pd)) { New-Item -ItemType Directory -Path $pd | Out-Null }
                Write-NewFile $p "dds $k"
            }
            $made[$nm] = Write-HashesCsv $d (Join-Path $d 'out')
        }
        $rows = Get-HashRows (Join-Path $wr 'w-att')
        $keys = @($rows | ForEach-Object { Get-HashRowKey $_ }) -join '|'
        $r = Invoke-PruneOutputs $wr 'w-att' 'w-ref' @()
        $made['w-att'] -eq 3 -and $keys -eq 'textures/a,b.dds|textures/clutter/cup.dds|textures/sub dir/deep/x.dds' -and $r.Files -eq 3 -and
        @([IO.Directory]::EnumerateFiles((Join-Path $wr 'w-att\out'), '*.dds', [IO.SearchOption]::AllDirectories)).Count -eq 0 -and
        @([IO.Directory]::EnumerateFiles((Join-Path $wr 'w-ref\out'), '*.dds', [IO.SearchOption]::AllDirectories)).Count -eq 3
    }
    Case 'the one file deletion and the one overwrite in this script sit in Invoke-PruneOutputs' {
        $text = [IO.File]::ReadAllText($PSCommandPath)
        $start = $text.IndexOf('function Invoke-' + 'PruneOutputs(')
        $end = $text.IndexOf("`n# ----", $start)
        $body = $text.Substring($start, $end - $start)
        $ok = $start -ge 0
        # static calls on [IO.File] and instance calls on a FileInfo alike (delete, and move or MoveTo)
        foreach ($group in @(@(('::Del' + 'ete('), ('.Del' + 'ete(')), @(('::Mo' + 've('), ('.Mo' + 'veTo(')))) {
            $all = 0; $inBody = 0
            foreach ($word in $group) {
                $all += [regex]::Matches($text, [regex]::Escape($word)).Count
                $inBody += [regex]::Matches($body, [regex]::Escape($word)).Count
            }
            if ($all -ne 1 -or $inBody -ne 1) { throw "$($group -join ' or ') occurs $all time(s), $inBody in the prune" }
        }
        $ok -and $body.Contains('[IO.File]::Del' + 'ete($f)') -and $body.Contains('[IO.File]::Mo' + 've($tmp, $metaPath, $true)') -and
        -not [regex]::IsMatch($text, '(?i)\[IO\.Directory\]::' + 'Del')
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

# The output key of a file under out: its path below out, forward slashes, lower case (output_key form). The one
# key rule: hashes.csv is written with it and the prune maps the files on disk with it.
function Get-OutputKey([string]$outRoot, [string]$path) {
    $path.Substring($outRoot.Length).Replace('\', '/').ToLowerInvariant()
}

# Every .dds under out, with its output key.
function Get-DdsEntries([string]$out) {
    $outRoot = [IO.Path]::GetFullPath($out).TrimEnd('\') + '\'
    foreach ($f in [IO.Directory]::EnumerateFiles($out, '*.dds', [IO.SearchOption]::AllDirectories)) {
        [pscustomobject]@{ Key = (Get-OutputKey $outRoot ([IO.Path]::GetFullPath($f))); Path = $f }
    }
}

# The key of a hashes.csv row ("<key>,<sha256>"; a key may hold a comma, the hash never does).
function Get-HashRowKey([string]$row) { $row.Substring(0, $row.LastIndexOf(',')) }

# Writes <attempt>\hashes.csv, created new: a header, then "<output key>,<sha256>" per .dds under out, sorted
# ordinally. Returns the file count.
function Write-HashesCsv([string]$attempt, [string]$out) {
    $entries = @(Get-DdsEntries $out)
    $rows = $entries | ForEach-Object -ThrottleLimit 4 -Parallel {
        '{0},{1}' -f $_.Key, (Get-FileHash -LiteralPath $_.Path -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    Write-NewFile (Join-Path $attempt 'hashes.csv') ("path,sha256`n" + (Join-Lines (Sort-Ordinal $rows)))
    $entries.Count
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
        if ([string]::Equals((Get-Prop $m 'corpus'), $corpus, 'OrdinalIgnoreCase') -and (Get-Prop $m 'build') -eq $build -and (Test-ExitAccepted $m)) {
            # A pruned attempt's out lost its DDS files: add back the bytes the prune recorded (EP-O5).
            return ((Get-DirBytes $out) + [int64](Get-Prop (Get-Prop $m 'prune') 'bytes'))
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

# The corpus root in event form (forward slashes, ending in a slash), and an event path made relative to it (an
# input path outside the corpus is kept as it is). The one rule for the dry run's lists and the attempt's log.
function Get-EventCorpusRoot([string]$corpusDir) {
    ([IO.Path]::GetFullPath($corpusDir).TrimEnd('\') + '\').Replace('\', '/')
}

function Get-EventRelPath([string]$corpusRoot, $p) {
    $s = [string]$p
    $n = $s.Replace('\', '/')
    if ($n.StartsWith($corpusRoot, 'OrdinalIgnoreCase')) { $n.Substring($corpusRoot.Length) } else { $s }
}

# Parses the dry run's JSONL (spec 7.3). Missing fields are tolerated (StrictMode). Done holds each batch_done's
# failure counts (EP-C11).
function ConvertFrom-PlanStream([string[]]$lines, [string]$corpusDir) {
    $corpusRoot = Get-EventCorpusRoot $corpusDir
    $ready = New-Object System.Collections.Generic.List[string]
    $notWritten = New-Object System.Collections.Generic.List[string]
    $skipped = New-Object System.Collections.Generic.List[string]
    $events = New-Object System.Collections.Generic.List[string]
    $done = New-Object System.Collections.Generic.List[object]
    foreach ($line in $lines) {
        if (-not $line.Trim().StartsWith('{')) { continue }
        $e = $line | ConvertFrom-Json
        $path = Get-EventRelPath $corpusRoot (Get-Prop $e 'path')
        switch (Get-Prop $e 'kind') {
            'batch_done' {
                $done.Add([pscustomobject]@{
                        failed = [int64](Get-Prop $e 'failed'); cancelled = @(Get-Prop $e 'cancelled' | Where-Object { $null -ne $_ }).Count
                        validationFailed = [int64](Get-Prop $e 'validation_failed'); checkedFailed = [int64](Get-Prop $e 'checked_failed')
                        skipped = [int64](Get-Prop $e 'skipped')
                    })
            }
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
    [pscustomobject]@{ Ready = $ready.ToArray(); NotWritten = $notWritten.ToArray(); Skipped = $skipped.ToArray(); Events = $events.ToArray(); Done = $done.ToArray() }
}

# ---------------------------------------------------------------- exit 1 with only the known skips (EP-C11)

# Reason a batch's done records show more than skips, or $null: no batch_done at all, a failed file, a cancelled
# file, or a failed validation (a validation failure exits 4 on its own; exit 1 must not hide it).
function Get-DoneProblem($done) {
    $d = @($done | Where-Object { $null -ne $_ })
    if ($d.Count -eq 0) { return 'no batch_done record' }
    $sum = { param($f) [int64](($d | Measure-Object -Property $f -Sum).Sum) }
    $why = @()
    foreach ($f in 'failed', 'cancelled', 'validationFailed', 'checkedFailed') {
        $n = & $sum $f
        if ($n) { $why += "batch_done shows $f = $n" }
    }
    if ($why.Count) { return ($why -join '; ') }
    $null
}

# The dry run's exit (EP-C11): 0, or 1 whose batch_done shows nothing failed or cancelled. Reason, or $null.
function Get-ExpectedExitProblem([int]$exitCode, $done) {
    if ($exitCode -eq 0) { return $null }
    if ($exitCode -ne 1) { return "exit $exitCode (only 0, or 1 with nothing failed or cancelled, counts as finished)" }
    $why = Get-DoneProblem $done
    if ($why) { return "exit 1 with more than skips: $why" }
    $null
}

# A real attempt's human log (stderr: the command has no --json): the done lines (the human form of batch_done),
# the skip lines (relative input paths) and the FAIL lines. Up-to-date and kept outputs log a skip line with no
# code ("up to date...", "kept: ..."); they are written outputs, not skips, and are left out. A raw map packed
# into a recipe logs no line at all. A done line that does not parse is a problem, never a pass.
function ConvertFrom-HumanLog([string[]]$lines, [string]$corpusDir) {
    $corpusRoot = Get-EventCorpusRoot $corpusDir
    $done = New-Object System.Collections.Generic.List[object]
    $skips = New-Object System.Collections.Generic.List[string]
    $fails = 0; $unparsed = 0
    $doneRx = '^done: (\d+) converted, (\d+) up to date, (\d+) skipped \((\d+) on purpose\), (\d+) failed, (.*?)(\d+) cancelled(,.*)?$'
    foreach ($line in $lines) {
        if ($line.StartsWith('done: ')) {
            $m = [regex]::Match($line, $doneRx)
            if (-not $m.Success) { $unparsed++; continue }
            $mid = $m.Groups[6].Value
            $vf = [regex]::Match($mid, '(\d+) failed validation')
            $cf = [regex]::Match($mid, 'checked in place \((\d+) failed\)')
            $done.Add([pscustomobject]@{
                    failed = [int64]$m.Groups[5].Value; cancelled = [int64]$m.Groups[7].Value; skipped = [int64]$m.Groups[3].Value
                    validationFailed = $(if ($vf.Success) { [int64]$vf.Groups[1].Value } else { [int64]0 })
                    checkedFailed = $(if ($cf.Success) { [int64]$cf.Groups[1].Value } else { [int64]0 })
                })
        } elseif ($line.StartsWith('skip  ')) {
            # "skip  <path>: [CODE ]reason"; a Windows path holds no ':' after its drive, so the first ': ' ends it
            $m = [regex]::Match($line, '^skip  (.+?): (.*)$')
            if (-not $m.Success) { $unparsed++; continue }
            if ($m.Groups[2].Value -match '^(up to date|kept: )') { continue }
            $skips.Add((Get-EventRelPath $corpusRoot $m.Groups[1].Value))
        } elseif ($line.StartsWith('FAIL  ')) { $fails++ }
    }
    [pscustomobject]@{ Done = $done.ToArray(); Skips = $skips.ToArray(); Fails = $fails; Unparsed = $unparsed }
}

# The known-skips verdict of a real attempt that exited 1 (EP-C11), for meta.json's knownSkips block. ok is true
# only when the done lines show nothing failed, cancelled or failing validation, no FAIL line, and the attempt's
# skipped set equals the dry run's planned-not-written-<label>.txt: each skip line's input path is mapped to its
# job key through skipped-inputs-<label>.txt (packed_source rows left out: they log no line), and the key sets
# must match both ways, with the done lines' skipped count equal to the skip lines.
function Get-KnownSkipsVerdict([string]$runRoot, [string]$label, [string[]]$logLines, [string]$corpusDir) {
    $pnwName = "planned-not-written-$label.txt"; $skName = "skipped-inputs-$label.txt"
    $v = [ordered]@{ ok = $false; list = $pnwName; expected = $null; skipped = $null; doneSkipped = $null; missing = $null; extra = $null; unmapped = $null; examples = @(); reason = $null }
    $why = @()
    $pnwPath = Join-Path $runRoot $pnwName; $skPath = Join-Path $runRoot $skName
    if (-not (Test-Path -LiteralPath $pnwPath -PathType Leaf) -or -not (Test-Path -LiteralPath $skPath -PathType Leaf)) {
        $v.reason = "no $pnwName or $skName in RunRoot: run -Expected with label $label first"
        return $v
    }
    $want = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    foreach ($l in [IO.File]::ReadAllLines($pnwPath)) { if ($l) { [void]$want.Add($l.Split("`t")[0]) } }
    $byPath = New-Object 'System.Collections.Generic.Dictionary[string,System.Collections.Generic.List[string]]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($l in [IO.File]::ReadAllLines($skPath)) {
        if (-not $l) { continue }
        $c = $l.Split("`t")
        if ($c.Count -lt 4 -or $c[1] -eq 'packed_source') { continue }
        if (-not $byPath.ContainsKey($c[0])) { $byPath[$c[0]] = New-Object System.Collections.Generic.List[string] }
        $byPath[$c[0]].Add($c[3])
    }
    $log = ConvertFrom-HumanLog $logLines $corpusDir
    $dp = Get-DoneProblem $log.Done
    if ($dp) { $why += $dp }
    if ($log.Fails) { $why += "$($log.Fails) FAIL line(s) in the log" }
    if ($log.Unparsed) { $why += "$($log.Unparsed) done or skip line(s) did not parse" }
    $got = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    $unmapped = @()
    foreach ($p in $log.Skips) {
        if ($byPath.ContainsKey($p)) { foreach ($k in $byPath[$p]) { [void]$got.Add($k) } } else { $unmapped += $p }
    }
    $missing = Sort-Ordinal @($want | Where-Object { -not $got.Contains($_) })
    $extra = Sort-Ordinal @($got | Where-Object { -not $want.Contains($_) })
    $doneSkipped = [int64]0
    foreach ($d in $log.Done) { $doneSkipped += $d.skipped }
    $v.expected = $want.Count; $v.skipped = $log.Skips.Count; $v.doneSkipped = $doneSkipped
    $v.missing = $missing.Count; $v.extra = $extra.Count; $v.unmapped = $unmapped.Count
    if ($unmapped.Count) { $why += "$($unmapped.Count) skipped input(s) the dry run did not skip" }
    if ($missing.Count) { $why += "$($missing.Count) planned-not-written key(s) not skipped" }
    if ($extra.Count) { $why += "$($extra.Count) skipped key(s) not in $pnwName" }
    if ($doneSkipped -ne $log.Skips.Count) { $why += "the done lines count $doneSkipped skipped, the log has $($log.Skips.Count) skip line(s)" }
    $v.examples = @(@($unmapped | Select-Object -First 3) + @($missing | Select-Object -First 3) + @($extra | Select-Object -First 3))
    if ($why.Count) { $v.reason = $why -join '; ' } else { $v.ok = $true }
    $v
}

# The child's exit counts (EP-C11): 0, or 1 with a knownSkips block whose ok is true. No finished check here (the
# free-space figure reads exitCode alone).
function Test-ExitAccepted($m) {
    $code = Get-Prop $m 'exitCode'
    if ($code -is [bool] -or $null -eq $code) { return $false }
    if ($code -eq 0) { return $true }
    ($code -eq 1) -and ((Get-Prop (Get-Prop $m 'knownSkips') 'ok') -eq $true)
}

# An attempt finished (EP-C11): the child ran to its end and its exit counts. Every check that once read
# "finished with exit 0" reads this.
function Test-AttemptFinished($m) {
    ((Get-Prop $m 'finished') -eq $true) -and (Test-ExitAccepted $m)
}

# The runner's exit code: the child's codes other than 0 and 1 first (as before); then 3 (contaminated) and 4 (not
# cold), which take precedence over the child's 1 (EP-C11); then the child's 0 or 1 (meta.json's knownSkips says
# whether a 1 counts as finished).
function Get-RunnerExit([int]$childCode, [bool]$contaminated, [bool]$notCold) {
    if ($childCode -ne 0 -and $childCode -ne 1) { return $childCode }
    if ($contaminated) { return 3 }
    if ($notCold) { return 4 }
    $childCode
}

# Whether a runner exit is a clean attempt (what a warmup must be): 0, or 1 when the child's 1 counts as finished.
function Test-CleanRunnerExit([int]$runnerExit, [bool]$childAccepted) {
    ($runnerExit -eq 0) -or ($runnerExit -eq 1 -and $childAccepted)
}

# samply's "needs elevation" reading of stderr: only for a child exit that does not count. An accepted exit 1's
# log holds every skipped input path, and a path may hold one of the words.
function Test-SamplyNeedsElevation([bool]$samply, [bool]$childAccepted, [int]$code, [string]$errText) {
    $samply -and $code -ne 0 -and -not $childAccepted -and ($errText -match '(?i)elevat|administrator|access (is )?denied|privilege')
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

# EP-C8: --validate is for fresh runs only; a noop or stale run, or any copy, routes files through check_one.
function Get-ValidateProblem([bool]$validate, [string]$cohort, [string]$copyFrom) {
    if (-not $validate) { return $null }
    if ($cohort -in 'warm-noop', 'warm-stale') { return "-Validate is refused for $cohort (EP-C8: fresh runs only)" }
    if ($copyFrom) { return '-Validate is refused with -CopyFrom (EP-C8: fresh runs only)' }
    $null
}

# The cohort's arguments after the shared ones: warm-stale's edge setting, and --validate.
function Get-ExtraArgs([string]$cohort, [string]$staleEdge, [bool]$validate) {
    $extra = @()
    if ($cohort -eq 'warm-stale') { $extra += @('--set', "mips.edge=$staleEdge") }
    if ($validate) { $extra += '--validate' }
    , $extra
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
    $ec = $c.Process.ExitCode
    $plan = ConvertFrom-PlanStream @([IO.File]::ReadAllLines((Join-Path $dir 'stdout.jsonl'))) $A.Corpus
    $why = Get-ExpectedExitProblem $ec $plan.Done
    if ($why) { throw "the dry run exited with ${ec}: $why; see $dir\stderr.log" }
    if ($ec -eq 1) { Say 'the dry run exited with 1 and batch_done shows nothing failed or cancelled: it counts as finished (EP-C11)' }
    $n = Write-ExpectedLists $A.RunRoot $A.Label $plan
    Say ('expected {0} key(s); {1} planned but not written; {2} skipped input(s); {3} skip/failure event(s); dry-run folder {4}' -f $n, @($plan.NotWritten).Count, @($plan.Skipped).Count, @($plan.Events).Count, $dir)
}

# The dry run's four lists in RunRoot, each created new. Returns the expected key count.
function Write-ExpectedLists([string]$runRoot, [string]$label, $plan) {
    $sorted = Sort-Ordinal $plan.Ready
    Write-NewFile (Join-Path $runRoot "expected-$label.txt") (Join-Lines $sorted)
    Write-NewFile (Join-Path $runRoot "planned-not-written-$label.txt") (Join-Lines (Sort-Ordinal $plan.NotWritten))
    Write-NewFile (Join-Path $runRoot "skipped-inputs-$label.txt") (Join-Lines (Sort-Ordinal $plan.Skipped))
    Write-NewFile (Join-Path $runRoot "expected-$label-events.txt") (Join-Lines (Sort-Ordinal $plan.Events))
    $sorted.Count
}

# ---------------------------------------------------------------- one attempt (spec 7.3 to 7.7)

$WarmCohorts = @('warm-fresh', 'warm-noop', 'warm-stale', 'overhead', 'parity')
# Whether the last attempt's child exit counted as finished (EP-C11); read after a warmup Invoke-Attempt returns.
$script:LastChildAccepted = $false
$NoColdStamp = 'no unused cold stamp under 15 minutes old in RunRoot: run tools\profile-evict.ps1 -RunRoot <RunRoot> -Corpus <the same corpus folder>, or the owner runs tools\profile-purge.ps1 elevated'

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
    if ($cohort -eq 'cold-fresh' -and -not (Get-ColdStamp $A.RunRoot (Get-Date) $A.Corpus)) { $problems += $NoColdStamp }
    if ($cohort -in $WarmCohorts) { $pp = Get-PredecessorProblem $A.RunRoot $A.Corpus (Get-Date); if ($pp) { $problems += $pp } }
    $figure = Get-ExpectedBytesFigure $A.RunRoot $A.Corpus $A.Build ([int64]$A.ExpectedBytes)
    if ($null -eq $figure) { $problems += 'no earlier attempt of this corpus and build in RunRoot to size the free-space check from: pass -ExpectedBytes (the corpus is never read outside the command)' }

    $vp = Get-ValidateProblem ([bool]$A.Validate) $cohort $A.CopyFrom
    if ($vp) { $problems += $vp }
    $extra = Get-ExtraArgs $cohort $A.StaleEdge ([bool]$A.Validate)
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
    $script:LastChildAccepted = $false

    # quiet check; a warm chain whose check needed a retry gets a warmup first (spec 7.3)
    $qs = Get-Date
    $quiet = Wait-Quiet $A.RunRoot $needed
    if (-not $quiet.PassedFirstTry -and ($cohort -in $WarmCohorts)) {
        Say 'the quiet check needed a retry: running a warmup first'
        $w = $A.Clone(); $w.Cohort = 'warmup'; $w.CopyFrom = $null; $w.Calibrate = $false; $w.Samply = $null
        $wr = Invoke-Attempt $w $false
        if (-not (Test-CleanRunnerExit $wr $script:LastChildAccepted)) { throw "the warmup ended with code $wr; stopping" }
        $qs = Get-Date
        $quiet = Wait-Quiet $A.RunRoot $needed
        if (-not $quiet.PassedFirstTry) { throw 'stopping: the quiet check after the warmup did not pass at its first try' }
    }
    if ($cohort -in $WarmCohorts -and -not $copies) {
        $pp = Get-PredecessorProblem $A.RunRoot $A.Corpus $qs
        if ($pp) { throw "refused: $pp" }
    }
    $purge = $null
    $coldMethod = $null
    if ($cohort -eq 'cold-fresh') {
        $purge = Get-ColdStamp $A.RunRoot (Get-Date) $A.Corpus
        if (-not $purge) { throw "refused: $NoColdStamp" }
        $coldMethod = if ($purge -like 'evict-*') { 'per-file-no-buffering' } else { 'standby-purge' }
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
        exitCode = $null; externalWallMs = $null; purgeStamp = $purge; coldMethod = $coldMethod; calibrate = [bool]$A.Calibrate; validate = [bool]$A.Validate; samply = $null
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
        # EP-C11: a child exit 1 counts as finished only with the known skips (the verdict is meta.json's knownSkips)
        if ($code -eq 1) {
            $logLines = [IO.File]::ReadAllLines((Join-Path $attempt 'stderr.log'), [Text.Encoding]::UTF8)
            $meta['knownSkips'] = Get-KnownSkipsVerdict $A.RunRoot $A.Label $logLines $A.Corpus
        }
        $childAccepted = Test-ExitAccepted $meta
        $meta['disk'] = [ordered]@{
            start = [ordered]@{ corpus = (Get-DiskFigure $diskStart $A.Corpus); output = (Get-DiskFigure $diskStart $out) }
            end = [ordered]@{ corpus = (Get-DiskFigure $diskEnd $A.Corpus); output = (Get-DiskFigure $diskEnd $out) }
        }
        $coldVerdict = $null
        if ($purge) {
            $coldVerdict = Get-ColdVerdict $A.RunRoot $purge $meta['disk'].start.corpus $meta['disk'].end.corpus
            $meta['cold'] = $coldVerdict
        }
        $timed = @($rows | Where-Object { $_.t_ms -ge $startMs -and $_.t_ms -le $endMs + 1200 })
        $verdict = Get-ContentionVerdict $timed $baseline ([bool]$A.Calibrate)
        $meta['contention'] = $verdict
        if ($A.Calibrate) {
            if ($verdict.samples -eq 0 -or $verdict.reasons.Count) { throw ('calibration unusable, baseline.json not written: ' + ($verdict.reasons -join '; ')) }
            Write-NewJson $baselinePath ([ordered]@{ otherCpuCores = $verdict.otherCpuCoresMean; otherIoBytesPerSec = $verdict.otherIoBytesPerSecMean; attempt = $name; samples = $verdict.samples; logicalCores = [Environment]::ProcessorCount })
        }
        $errText = [IO.File]::ReadAllText((Join-Path $attempt 'stderr.log'))
        if (Test-SamplyNeedsElevation ([bool]$A.Samply) $childAccepted $code $errText) {
            $meta['samply'] = 'needs elevation'
            Say 'samply needs elevation: the owner runs this command from an elevated shell; stopping'
            return 5
        }
        if ($A.Samply) { $meta['samply'] = 'recorded' }

        # outputs: hashes in output_key form, and for noop the file lists after
        $meta['ddsFiles'] = Write-HashesCsv $attempt $out
        if ($cohort -eq 'warm-noop') { Write-NewFile (Join-Path $attempt 'files-after.csv') ("path,size,mtime_utc_ticks`n" + (Join-Lines (Get-FileList $out))) }

        Write-NewJson (Join-Path $A.RunRoot "last-run-$name.json") ([ordered]@{
                attempt = $name; corpus = $A.Corpus; cohort = $cohort; finished = $true; killed = $false; exitCode = $code
                endedUnixMs = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
            })
        Say ('exit {0}, wall {1} ms, contention: {2}' -f $code, $wall.ElapsedMilliseconds, $(if ($verdict.contaminated) { 'CONTAMINATED ' + ($verdict.reasons -join '; ') } else { 'clean' }))
        $notCold = [bool]($coldVerdict -and $coldVerdict.Contains('cold') -and -not $coldVerdict.cold)
        if ($notCold) { Say "NOT COLD: $($coldVerdict.reason)" }
        if ($code -eq 1) {
            $ks = $meta['knownSkips']
            if ($childAccepted) { Say ('exit 1 counts as finished: only the known skips ({0} of {1}, EP-C11)' -f $ks.skipped, $ks.list) }
            else { Say "exit 1 does NOT count as finished: $($ks.reason)" }
        }
        Say $DetectionLimit
        $script:LastChildAccepted = $childAccepted
        return (Get-RunnerExit $code ([bool]$verdict.contaminated) $notCold)
    } catch {
        $meta['error'] = $_.Exception.Message
        Say "attempt stopped: $($_.Exception.Message)"
        return 1
    } finally {
        Write-NewJson (Join-Path $attempt 'meta.json') $meta
    }
}

# ---------------------------------------------------------------- prune (EP-O5)

# Reason a name is not an attempt folder directly in RunRoot, or $null (the -CopyFrom name rule).
function Get-AttemptDirProblem([string]$runRoot, [string]$name) {
    if ([string]::IsNullOrWhiteSpace($name) -or $name -ne [IO.Path]::GetFileName($name) -or $name -match '[\\/:]' -or $name -eq '..' -or $name -eq '.') {
        return 'must be an attempt folder name directly inside RunRoot'
    }
    $dir = Join-Path $runRoot $name
    if (-not (Test-Path -LiteralPath $dir -PathType Container)) { return "no attempt folder $name in RunRoot" }
    if ((Get-Item -LiteralPath $dir -Force).LinkType) { return "$name is a junction or link" }
    $null
}

# The hashes.csv rows (header dropped), or $null when the file is missing.
function Get-HashRows([string]$dir) {
    $p = Join-Path $dir 'hashes.csv'
    if (-not (Test-Path -LiteralPath $p -PathType Leaf)) { return $null }
    , @([IO.File]::ReadAllLines($p) | Select-Object -Skip 1 | Where-Object { $_ })
}

# A corpus's reference (EP-R4: the first base attempt): the oldest attempt in RunRoot, by its time-stamped name,
# whose meta.json shows a base build of that corpus that finished (Test-AttemptFinished). $null when none. An
# unreadable meta.json is skipped with a warning.
function Get-CorpusReference([string]$runRoot, [string]$corpus) {
    $want = $corpus.TrimEnd('\')
    $dirs = @(Get-ChildItem -LiteralPath $runRoot -Directory -ErrorAction SilentlyContinue | ForEach-Object Name)
    foreach ($name in (Sort-Ordinal $dirs)) {
        $mp = Join-Path (Join-Path $runRoot $name) 'meta.json'
        if (-not (Test-Path -LiteralPath $mp -PathType Leaf)) { continue }
        try { $m = Read-Json $mp }
        catch { Write-Warning "skipped the unreadable $name\meta.json: $($_.Exception.Message)"; continue }
        if ((Get-Prop $m 'build') -eq 'base' -and (Test-AttemptFinished $m) -and
            [string]::Equals(([string](Get-Prop $m 'corpus')).TrimEnd('\'), $want, 'OrdinalIgnoreCase')) { return $name }
    }
    $null
}

# EP-O5 (amends EP-R3): the ONLY deletion in this script. Deletes exactly the .dds files the attempt's hashes.csv
# lists, under <attempt>\out, once that list equals the reference attempt's. A corpus reference is protected by
# what it is, not only by the arguments: the corpus reference (Get-CorpusReference) is never pruned, and
# -Reference must be a finished base-build attempt of the same corpus. Other base attempts (the overhead
# cohort's) may go. meta.json is copied to meta.before-prune.json (created new: also the "already pruned" lock)
# and replaced by a copy with a prune block before anything is deleted. Throws "refused: ..." and deletes
# nothing on any problem.
function Invoke-PruneOutputs([string]$runRoot, [string]$attempt, [string]$reference, [string[]]$keep) {
    foreach ($n in @($attempt, $reference)) {
        $why = Get-AttemptDirProblem $runRoot $n
        if ($why) { throw "refused: $n $why" }
    }
    if ($attempt -ieq $reference) { throw "refused: $attempt is the reference attempt (kept whole)" }
    if (@($keep | Where-Object { $_ -ieq $attempt }).Count) { throw "refused: $attempt is in the -Keep list" }
    $dir = Join-Path $runRoot $attempt
    $metaPath = Join-Path $dir 'meta.json'
    $backup = Join-Path $dir 'meta.before-prune.json'
    if (-not (Test-Path -LiteralPath $metaPath -PathType Leaf)) { throw "refused: $attempt has no meta.json" }
    if (Test-Path -LiteralPath $backup) { throw "refused: $attempt was already pruned (meta.before-prune.json exists)" }
    $meta = Read-Json $metaPath
    $ownCorpus = [string](Get-Prop $meta 'corpus')
    if ($ownCorpus -and $attempt -ieq (Get-CorpusReference $runRoot $ownCorpus)) {
        throw "refused: $attempt is the corpus reference (the oldest finished base-build attempt of its corpus, EP-R4) and is kept whole (EP-O5)"
    }
    if (Get-Prop $meta 'prune') { throw "refused: $attempt was already pruned" }
    if (-not (Test-AttemptFinished $meta)) { throw "refused: $attempt did not finish (exit 0, or exit 1 with only the known skips, EP-C11)" }
    $refMetaPath = Join-Path (Join-Path $runRoot $reference) 'meta.json'
    if (-not (Test-Path -LiteralPath $refMetaPath -PathType Leaf)) { throw "refused: the reference $reference has no meta.json" }
    $refMeta = Read-Json $refMetaPath
    if ((Get-Prop $refMeta 'build') -ne 'base') { throw "refused: the reference $reference is not a base-build attempt (build $(Get-Prop $refMeta 'build'))" }
    if (-not (Test-AttemptFinished $refMeta)) { throw "refused: the reference $reference did not finish (exit 0, or exit 1 with only the known skips, EP-C11)" }
    $corpusOf = { param($m) ([string](Get-Prop $m 'corpus')).TrimEnd('\') }
    if (-not (& $corpusOf $meta) -or -not [string]::Equals((& $corpusOf $meta), (& $corpusOf $refMeta), 'OrdinalIgnoreCase')) {
        throw "refused: the reference $reference is of another corpus ($(& $corpusOf $refMeta), not $(& $corpusOf $meta))"
    }
    $rows = Get-HashRows $dir
    if ($null -eq $rows) { throw "refused: $attempt has no hashes.csv" }
    $refRows = Get-HashRows (Join-Path $runRoot $reference)
    if ($null -eq $refRows -or $refRows.Count -eq 0) { throw "refused: the reference $reference has no hashes.csv rows" }
    $mine = @{}; foreach ($r in $rows) { $mine[$r] = $true }
    $theirs = @{}; foreach ($r in $refRows) { $theirs[$r] = $true }
    $onlyMine = @($rows | Where-Object { -not $theirs.ContainsKey($_) }).Count
    $onlyRef = @($refRows | Where-Object { -not $mine.ContainsKey($_) }).Count
    if ($onlyMine -or $onlyRef -or $rows.Count -ne $refRows.Count) {
        throw "refused: $attempt's hashes.csv differs from the reference $reference ($onlyMine row(s) only here, $onlyRef only in the reference)"
    }
    $out = Join-Path $dir 'out'
    if (-not (Test-Path -LiteralPath $out -PathType Container)) { throw "refused: $attempt has no out folder" }
    $outItem = Get-Item -LiteralPath $out -Force
    if ($outItem.LinkType -or ($outItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw "refused: $attempt\out is a junction or link" }
    $links = @(Get-ChildItem -LiteralPath $out -Recurse -Force -Attributes ReparsePoint -ErrorAction Stop)
    if ($links.Count) { throw "refused: $attempt\out holds a junction or link ($($links[0].FullName))" }
    $onDisk = @{}
    foreach ($e in Get-DdsEntries $out) { $onDisk[$e.Key] = $e.Path }
    $keys = @($rows | ForEach-Object { Get-HashRowKey $_ })
    $missing = @($keys | Where-Object { -not $onDisk.ContainsKey($_) })
    if ($missing.Count) { throw "refused: $($missing.Count) hashed file(s) are not on disk, e.g. $($missing[0])" }
    if ($onDisk.Count -ne $keys.Count) { throw "refused: $($onDisk.Count - $keys.Count) .dds file(s) under out are not in hashes.csv" }
    $files = @($keys | ForEach-Object { $onDisk[$_] })
    $bytes = [int64]0
    foreach ($f in $files) { $bytes += ([IO.FileInfo]$f).Length }

    [IO.File]::Copy($metaPath, $backup, $false)
    $record = [ordered]@{ at = (Get-Date).ToString('o'); reference = $reference; files = $files.Count; bytes = $bytes; keep = @($keep) }
    $meta | Add-Member -NotePropertyName 'prune' -NotePropertyValue $record
    $tmp = Join-Path $dir 'meta.prune-new.json'
    Write-NewJson $tmp $meta
    [IO.File]::Move($tmp, $metaPath, $true)
    foreach ($f in $files) { [IO.File]::Delete($f) }
    $left = @([IO.Directory]::EnumerateFiles($out, '*.dds', [IO.SearchOption]::AllDirectories)).Count
    if ($left) { throw "$left .dds file(s) remain under $out after the prune" }
    [pscustomobject]@{ Files = $files.Count; Bytes = $bytes }
}

# ---------------------------------------------------------------- entry

if ($SelfTest) { Invoke-SelfTest }

if ($PSCmdlet.ParameterSetName -eq 'Prune') {
    $RunRoot = [IO.Path]::GetFullPath($RunRoot).TrimEnd('\')
    $why = Get-RunRootProblem $RunRoot (Get-ProtectedList)
    if ($why) { throw "refused: RunRoot $RunRoot $why" }
    if (-not (Test-Path -LiteralPath $RunRoot -PathType Container)) { throw "refused: RunRoot not found: $RunRoot" }
    $r = Invoke-PruneOutputs $RunRoot $Attempt $Reference $Keep
    Say ('pruned {0}: {1} .dds file(s), {2:N0} bytes; meta.json records it' -f $Attempt, $r.Files, $r.Bytes)
    exit 0
}

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
$code = Invoke-Attempt @{ Exe = $Exe; Corpus = $Corpus; Label = $Label; Cohort = $Cohort; Build = $Build; RunRoot = $RunRoot; Set = $Set; CopyFrom = $CopyFrom; StaleEdge = $StaleEdge; Samply = $Samply; Calibrate = [bool]$Calibrate; Validate = [bool]$Validate; ExpectedBytes = $ExpectedBytes } $dry
exit $code

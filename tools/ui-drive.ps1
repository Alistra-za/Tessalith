#Requires -Version 7
<#
Wrapper around the headless UI driver (crates/gui/tests/ui_drive, Note HM). Usage, one mode per call:

  -Build                     cargo test --offline -p sdc-gui --test ui_drive --no-run, then copies the test executable to
                             <repo>\.claude\scratch\ui_drive.exe. Needs CARGO_TARGET_DIR in the environment (set CARGO_BUILD_JOBS=4
                             and CARGO_INCREMENTAL=0 too). The copy matters: another worktree's build on a shared target can replace
                             the original under a running run.
  -List                      the copy with --list: fixture names, then `check <name>` lines.
  -Fixture f -Steps file -Out dir [-Size WxH] [-Theme dark|light] [-Goal 'json' | -GoalFile file]
                             runs the copy (no cargo, so several explorers can run at once). Output goes to <Out>\driver.log and
                             the console; the driver's exit code is passed through (0 ok, 1 a step failed, 2 bad arguments).
  -Replay runDir [-Upto n] -Out dir
                             fixture, size and theme from runDir\run.json, the first n steps of runDir\steps.jsonl (all if no -Upto).
  -Collect dir               every *.checks.json under dir, deduplicated on (check, subject role, subject label, run size), into
                             dir\mechanical.json and dir\mechanical.md (Check | Rule | Where | Size | Seen | First frame).
  -SelfTest                  -Collect on two generated runs holding the same finding plus one other: expects 2 rows.
#>
[CmdletBinding()]
param(
    [switch]$Build,
    [switch]$List,
    [string]$Fixture,
    [string]$Steps,
    [string]$Out,
    [string]$Size,
    [string]$Theme,
    [string]$Goal,
    [string]$GoalFile,
    [string]$Replay,
    [int]$Upto = -1,
    [string]$Collect,
    [switch]$SelfTest
)
$ErrorActionPreference = 'Stop'
# embedded double quotes in -Goal must reach the exe intact
$PSNativeCommandArgumentPassing = 'Standard'
$repo = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $repo '.claude\scratch\ui_drive.exe'

function Invoke-Build {
    if (-not $env:CARGO_TARGET_DIR) { throw 'set CARGO_TARGET_DIR (and CARGO_BUILD_JOBS=4, CARGO_INCREMENTAL=0) before -Build' }
    Push-Location $repo
    try {
        $lines = & cargo test --offline -p sdc-gui --test ui_drive --no-run --message-format=json
        if ($LASTEXITCODE -ne 0) { throw "cargo failed with exit $LASTEXITCODE" }
    } finally { Pop-Location }
    $built = $null
    foreach ($l in $lines) {
        if ($l -notlike '{*') { continue }
        $m = $l | ConvertFrom-Json
        if ($m.reason -eq 'compiler-artifact' -and $m.target.name -eq 'ui_drive' -and $m.executable) { $built = $m.executable }
    }
    if (-not $built) { throw 'cargo reported no ui_drive executable' }
    New-Item -ItemType Directory -Force (Split-Path $exe) | Out-Null
    Copy-Item -LiteralPath $built -Destination $exe -Force
    Write-Host "built $exe"
}

# Runs the copy; console and <out>\driver.log get the same text. Returns the driver's exit code.
function Invoke-Driver([string[]]$DriverArgs, [string]$LogDir) {
    if (-not (Test-Path -LiteralPath $exe)) { throw "no $exe; run -Build first" }
    if ($LogDir) {
        New-Item -ItemType Directory -Force $LogDir | Out-Null
        & $exe @DriverArgs 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath (Join-Path $LogDir 'driver.log') | Out-Host
    } else {
        & $exe @DriverArgs 2>&1 | ForEach-Object { "$_" } | Out-Host
    }
    return $LASTEXITCODE
}

function Invoke-Run([string]$Fx, [string]$StepsFile, [string]$OutDir, [string]$Sz, [string]$Th, [string]$GoalLine) {
    if (-not $Fx -or -not $StepsFile -or -not $OutDir) { throw '-Fixture, -Steps and -Out are required' }
    $a = @('--fixture', $Fx, '--script', $StepsFile, '--out', $OutDir)
    if ($Sz) { $a += @('--size', $Sz) }
    if ($Th) { $a += @('--theme', $Th) }
    if ($GoalLine) { $a += @('--goal', $GoalLine) }
    return Invoke-Driver $a $OutDir
}

function Invoke-Collect([string]$Dir) {
    $Dir = (Resolve-Path -LiteralPath $Dir).Path
    $rows = [ordered]@{}
    $files = Get-ChildItem -LiteralPath $Dir -Recurse -Filter '*.checks.json' -File | Sort-Object FullName
    foreach ($f in $files) {
        $runJson = Join-Path $f.DirectoryName 'run.json'
        $size = '?'
        if (Test-Path -LiteralPath $runJson) {
            $r = Get-Content -Raw -LiteralPath $runJson | ConvertFrom-Json
            $size = '{0}x{1}' -f [int]$r.size[0], [int]$r.size[1]
        }
        $frame = $f.Name -replace '\.checks\.json$', ''
        $first = '{0} {1}' -f [IO.Path]::GetRelativePath($Dir, $f.DirectoryName), $frame
        foreach ($x in @(Get-Content -Raw -LiteralPath $f.FullName | ConvertFrom-Json)) {
            $key = '{0}|{1}|{2}|{3}' -f $x.check, $x.subject.role, $x.subject.label, $size
            if (-not $rows.Contains($key)) {
                $rows[$key] = [pscustomobject]@{
                    check = $x.check; rule = $x.rule
                    where = ('{0} "{1}"' -f $x.subject.role, $x.subject.label).Trim()
                    size = $size; seen = 0; first_frame = $first
                }
            }
            $rows[$key].seen++
        }
    }
    $all = @($rows.Values)
    ConvertTo-Json -InputObject $all -Depth 4 | Set-Content -LiteralPath (Join-Path $Dir 'mechanical.json') -Encoding utf8
    $cell = { param($s) ([string]$s) -replace '\|', '\|' -replace '[\r\n]+', ' ' }
    $md = @('| Check | Rule | Where | Size | Seen | First frame |', '|---|---|---|---|---|---|')
    foreach ($r in $all) {
        $md += '| {0} | {1} | {2} | {3} | {4} | {5} |' -f (& $cell $r.check), (& $cell $r.rule), (& $cell $r.where), $r.size, $r.seen, (& $cell $r.first_frame)
    }
    $md | Set-Content -LiteralPath (Join-Path $Dir 'mechanical.md') -Encoding utf8
    Write-Host "collected $($all.Count) rows from $($files.Count) frames into $Dir\mechanical.md"
    return $all.Count
}

function Invoke-SelfTest {
    $tmp = Join-Path $repo ('.claude\scratch\ui-drive-selftest-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
    try {
        $names = '{"check":"names","rule":"every control has a name","subject":{"role":"Button","label":"","rect":[0,0,10,10]},"other":null,"detail":"d"}'
        $overlap = '{"check":"overlap","rule":"no overlap","subject":{"role":"Label","label":"A","rect":[0,0,10,10]},"other":{"role":"Label","label":"B","rect":[5,5,10,10]},"detail":"d"}'
        foreach ($case in @(@('run1', @($names)), @('run2', @($names, $overlap)))) {
            $d = Join-Path $tmp $case[0]
            New-Item -ItemType Directory -Force $d | Out-Null
            '{"size":[1400.0,790.0]}' | Set-Content -LiteralPath (Join-Path $d 'run.json')
            '[' + ($case[1] -join ',') + ']' | Set-Content -LiteralPath (Join-Path $d '00.checks.json')
        }
        $n = Invoke-Collect $tmp
        $seen = (Get-Content -Raw (Join-Path $tmp 'mechanical.json') | ConvertFrom-Json | Where-Object check -eq 'names').seen
        if ($n -eq 2 -and $seen -eq 2) { Write-Host 'PASS: 2 rows'; return 0 }
        Write-Host "FAIL: $n rows, names seen $seen"; return 1
    } finally { Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue }
}

if ($SelfTest) { exit (Invoke-SelfTest) }
if ($Build) { Invoke-Build; exit 0 }
if ($List) { exit (Invoke-Driver @('--list') $null) }
if ($Collect) { [void](Invoke-Collect $Collect); exit 0 }
if ($Replay) {
    if (-not $Out) { throw '-Out is required with -Replay' }
    $r = Get-Content -Raw -LiteralPath (Join-Path $Replay 'run.json') | ConvertFrom-Json
    $lines = @(Get-Content -LiteralPath (Join-Path $Replay 'steps.jsonl') | Where-Object { $_.Trim() })
    if ($Upto -ge 0) { $lines = @($lines | Select-Object -First $Upto) }
    New-Item -ItemType Directory -Force $Out | Out-Null
    $src = Join-Path $Out 'replay-steps.jsonl'
    $lines | Set-Content -LiteralPath $src
    exit (Invoke-Run $r.fixture $src $Out ('{0}x{1}' -f [int]$r.size[0], [int]$r.size[1]) $r.theme '')
}
if ($Fixture) {
    $g = $Goal
    if ($GoalFile) { $g = (Get-Content -Raw -LiteralPath $GoalFile).Trim() }
    exit (Invoke-Run $Fixture $Steps $Out $Size $Theme $g)
}
throw 'nothing to do; see the header of tools\ui-drive.ps1'

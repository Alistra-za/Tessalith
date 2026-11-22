#Requires -Version 7
<#
.SYNOPSIS
  Runs the full gate on the current checkout and prints PASS or FAIL per step.
.DESCRIPTION
  Each step's output goes to <Out>\<step>.txt, ending in "exit N". Exits 1 if a step fails, 2 if it didn't start.
  <Out>\progress.txt gets one line per event (start, PASS or FAIL, skipped, final gate result), so a long run shows
  which step is running. The step logs fill as a step runs (measured: a native child's stdout and stderr lines were
  readable with Get-Content while the step was still going; a file-size check can show a stale 0 bytes on NTFS).
  Set CARGO_TARGET_DIR first (CLAUDE.md, Disk). A failed fmt, clippy or render3d-alone step skips the tests, which take about an hour.
  Test builds encode on AVX2 at most; the avx512 step re-runs the ISPC pins (level_identity, bc6h, target) with
  SDC_TEST_AVX512=1, so on an AVX-512 CPU they check the AVX-512 path too. The script clears SDC_TEST_AVX512 in the
  process that runs it, so run it in a child shell if the caller's value matters.
.EXAMPLE
  tools\gate.ps1 -Out .claude\scratch\gate
#>
param([Parameter(Mandatory)][string]$Out)

$top = git rev-parse --show-toplevel
if ($LASTEXITCODE) { 'Stopped: not inside a git checkout.'; exit 2 }
$target = [IO.Path]::GetFullPath($(if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { $top }))
$freeGb = [int]([IO.DriveInfo]::new($target.Substring(0, 1)).AvailableFreeSpace / 1GB)
if ($freeGb -lt 20) { "Stopped: $freeGb GB free on the build drive (CLAUDE.md, Disk)."; exit 2 }
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = 4 }

New-Item -ItemType Directory -Force $Out | Out-Null
Remove-Item (Join-Path $Out '*.txt')

$progress = Join-Path $Out 'progress.txt'
function Note([string]$Text) { "$(Get-Date -Format HH:mm:ss) $Text" >> $progress }
$failed = $false
function Step([string]$Name, [scriptblock]$Run) {
    $log = Join-Path $Out "$Name.txt"
    Note "start $Name"
    & $Run *> $log
    $code = $LASTEXITCODE
    "exit $code" >> $log
    if ($code) { $script:failed = $true; Note "$Name FAIL (exit $code)"; "$Name FAIL, see $log" } else { Note "$Name PASS"; "$Name PASS" }
}

# Every step but avx512 runs capped at AVX2, whatever the caller's shell holds.
Remove-Item Env:SDC_TEST_AVX512 -ErrorAction Ignore

Step fmt { cargo fmt --all --check }
Step clippy-all { cargo clippy --offline --workspace --all-targets -- -D warnings }
Step clippy { cargo clippy --offline --workspace -- -D warnings }
Step render3d-alone { cargo check --offline -p render3d }
if ($failed) {
    'Tests skipped: fix fmt, clippy and the render3d check first.'
    foreach ($s in 'test', 'avx512', 'render') { Note "$s skipped (an earlier step failed)" }
} else {
    Step test { cargo test --offline --workspace --no-fail-fast -- --skip render_ --skip render:: --test-threads=4 }
    Step avx512 {
        $env:SDC_TEST_AVX512 = '1'
        try { cargo test --offline -p encoders-sys -p sdc --test level_identity --test bc6h --test target }
        finally { Remove-Item Env:SDC_TEST_AVX512 }
    }
    Step render { cargo test --offline --workspace --no-fail-fast -- render_ render:: --test-threads=1 }
}
Step doc-dupes { py -3.13 -I "$top/tools/doc-dupes.py" --root $top }
Step doc-lint { py -3.13 -I "$top/tools/doc-lint.py" --root $top }
Step doc-markers { pwsh -NoProfile -File "$top/tools/doc-markers.ps1" -Root $top }
Note "gate $(if ($failed) { 'FAIL' } else { 'PASS' })"
exit $(if ($failed) { 1 } else { 0 })

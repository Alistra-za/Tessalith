#Requires -Version 7
<#
.SYNOPSIS
  Runs the full gate on the current checkout and prints PASS or FAIL per step.
.DESCRIPTION
  Each step's output goes to <Out>\<step>.txt, ending in "exit N". Exits 1 if a step fails, 2 if it didn't start.
  Set CARGO_TARGET_DIR first (CLAUDE.md, Disk). A failed fmt or clippy step skips the tests, which take about an hour.
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

$failed = $false
function Step([string]$Name, [scriptblock]$Run) {
    $log = Join-Path $Out "$Name.txt"
    & $Run *> $log
    $code = $LASTEXITCODE
    "exit $code" >> $log
    if ($code) { $script:failed = $true; "$Name FAIL, see $log" } else { "$Name PASS" }
}

Step fmt { cargo fmt --all --check }
Step clippy-all { cargo clippy --offline --workspace --all-targets -- -D warnings }
Step clippy { cargo clippy --offline --workspace -- -D warnings }
Step render3d-alone { cargo check --offline -p render3d }
if ($failed) {
    'Tests skipped: fix fmt and clippy first.'
} else {
    Step test { cargo test --offline --workspace --no-fail-fast -- --skip render_ --test-threads=4 }
    Step render { cargo test --offline --workspace --no-fail-fast -- render_ --test-threads=1 }
}
Step doc-dupes { py -3.13 -I "$top/tools/doc-dupes.py" --root $top }
Step doc-markers { pwsh -NoProfile -File "$top/tools/doc-markers.ps1" -Root $top }
exit $(if ($failed) { 1 } else { 0 })

# Rebuilds the ISPC objects into a temp dir and compares SHA-256 with the vendored objects. One compiler:
# ISPC 1.32.0 (-Ispc) builds every vendored file (owner decision, release 3.2). 1.32.0 also fixes ISPC issue 3891,
# an AVX-512 miscompile in 1.31.0, so one compiler serves all three targets. -Update copies the new objects and
# main headers into vendor/ispc-objs (then run tools/hash-vendor.ps1). -Ispc512 is gone: it stays as a parameter
# only so that an old command line fails with a message instead of binding to nothing.
#
# Targets: sse2, avx2 and avx512skx-x16 (`--target=sse2,avx2,avx512skx-x16`): each source gives one dispatcher
# object, one object per target and the main header. A vendored `.obj` the build does not produce (a dropped
# target's) is reported as EXTRA and counted as a difference. -Update never deletes a vendored file: remove an
# EXTRA object with `git rm`.
#
# kernel.ispc is built with -DSDC_BC6H_ONLY (ISPC TC patch 0003): its objects hold the BC6H
# encoder only. bc7e.ispc does not get the define.
#
# Reproducibility: ISPC embeds the source path it was given, so it is always run with the working
# directory at the repo root and a repo-relative source path, with the target pinned explicitly
# (--arch=x86-64 --target-os=windows). The generated header's line 2 (the output path, which is a
# temp dir) is normalised to the bare file name before hashing and vendoring.
param(
  [string]$Ispc = "ispc.exe",
  # removed: passing it is an error
  [string]$Ispc512 = "",
  [string]$Bc7eSource = "$PSScriptRoot\..\crates\encoders-sys\vendor\bc7e\bc7e.ispc",
  [switch]$Update,
  # with -Update: replace only the bc7e objects (leave the ISPC TC objects untouched)
  [switch]$Bc7eOnly
)
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path "$PSScriptRoot\..").Path
$vendor = Join-Path $repo "crates\encoders-sys\vendor"
if ($PSBoundParameters.ContainsKey('Ispc512')) { throw "ISPC 1.32.0 builds every object now: pass it as -Ispc and drop -Ispc512" }
if (-not (Get-Command $Ispc -ErrorAction SilentlyContinue)) { throw "ISPC 1.32.0 not found as '$Ispc'; pass -Ispc <path to ispc.exe>" }
$ver = & $Ispc --version | Out-String
if ($ver -notmatch "1\.32\.0") { throw "ISPC 1.32.0 required as -Ispc; got: $ver" }

# Repo-relative, forward-slash source path (absolute only if the source lives outside the repo).
function Get-SourceArg([string]$path) {
  $full = (Resolve-Path $path).Path
  if ($full.StartsWith($repo + "\", [StringComparison]::OrdinalIgnoreCase)) { $full = $full.Substring($repo.Length + 1) }
  return $full.Replace([string][char]92, "/")
}

# Rewrites header line 2 ("// <output path>") to "// <file name>" so the header does not depend on the temp dir.
function Normalize-Header([string]$path) {
  $lines = [IO.File]::ReadAllLines($path)
  if ($lines.Length -gt 1 -and $lines[1] -match '^// .*[/\\]') { $lines[1] = "// " + [IO.Path]::GetFileName($path) }
  [IO.File]::WriteAllLines($path, $lines, (New-Object Text.UTF8Encoding($false)))
}

# Builds both sources into $out\bc7e and $out\ispctc.
function Build-Objects([string]$exe, [string]$out) {
  New-Item -ItemType Directory "$out\bc7e", "$out\ispctc" | Out-Null
  Push-Location $repo
  try {
    $common = @("-O2", "--arch=x86-64", "--target-os=windows", "--target=sse2,avx2,avx512skx-x16", "--opt=fast-math")
    # ISPC prints hundreds of compiler warnings for these two vendored sources; show them only on failure.
    $log = & $exe @common "--opt=disable-assertions" (Get-SourceArg $Bc7eSource) -o "$out\bc7e\bc7e.obj" -h "$out\bc7e\bc7e_ispc.h" 2>&1
    if ($LASTEXITCODE) { $log; throw "ispc failed on $Bc7eSource" }
    $log = & $exe @common "-DSDC_BC6H_ONLY" (Get-SourceArg "$vendor\ispctc\kernel.ispc") -o "$out\ispctc\kernel.obj" -h "$out\ispctc\kernel_ispc.h" 2>&1
    if ($LASTEXITCODE) { $log; throw "ispc failed on kernel.ispc" }
  }
  finally { Pop-Location }
  Normalize-Header "$out\bc7e\bc7e_ispc.h"
  Normalize-Header "$out\ispctc\kernel_ispc.h"
}

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("sdc-ispc-" + [guid]::NewGuid())
Build-Objects $Ispc $tmp

$diff = 0
foreach ($dir in "bc7e", "ispctc") {
  # the per-target headers ISPC also emits are not vendored; compare objects and the main header
  $outputs = Get-ChildItem "$tmp\$dir" | Where-Object { $_.Extension -eq ".obj" -or $_.Name -in "bc7e_ispc.h", "kernel_ispc.h" }
  foreach ($o in $outputs) {
    $new = "$tmp\$dir\$($o.Name)"
    $old = "$vendor\ispc-objs\$dir\$($o.Name)"
    $same = (Test-Path $old) -and ((Get-FileHash $old).Hash -eq (Get-FileHash $new).Hash)
    if (-not $same) { $diff++ }
    "{0,-5} {1}/{2}" -f ($(if ($same) { "MATCH" } else { "DIFF" })), $dir, $o.Name
  }
  # a vendored object the build no longer produces (a dropped target) is a difference too, so a stale one cannot hide
  $built = $outputs | ForEach-Object { $_.Name }
  foreach ($v in Get-ChildItem "$vendor\ispc-objs\$dir" -Filter "*.obj") {
    if ($v.Name -notin $built) { $diff++; "{0,-5} {1}/{2}" -f "EXTRA", $dir, $v.Name }
  }
}
if ($Update) {
  foreach ($dir in $(if ($Bc7eOnly) { @("bc7e") } else { @("bc7e", "ispctc") })) {
    $outputs = Get-ChildItem "$tmp\$dir" | Where-Object { $_.Extension -eq ".obj" -or $_.Name -in "bc7e_ispc.h", "kernel_ispc.h" }
    foreach ($o in $outputs) { Copy-Item "$tmp\$dir\$($o.Name)" "$vendor\ispc-objs\$dir\" -Force }
  }
  "Updated vendor/ispc-objs. Now run tools/hash-vendor.ps1."
}
Remove-Item -Recurse -Force $tmp
exit $(if ($Update) { 0 } else { $diff })

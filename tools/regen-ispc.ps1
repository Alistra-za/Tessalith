# Rebuilds the ISPC objects into a temp dir and compares SHA-256 with the vendored objects. Two compilers:
# ISPC 1.31.0 (-Ispc) builds every vendored file except the two `_avx512skx` objects, which come from
# ISPC 1.32.0 (-Ispc512; it fixes an AVX-512 miscompile of 1.31.0, ISPC issue 3891). Without -Ispc512 those
# two objects are reported as SKIP and not counted. -Update (needs both compilers) copies the new objects and
# main headers into vendor/ispc-objs (then run tools/hash-vendor.ps1).
#
# Reproducibility: ISPC embeds the source path it was given, so it is always run with the working
# directory at the repo root and a repo-relative source path, with the target pinned explicitly
# (--arch=x86-64 --target-os=windows). The generated header's line 2 (the output path, which is a
# temp dir) is normalised to the bare file name before hashing and vendoring.
param(
  [string]$Ispc = "ispc.exe",
  # ISPC 1.32.0, for the two `_avx512skx` objects only
  [string]$Ispc512 = "",
  [string]$Bc7eSource = "$PSScriptRoot\..\crates\encoders-sys\vendor\bc7e\bc7e.ispc",
  [switch]$Update,
  # with -Update: replace only the bc7e objects (leave the ISPC TC objects untouched)
  [switch]$Bc7eOnly
)
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path "$PSScriptRoot\..").Path
$vendor = Join-Path $repo "crates\encoders-sys\vendor"
if (-not (Get-Command $Ispc -ErrorAction SilentlyContinue)) { throw "ISPC 1.31.0 not found as '$Ispc'; pass -Ispc <path to ispc.exe> (and -Ispc512 <path to ISPC 1.32.0>)" }
$ver = & $Ispc --version | Out-String
if ($ver -notmatch "1\.31\.0") { throw "ISPC 1.31.0 required as -Ispc (ISPC 1.32.0 goes in -Ispc512); got: $ver" }
if ($Ispc512) {
  if (-not (Get-Command $Ispc512 -ErrorAction SilentlyContinue)) { throw "ISPC 1.32.0 not found as '$Ispc512'; pass -Ispc512 <path to ispc.exe>" }
  $ver512 = & $Ispc512 --version | Out-String
  if ($ver512 -notmatch "1\.32\.0") { throw "ISPC 1.32.0 required as -Ispc512 (ISPC 1.31.0 goes in -Ispc); got: $ver512" }
}
elseif ($Update) { throw "-Update needs -Ispc512 <path to ISPC 1.32.0>: the _avx512skx objects come from ISPC 1.32.0 (-Bc7eOnly too)" }

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

# Builds both sources with one compiler into $out\bc7e and $out\ispctc.
function Build-Objects([string]$exe, [string]$out) {
  New-Item -ItemType Directory "$out\bc7e", "$out\ispctc" | Out-Null
  Push-Location $repo
  try {
    $common = @("-O2", "--arch=x86-64", "--target-os=windows", "--target=sse2,sse4,avx,avx2,avx512skx-x16", "--opt=fast-math")
    # ISPC prints hundreds of compiler warnings for these two vendored sources; show them only on failure.
    $log = & $exe @common "--opt=disable-assertions" (Get-SourceArg $Bc7eSource) -o "$out\bc7e\bc7e.obj" -h "$out\bc7e\bc7e_ispc.h" 2>&1
    if ($LASTEXITCODE) { $log; throw "ispc failed on $Bc7eSource" }
    $log = & $exe @common (Get-SourceArg "$vendor\ispctc\kernel.ispc") -o "$out\ispctc\kernel.obj" -h "$out\ispctc\kernel_ispc.h" 2>&1
    if ($LASTEXITCODE) { $log; throw "ispc failed on kernel.ispc" }
  }
  finally { Pop-Location }
  Normalize-Header "$out\bc7e\bc7e_ispc.h"
  Normalize-Header "$out\ispctc\kernel_ispc.h"
}

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("sdc-ispc-" + [guid]::NewGuid())
Build-Objects $Ispc "$tmp\131"
if ($Ispc512) { Build-Objects $Ispc512 "$tmp\132" }

# The build each vendored file comes from: the `_avx512skx` objects from ISPC 1.32.0, everything else from 1.31.0
# (1.31.0's `_avx512skx` objects are never vendored).
function Get-Owner([string]$name) {
  if ($name -like "*_avx512skx.obj") { if ($Ispc512) { return "$tmp\132" } else { return $null } }
  return "$tmp\131"
}

$diff = 0
foreach ($dir in "bc7e", "ispctc") {
  # the per-target headers ISPC also emits are not vendored; compare objects and the main header
  $outputs = Get-ChildItem "$tmp\131\$dir" | Where-Object { $_.Extension -eq ".obj" -or $_.Name -in "bc7e_ispc.h", "kernel_ispc.h" }
  foreach ($o in $outputs) {
    $owner = Get-Owner $o.Name
    if (-not $owner) { "{0,-5} {1}/{2}" -f "SKIP", $dir, $o.Name; continue }
    $new = "$owner\$dir\$($o.Name)"
    $old = "$vendor\ispc-objs\$dir\$($o.Name)"
    $same = (Test-Path $old) -and ((Get-FileHash $old).Hash -eq (Get-FileHash $new).Hash)
    if (-not $same) { $diff++ }
    "{0,-5} {1}/{2}" -f ($(if ($same) { "MATCH" } else { "DIFF" })), $dir, $o.Name
  }
}
if ($Update) {
  foreach ($dir in $(if ($Bc7eOnly) { @("bc7e") } else { @("bc7e", "ispctc") })) {
    $outputs = Get-ChildItem "$tmp\131\$dir" | Where-Object { $_.Extension -eq ".obj" -or $_.Name -in "bc7e_ispc.h", "kernel_ispc.h" }
    foreach ($o in $outputs) { Copy-Item "$(Get-Owner $o.Name)\$dir\$($o.Name)" "$vendor\ispc-objs\$dir\" -Force }
  }
  "Updated vendor/ispc-objs. Now run tools/hash-vendor.ps1."
}
Remove-Item -Recurse -Force $tmp
exit $(if ($Update) { 0 } else { $diff })

#Requires -Version 7
# Compile trial of render3d's shader permutations with FXC, DXC (DXIL and SPIR-V) and glslang, for the graphics-backend
# research (2026-11-16). Input: the folder `dump_permutations` (crates/render3d/tests/backend_baseline.rs) wrote, one
# include-expanded `<n>.hlsl` and one `<n>.json` ({rel, target, entry, defines, groups}) per permutation.
# For each permutation, serially, one process at a time, timed from start to exit:
#   fxc      fxc.exe /T <target> /E <entry> /O3 /D K=V ...                      (the baseline: what render3d does today)
#   dxil     dxc.exe -T <vs|ps|cs>_6_0 -E <entry> -O3 -D K=V ...                (the D3D12 SM6 route)
#   spirv    dxc.exe ... -spirv -fspv-target-env=vulkan1.2 -fvk-use-dx-layout, registers shifted as for glslang
#            (Vulkan / wgpu route; skipped with a note when this dxc was built without SPIR-V codegen, as the
#            Windows SDK's is: pass -Dxc <dxc.exe from a github.com/microsoft/DirectXShaderCompiler release>)
#   glslang  glslangValidator -D -V -e <entry> -S <vert|frag|comp> --target-env vulkan1.2 --hlsl-iomap, registers
#            shifted per class (t +0, s +200, b +300, u +400) so they do not collide in set 0
# A failed dxil or spirv compile is retried once with -HV 2018 (row compiler "<name>-hv2018").
# Writes to -Out: results.csv (id, rel, target, entry, groups, compiler, ok, ms, bytes, error = first error line,
# error_norm, errors_all = every distinct normalised error of that compile), summary.md (pass counts, total and median
# ms, process-spawn floor, distinct first errors with counts and one example, and every error with the number of
# permutations reporting it), and bin\<id>.<compiler>.<dxbc|dxil|spv> for every output.
# Normalised = from the first "error" on, without file, line and column.
#
#   .\tools\r3d-dxc-trial.ps1 -Dump <dir> -Out <dir> [-Only <id,id>] [-Limit <n>] [-TimeoutS 180]
#                             [-SdkBin <Windows SDK bin\x64>] [-Dxc <dxc.exe>] [-Glslang <glslangValidator.exe>]
#                             [-Skip <fxc,dxil,...>]
param(
    [Parameter(Mandatory)] [string]$Dump,
    [Parameter(Mandatory)] [string]$Out,
    [string]$Only,
    [int]$Limit = 0,
    [int]$TimeoutS = 180,
    [string]$SdkBin = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64',
    [string]$Dxc = '',
    [string]$Glslang = 'F:\ai\dds\tools\RenderDoc_1.43_64\plugins\spirv\glslangValidator.exe',
    [string]$Skip = ''
)
$ErrorActionPreference = 'Stop'
$fxc = Join-Path $SdkBin 'fxc.exe'
$dxc = if ($Dxc) { $Dxc } else { Join-Path $SdkBin 'dxc.exe' }
$skipSet = @($Skip -split ',' | Where-Object { $_ })
New-Item -ItemType Directory -Force $Out, (Join-Path $Out 'bin'), (Join-Path $Out 'tmp') | Out-Null
$Out = (Resolve-Path $Out).Path

# Runs one process with both streams read asynchronously (a warning flood cannot block it). Returns ok, ms, output.
function Invoke-Tool([string]$exe, [string[]]$argv) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new($exe)
    foreach ($a in $argv) { $psi.ArgumentList.Add($a) }
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $p = [System.Diagnostics.Process]::Start($psi)
    $so = $p.StandardOutput.ReadToEndAsync()
    $se = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit($TimeoutS * 1000)) {
        $p.Kill($true)
        $p.WaitForExit()
        $sw.Stop()
        return @{ ok = $false; ms = $sw.Elapsed.TotalMilliseconds; text = "error: timeout after $TimeoutS s" }
    }
    $p.WaitForExit()
    $sw.Stop()
    return @{ ok = ($p.ExitCode -eq 0); ms = $sw.Elapsed.TotalMilliseconds; text = ($so.Result + "`n" + $se.Result) }
}

# The first line that reports an error (not glslang's "N compilation errors" tail).
function Get-FirstError([string]$text) {
    foreach ($l in ($text -split "`r?`n")) {
        if ($l -match '(?i)error' -and $l -notmatch '(?i)compilation errors|^\s*$') { return $l.Trim() }
    }
    $t = ($text -split "`r?`n" | Where-Object { $_.Trim() } | Select-Object -First 1)
    if ($t) { return $t.Trim() } else { return '' }
}

# The error without where it happened: text from the first "error", file:line:col and (line,col) removed.
function Get-NormError([string]$line) {
    $i = $line.IndexOf('error', [StringComparison]::OrdinalIgnoreCase)
    $s = if ($i -ge 0) { $line.Substring($i) } else { $line }
    $s = $s -replace '[A-Za-z]:\\[^:]*?:\d+(:\d+)?:?', ''
    $s = $s -replace '[\w./\\-]+\.hlsli?(\(\d+(,\d+)?(-\d+)?\)|:\d+(:\d+)?):?', ''
    $s = $s -replace '\(\d+(,\d+)?(-\d+)?\)', ''
    return ($s -replace '\s+', ' ').Trim()
}

# Every distinct normalised error of one compile (a first error can hide the rest).
function Get-AllErrors([string]$text) {
    $set = [System.Collections.Generic.SortedSet[string]]::new()
    foreach ($l in ($text -split "`r?`n")) {
        if ($l -match '(?i)\berror\b' -and $l -notmatch '(?i)compilation errors|errors? generated') {
            [void]$set.Add((Get-NormError $l.Trim()))
        }
    }
    return @($set)
}

function Get-Stage([string]$target) { return $target.Substring(0, 2) }

function Get-Args([string]$compiler, $j, [string]$in, [string]$bin, [bool]$hv2018) {
    $stage = Get-Stage $j.target
    $defs = @($j.defines)
    switch ($compiler) {
        'fxc' {
            $a = @('/nologo', '/T', $j.target, '/E', $j.entry, '/O3')
            foreach ($d in $defs) { $a += @('/D', $d) }
            return @{ exe = $fxc; argv = $a + @('/Fo', $bin, $in) }
        }
        'dxil' {
            $a = @('-T', "${stage}_6_0", '-E', $j.entry, '-O3')
            if ($hv2018) { $a += @('-HV', '2018') }
            foreach ($d in $defs) { $a += @('-D', $d) }
            return @{ exe = $dxc; argv = $a + @('-Fo', $bin, $in) }
        }
        'spirv' {
            # Without the shifts b0, t0 and s0 would all ask for binding 0 of set 0.
            $a = @('-T', "${stage}_6_0", '-E', $j.entry, '-O3', '-spirv', '-fspv-target-env=vulkan1.2', '-fvk-use-dx-layout',
                '-fvk-t-shift', '0', '0', '-fvk-s-shift', '200', '0', '-fvk-b-shift', '300', '0', '-fvk-u-shift', '400', '0')
            if ($hv2018) { $a += @('-HV', '2018') }
            foreach ($d in $defs) { $a += @('-D', $d) }
            return @{ exe = $dxc; argv = $a + @('-Fo', $bin, $in) }
        }
        'glslang' {
            $s = @{ vs = 'vert'; ps = 'frag'; cs = 'comp' }[$stage]
            $a = @('-D', '-V', '-e', $j.entry, '-S', $s, '--target-env', 'vulkan1.2', '--hlsl-iomap',
                '--shift-texture-binding', '0', '--shift-ssbo-binding', '0', '--shift-sampler-binding', '200',
                '--shift-cbuffer-binding', '300', '--shift-uav-binding', '400')
            foreach ($d in $defs) { $a += "-D$d" }
            return @{ exe = $Glslang; argv = $a + @('-o', $bin, $in) }
        }
    }
}

$ext = @{ fxc = 'dxbc'; dxil = 'dxil'; spirv = 'spv'; glslang = 'spv' }
$compilers = @('fxc', 'dxil', 'spirv', 'glslang') | Where-Object { $skipSet -notcontains $_ }

# Probe: does this dxc have SPIR-V codegen? And the process-spawn floor on a trivial shader (median of 5).
$tmp = Join-Path $Out 'tmp'
$triv = Join-Path $tmp 'trivial.hlsl'
Set-Content -Encoding ascii $triv 'float4 main(float4 p : POSITION) : SV_Position { return p * 2; }'
$tj = [pscustomobject]@{ target = 'vs_5_0'; entry = 'main'; defines = @() }
$floor = @{}
$notes = @()
foreach ($c in $compilers) {
    $times = @()
    $res = $null
    for ($k = 0; $k -lt 5; $k++) {
        $g = Get-Args $c $tj $triv (Join-Path $tmp "trivial.$c") $false
        $res = Invoke-Tool $g.exe $g.argv
        $times += $res.ms
    }
    $floor[$c] = ($times | Sort-Object)[2]
    if (-not $res.ok) { $notes += "- $c cannot compile a trivial shader: $(Get-FirstError $res.text)" }
}
if ($notes -match 'SPIR-V CodeGen not available') {
    $compilers = $compilers | Where-Object { $_ -ne 'spirv' }
}

$jsons = Get-ChildItem $Dump -Filter *.json | Sort-Object { [int]$_.BaseName }
if ($Only) { $ids = $Only -split ','; $jsons = $jsons | Where-Object { $ids -contains $_.BaseName } }
if ($Limit -gt 0) { $jsons = $jsons | Select-Object -First $Limit }
$rows = [System.Collections.Generic.List[object]]::new()
$n = 0
foreach ($f in $jsons) {
    $n++
    $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
    $in = Join-Path $Dump "$($f.BaseName).hlsl"
    foreach ($c in $compilers) {
        $tries = @($false)
        if ($c -in 'dxil', 'spirv') { $tries += $true }
        foreach ($hv in $tries) {
            $name = if ($hv) { "$c-hv2018" } else { $c }
            $bin = Join-Path $Out "bin\$($f.BaseName).$name.$($ext[$c])"
            $g = Get-Args $c $j $in $bin $hv
            $r = Invoke-Tool $g.exe $g.argv
            $err = if ($r.ok) { '' } else { Get-FirstError $r.text }
            $rows.Add([pscustomobject]@{
                    id = [int]$f.BaseName; rel = $j.rel; target = $j.target; entry = $j.entry
                    groups = (@($j.groups) -join ' '); compiler = $name; ok = $r.ok
                    ms = [math]::Round($r.ms, 1)
                    bytes = if ($r.ok -and (Test-Path $bin)) { (Get-Item $bin).Length } else { 0 }
                    error = $err; error_norm = if ($r.ok) { '' } else { Get-NormError $err }
                    errors_all = if ($r.ok) { '' } else { (Get-AllErrors $r.text) -join ' || ' }
                })
            if ($r.ok) { break }
        }
    }
    Write-Host ("[{0}/{1}] {2} {3} {4}: {5}" -f $n, $jsons.Count, $f.BaseName, $j.rel, $j.target,
        (($rows | Where-Object id -eq ([int]$f.BaseName) | ForEach-Object { "$($_.compiler)=$(if ($_.ok) {'ok'} else {'FAIL'})" }) -join ' '))
}
$rows | Export-Csv -NoTypeInformation -Encoding utf8 (Join-Path $Out 'results.csv')

$md = @("# r3d-dxc-trial summary", '', "Dump: $Dump; permutations: $($jsons.Count); run $(Get-Date -Format s)", '')
$md += "fxc: $fxc ($((Get-Item $fxc).VersionInfo.FileVersion)); dxc: $dxc ($((Get-Item (Join-Path (Split-Path $dxc) 'dxcompiler.dll')).VersionInfo.FileVersion)); glslang: $Glslang"
$md += ''
$md += $notes
$md += ''
$md += '| compiler | ok | failed | total ms | median ms (ok) | max ms (ok) | spawn floor ms (trivial VS, median of 5) |'
$md += '|---|---:|---:|---:|---:|---:|---:|'
foreach ($c in ($rows.compiler | Select-Object -Unique)) {
    $cr = $rows | Where-Object compiler -eq $c
    $ok = @($cr | Where-Object ok)
    $times = @($ok.ms | Sort-Object)
    $med = if ($times.Count) { $times[[int][math]::Floor(($times.Count - 1) / 2)] } else { 0 }
    $max = if ($times.Count) { $times[-1] } else { 0 }
    $base = $c -replace '-hv2018$', ''
    $md += "| $c | $($ok.Count) | $($cr.Count - $ok.Count) | $([math]::Round(($cr.ms | Measure-Object -Sum).Sum)) | $med | $max | $($floor[$base]) |"
}
$md += ''
$md += 'Pass counts by target (first attempt, then after the -HV 2018 retry):'
$md += ''
$md += '| compiler | target | ok | of | ok after retry |'
$md += '|---|---|---:|---:|---:|'
foreach ($c in $compilers) {
    foreach ($t in ($rows.target | Select-Object -Unique)) {
        $first = @($rows | Where-Object { $_.compiler -eq $c -and $_.target -eq $t })
        $retry = @($rows | Where-Object { $_.compiler -eq "$c-hv2018" -and $_.target -eq $t -and $_.ok })
        $md += "| $c | $t | $(@($first | Where-Object ok).Count) | $($first.Count) | $(@($first | Where-Object ok).Count + $retry.Count) |"
    }
}
$md += ''
$md += '## Distinct errors (normalised: no file, line or column)'
$md += ''
foreach ($c in ($rows.compiler | Select-Object -Unique)) {
    $bad = @($rows | Where-Object { $_.compiler -eq $c -and -not $_.ok })
    if (-not $bad.Count) { continue }
    $md += "### $c ($($bad.Count) failures)"
    $md += ''
    $md += '| count | error | example id | example (raw first error line) |'
    $md += '|---:|---|---:|---|'
    foreach ($g in ($bad | Group-Object error_norm | Sort-Object Count -Descending)) {
        $ex = $g.Group[0]
        $md += "| $($g.Count) | $($g.Name -replace '\|', '\|') | $($ex.id) | $($ex.error -replace '\|', '\|') |"
    }
    $md += ''
    $md += "Every error each failed compile reported (permutations that report it):"
    $md += ''
    $md += '| permutations | error |'
    $md += '|---:|---|'
    $all = $bad | ForEach-Object { $_.errors_all -split ' \|\| ' | Where-Object { $_ } }
    foreach ($g in ($all | Group-Object | Sort-Object Count -Descending)) {
        $md += "| $($g.Count) | $($g.Name -replace '\|', '\|') |"
    }
    $md += ''
}
$md | Set-Content -Encoding utf8 (Join-Path $Out 'summary.md')
Write-Host "wrote $(Join-Path $Out 'results.csv') and summary.md"

<#
.SYNOPSIS
  Builds the mixed profiling corpus (MIX, spec 7.2) as a read-only copy of picked files from MO2 mods
  (plan 2026-11-21-engine-profile, Task 6). NOTHING IN THIS SCRIPT DELETES, EMPTIES, MOVES OR OVERWRITES
  A FILE OR FOLDER (EP-R3): sources are only read, every copy is a new-file open, -Dest must not exist.

.DESCRIPTION
  tools\profile-corpus.ps1 -Mods <folder> -Dest <folder> [-Seed <int>] [-MaxGB <n>] [-SelfTest]

  -Mods     the MO2 mods folder: one folder per mod, each with a textures folder. Read only.
  -Dest     the new corpus folder. Must not exist; it is created. Refused when it overlaps a protected path
            (tools\profile-paths.ps1, junctions resolved, whole path components), -Mods, or an owner project.
            The real mixed-corpus folder is the one protected entry that is allowed as a new destination.
  -Seed     fixed seed of every pick (default 20261121). The same seed and tree give the same picks.
  -MaxGB    byte cap of the whole corpus (default 15).
  -SelfTest builds a fake mods tree under a new folder and leaves it in place; checks picks, alpha formats, seeds,
            source hashes and every -Dest refusal against a stand-in protected tree. Writes nothing elsewhere.
  (A hidden -SelfTestRoot sets the parent of the self-test folder, default a folder in the temp folder; checked like -Dest.)

  Output: <Dest>\<mod>\textures\... (each file's path kept below its mod), sources.csv, coverage.md.
  Quotas (files unless noted): 4K normals 40; matched PBR sets 30 (sets); source images 100; raw packing maps 40;
  icons 60; foliage candidates 60; matched 1K-2K diffuse + normal pairs 100. After copying, every source is
  re-hashed and the run fails when one changed.
#>
[CmdletBinding()]
param(
    [string]$Mods,
    [string]$Dest,
    [int]$Seed = 20261121,
    [double]$MaxGB = 15,
    [switch]$SelfTest,
    [Parameter(DontShow)][string]$SelfTestRoot
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'profile-paths.ps1')

# The one protected entry that is allowed as a new destination, read from the shared list (not a second literal).
function Get-MixedCorpusPath { @(Get-ProtectedList | Where-Object { (Split-Path -Leaf $_.TrimEnd('\')) -ieq 'mixed-corpus' })[0] }
$MixedCorpus = Get-MixedCorpusPath

# Relative arguments resolve against PowerShell's location, the same base Test-Path uses.
foreach ($n in 'Mods', 'Dest', 'SelfTestRoot') {
    $v = Get-Variable $n -ValueOnly
    if ($v) { Set-Variable $n ($PSCmdlet.GetUnresolvedProviderPathFromPSPath($v)) }
}

# ---------------------------------------------------------------- new-file writers

function Write-NewBytes([string]$path, [byte[]]$bytes) {
    $fs = [IO.FileStream]::new($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try { $fs.Write($bytes, 0, $bytes.Length) } finally { $fs.Dispose() }
}

function Write-NewText([string]$path, [string]$text) {
    Write-NewBytes $path ([Text.UTF8Encoding]::new($false).GetBytes($text))
}

# Copies with a new-file open on the destination and returns the SHA-256 of what was read.
function Copy-NewFile([string]$src, [string]$dst) {
    [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($dst))
    $in = [IO.FileStream]::new($src, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $out = $null
    $h = [Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
    try {
        $out = [IO.FileStream]::new($dst, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $buf = New-Object byte[] (1MB)
        while (($n = $in.Read($buf, 0, $buf.Length)) -gt 0) {
            $h.AppendData($buf, 0, $n)
            $out.Write($buf, 0, $n)
        }
        $hex = [Convert]::ToHexString($h.GetHashAndReset())
    } finally {
        $in.Dispose()
        if ($out) { $out.Dispose() }
        $h.Dispose()
    }
    $hex
}

# ---------------------------------------------------------------- image headers

# Reads the first 148 bytes: size and format come from them alone (legacy FourCC at 84, DXGI at 128 for DX10).
function Read-DdsHeader([string]$path) {
    $buf = New-Object byte[] 148
    $n = 0
    $fs = $null
    try {
        $fs = [IO.FileStream]::new($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        while ($n -lt 148 -and ($k = $fs.Read($buf, $n, 148 - $n)) -gt 0) { $n += $k }
    } catch { return $null } finally { if ($fs) { $fs.Dispose() } }
    if ($n -lt 128 -or [BitConverter]::ToUInt32($buf, 0) -ne 0x20534444) { return $null }
    $h = [int][BitConverter]::ToUInt32($buf, 12)
    $w = [int][BitConverter]::ToUInt32($buf, 16)
    $pf = [BitConverter]::ToUInt32($buf, 80)
    $fccVal = [BitConverter]::ToUInt32($buf, 84)
    $hasFcc = ($pf -band 4) -ne 0 -and $fccVal -ne 0    # DDPF_FOURCC
    $d3d = if ($hasFcc -and $fccVal -lt 0x100) { $fccVal } else { 0 }    # numeric D3DFMT code in the FourCC field
    $fcc = if ($hasFcc -and $fccVal -ge 0x100) { [Text.Encoding]::ASCII.GetString($buf, 84, 4) } else { '' }
    $fmt = ''
    $alpha = $false
    if ($fcc -eq 'DX10') {
        if ($n -lt 148) { return $null }
        $dxgi = [int][BitConverter]::ToUInt32($buf, 128)
        $mode = [int]([BitConverter]::ToUInt32($buf, 144) -band 7)
        if ($dxgi -ge 70 -and $dxgi -le 72) { $fmt = 'BC1'; $alpha = ($mode -eq 1 -or $mode -eq 2) }
        elseif ($dxgi -ge 73 -and $dxgi -le 75) { $fmt = 'BC2'; $alpha = $true }
        elseif ($dxgi -ge 76 -and $dxgi -le 78) { $fmt = 'BC3'; $alpha = $true }
        elseif ($dxgi -ge 79 -and $dxgi -le 81) { $fmt = 'BC4' }
        elseif ($dxgi -ge 82 -and $dxgi -le 84) { $fmt = 'BC5' }
        elseif ($dxgi -ge 94 -and $dxgi -le 96) { $fmt = 'BC6H' }
        elseif ($dxgi -ge 97 -and $dxgi -le 99) { $fmt = 'BC7'; $alpha = $true }
        elseif ($dxgi -in 28, 29) { $fmt = 'RGBA8'; $alpha = $true }
        elseif ($dxgi -in 87, 91) { $fmt = 'BGRA8'; $alpha = $true }
        else { $fmt = "DXGI$dxgi" }
    } elseif ($fcc) {
        switch ($fcc) {
            'DXT1' { $fmt = 'BC1'; $alpha = (($pf -band 1) -ne 0) }
            'DXT2' { $fmt = 'BC2'; $alpha = $true }
            'DXT3' { $fmt = 'BC2'; $alpha = $true }
            'DXT4' { $fmt = 'BC3'; $alpha = $true }
            'DXT5' { $fmt = 'BC3'; $alpha = $true }
            { $_ -in 'ATI1', 'BC4U', 'BC4S' } { $fmt = 'BC4' }
            { $_ -in 'ATI2', 'BC5U', 'BC5S' } { $fmt = 'BC5' }
            default { $fmt = "FOURCC-$fcc" }
        }
    } elseif ($d3d) {
        $fmt = "D3DFMT$d3d"
    } else {
        $fmt = 'RAW' + [BitConverter]::ToUInt32($buf, 88)
        $alpha = (($pf -band 1) -ne 0)
    }
    [pscustomobject]@{ Width = $w; Height = $h; Format = $fmt; Alpha = $alpha }
}

function Read-PngSize([string]$path) {
    $buf = New-Object byte[] 24
    $fs = $null
    try {
        $fs = [IO.FileStream]::new($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        $n = $fs.Read($buf, 0, 24)
    } catch { return $null } finally { if ($fs) { $fs.Dispose() } }
    if ($n -lt 24 -or $buf[1] -ne 0x50 -or $buf[2] -ne 0x4E -or $buf[3] -ne 0x47) { return $null }
    $w = ([int]$buf[16] -shl 24) -bor ([int]$buf[17] -shl 16) -bor ([int]$buf[18] -shl 8) -bor [int]$buf[19]
    $h = ([int]$buf[20] -shl 24) -bor ([int]$buf[21] -shl 16) -bor ([int]$buf[22] -shl 8) -bor [int]$buf[23]
    [pscustomobject]@{ Width = $w; Height = $h }
}

# ---------------------------------------------------------------- destination check

# Reason a -Dest is refused, or $null. Overlap is compared after resolving junctions (Resolve-Real inside
# Get-RunRootProblem and Test-Overlap), on whole path components.
function Get-DestProblem([string]$dest, [string]$mods, [string[]]$protected, [string]$mixed = $MixedCorpus) {
    $full = [IO.Path]::GetFullPath($dest)
    if (Test-Path -LiteralPath $full) { return 'already exists (the builder never reuses a folder)' }
    $mixedNorm = if ($mixed) { [IO.Path]::GetFullPath($mixed).TrimEnd('\') } else { $null }
    $list = @($protected | Where-Object { -not $mixedNorm -or [IO.Path]::GetFullPath($_).TrimEnd('\') -ine $mixedNorm })
    $why = Get-RunRootProblem $full $list
    if ($why) { return $why }
    if ($mixedNorm -and (Test-Path -LiteralPath $mixedNorm) -and (Test-Contained $full $mixedNorm)) { return 'is inside the existing mixed corpus' }
    if (Test-Overlap $full $mods) { return 'overlaps -Mods' }
    $null
}

# ---------------------------------------------------------------- selection

function Get-Rank([string]$s) {
    $b = [Text.Encoding]::UTF8.GetBytes("$($script:Ctx.Seed)|$($s.ToLowerInvariant())")
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($b))
}

# Fixed seeded order: sorted by the SHA-256 of "<seed>|<lower-case key>".
function Sort-Seeded([string[]]$items) {
    if (-not $items -or $items.Count -eq 0) { return , @() }
    $keys = [string[]]::new($items.Count)
    for ($i = 0; $i -lt $items.Count; $i++) { $keys[$i] = Get-Rank $items[$i] }
    $arr = [string[]]$items.Clone()
    [Array]::Sort($keys, $arr, [StringComparer]::Ordinal)
    , $arr
}

function Get-Hdr([string]$rel) {
    $c = $script:Ctx
    if (-not $c.Hdr.ContainsKey($rel)) { $c.Hdr[$rel] = Read-DdsHeader (Join-Path $c.Mods $rel) }
    $c.Hdr[$rel]
}

function Get-Size([string]$rel) { ([IO.FileInfo]::new((Join-Path $script:Ctx.Mods $rel))).Length }

# Adds the files of one pick (a file, or the members of a set) when none is used yet and the byte cap allows it.
function Add-Pick([string[]]$rels, [string]$cat) {
    $c = $script:Ctx
    foreach ($r in $rels) { if ($c.Used.Contains($r)) { return $false } }
    $size = 0L
    foreach ($r in $rels) { $size += Get-Size $r }
    if ($c.Bytes + $size -gt $c.Budget) { $c.Capped = $true; return $false }
    foreach ($r in $rels) { [void]$c.Used.Add($r); $c.Picks.Add([pscustomobject]@{ Rel = $r; Category = $cat }) }
    $c.Bytes += $size
    $c.Cov[$cat].Files += $rels.Count
    $c.Cov[$cat].Bytes += $size
    $true
}

function Pick-Files([string]$cat, [string[]]$cands, [int]$quota, [scriptblock]$test) {
    $c = $script:Ctx
    foreach ($r in (Sort-Seeded $cands)) {
        if ($c.Cov[$cat].Files -ge $quota) { break }
        if ($c.Used.Contains($r)) { continue }
        if ($test) { $h = Get-Hdr $r; if (-not $h -or -not (& $test $h)) { continue } }
        [void](Add-Pick @($r) $cat)
    }
}

function Select-Corpus([string]$mods, [int]$seed, [double]$maxGB) {
    $modsFull = [IO.Path]::GetFullPath($mods).TrimEnd('\')
    $script:Ctx = @{
        Mods = $modsFull; Seed = $seed; Budget = [long]($maxGB * 1GB); Bytes = 0L; Capped = $false
        Used = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        Picks = [Collections.Generic.List[object]]::new(); Hdr = @{}; Cov = [ordered]@{}
    }
    $quota = [ordered]@{ 'pbr-set' = 30; 'normal-4k' = 40; 'pair-1k-2k' = 100; 'foliage' = 60; 'icon' = 60; 'raw-map' = 40; 'source-image' = 100 }
    foreach ($k in $quota.Keys) { $script:Ctx.Cov[$k] = @{ Quota = $quota[$k]; Files = 0; Bytes = 0L; Sets = 0; Candidates = 0 } }

    $dds = [Collections.Generic.List[string]]::new()
    $img = [Collections.Generic.List[string]]::new()
    $opt = [IO.EnumerationOptions]::new()
    $opt.RecurseSubdirectories = $true
    $opt.IgnoreInaccessible = $true
    # shortcut: junctions and symlinks (a mod folder, its textures folder, or any folder or file below) are skipped, not followed, so a link cannot lead the scan out of -Mods; upgrade when an owner mod is a link.
    $opt.AttributesToSkip = [IO.FileAttributes]::ReparsePoint
    $top = [IO.EnumerationOptions]::new()
    $top.IgnoreInaccessible = $true
    $top.AttributesToSkip = [IO.FileAttributes]::ReparsePoint
    foreach ($modDir in [IO.Directory]::EnumerateDirectories($modsFull, '*', $top)) {
        $tex = Get-ChildItem -LiteralPath $modDir -Directory -Filter 'textures' -Force | Where-Object { -not $_.LinkType } | Select-Object -First 1
        if (-not $tex) { continue }
        foreach ($f in [IO.Directory]::EnumerateFiles($tex.FullName, '*', $opt)) {
            $rel = $f.Substring($modsFull.Length + 1)
            switch ([IO.Path]::GetExtension($f).ToLowerInvariant()) {
                '.dds' { $dds.Add($rel) }
                { $_ -in '.png', '.tga', '.tif', '.tiff' } { $img.Add($rel) }
            }
        }
    }

    # Matched groups by base name in one folder: roles c (colour), n (_n) and r (_rmaos); sets and pairs both read this table.
    $groups = @{}
    foreach ($rel in $dds) {
        $name = [IO.Path]::GetFileName($rel).ToLowerInvariant()
        $folder = [IO.Path]::GetDirectoryName($rel).ToLowerInvariant()
        if ($name -match '^(.+)_rmaos\.dds$') { $role = 'r'; $base = $Matches[1] }
        elseif ($name -match '^(.+)_n\.dds$') { $role = 'n'; $base = $Matches[1] }
        else { $role = 'c'; $base = $name.Substring(0, $name.Length - 4) }
        $key = "$folder|$base"
        if (-not $groups.ContainsKey($key)) { $groups[$key] = @{} }
        if (-not $groups[$key].ContainsKey($role)) { $groups[$key][$role] = $rel }
    }

    $sets = @($groups.Keys | Where-Object { $g = $groups[$_]; $g.c -and $g.n -and $g.r })
    $script:Ctx.Cov['pbr-set'].Candidates = $sets.Count
    foreach ($k in (Sort-Seeded $sets)) {
        if ($script:Ctx.Cov['pbr-set'].Sets -ge 30) { break }
        $g = $groups[$k]
        if (Add-Pick @($g.c, $g.n, $g.r) 'pbr-set') { $script:Ctx.Cov['pbr-set'].Sets++ }
    }

    $nCands = @($dds | Where-Object { $_.EndsWith('_n.dds', [StringComparison]::OrdinalIgnoreCase) })
    $script:Ctx.Cov['normal-4k'].Candidates = $nCands.Count
    Pick-Files 'normal-4k' $nCands 40 { param($h) [Math]::Max($h.Width, $h.Height) -ge 4096 }

    $pairs = @($groups.Keys | Where-Object { $g = $groups[$_]; $g.c -and $g.n -and -not $g.r })
    $script:Ctx.Cov['pair-1k-2k'].Candidates = $pairs.Count
    foreach ($k in (Sort-Seeded $pairs)) {
        if ($script:Ctx.Cov['pair-1k-2k'].Files -ge 100) { break }
        $g = $groups[$k]
        $ok = $true
        foreach ($r in @($g.c, $g.n)) {
            $h = Get-Hdr $r
            if (-not $h -or [Math]::Max($h.Width, $h.Height) -lt 1024 -or [Math]::Max($h.Width, $h.Height) -gt 2048) { $ok = $false }
        }
        if ($ok) { [void](Add-Pick @($g.c, $g.n) 'pair-1k-2k') }
    }

    $fol = @($dds | Where-Object {
            $parts = $_.Split('\')
            $parts.Count -ge 4 -and -not ($_.EndsWith('_n.dds', [StringComparison]::OrdinalIgnoreCase) -or $_.EndsWith('_rmaos.dds', [StringComparison]::OrdinalIgnoreCase)) -and
            (@($parts[2..($parts.Count - 2)] | Where-Object { $_ -in 'landscape', 'plants', 'trees' }).Count -gt 0)
        })
    $script:Ctx.Cov['foliage'].Candidates = $fol.Count
    Pick-Files 'foliage' $fol 60 { param($h) $h.Alpha }

    $icons = @($dds | Where-Object { $p = $_.Split('\'); $p.Count -ge 4 -and $p[2] -ieq 'interface' })
    $script:Ctx.Cov['icon'].Candidates = $icons.Count
    Pick-Files 'icon' $icons 60 { param($h) [Math]::Max($h.Width, $h.Height) -le 256 }

    $rawRe = '(^|[_\-. ])(rough(ness)?|metal(lic|ness)?|ao|occlusion|ambientocclusion)([_\-. ]|$)'
    $raw = @($img | Where-Object { [IO.Path]::GetFileNameWithoutExtension($_) -match $rawRe })
    $script:Ctx.Cov['raw-map'].Candidates = $raw.Count
    Pick-Files 'raw-map' $raw 40 $null

    $srcs = @($img | Where-Object { [IO.Path]::GetFileNameWithoutExtension($_) -notmatch $rawRe })
    $script:Ctx.Cov['source-image'].Candidates = $srcs.Count
    Pick-Files 'source-image' $srcs 100 $null
}

# ---------------------------------------------------------------- build

function Csv([object]$v) { '"' + ([string]$v).Replace('"', '""') + '"' }

function Invoke-Build([string]$mods, [string]$dest, [int]$seed, [double]$maxGB, [string[]]$protected) {
    if (-not $mods -or -not (Test-Path -LiteralPath $mods -PathType Container)) { throw "refused: -Mods $mods is not an existing folder" }
    $modsFull = [IO.Path]::GetFullPath($mods).TrimEnd('\')
    $why = Get-DestProblem $dest $modsFull $protected
    if ($why) { throw "refused: -Dest $dest $why" }
    $destFull = [IO.Path]::GetFullPath($dest)

    Select-Corpus $modsFull $seed $maxGB
    $c = $script:Ctx
    # The scan can take long: check the destination again right before creating it.
    $why = Get-DestProblem $destFull $modsFull $protected
    if ($why) { throw "refused: -Dest $dest $why" }
    New-Item -ItemType Directory -Path $destFull | Out-Null    # no -Force: fails if the folder exists

    $rows = [Collections.Generic.List[object]]::new()
    foreach ($p in $c.Picks) {
        $src = Join-Path $modsFull $p.Rel
        $dst = Join-Path $destFull $p.Rel
        $sha = Copy-NewFile $src $dst
        $h = $null
        $ext = [IO.Path]::GetExtension($src).TrimStart('.').ToLowerInvariant()
        if ($ext -eq 'dds') { $h = Get-Hdr $p.Rel } elseif ($ext -eq 'png') { $s = Read-PngSize $src; if ($s) { $h = [pscustomobject]@{ Format = 'png'; Width = $s.Width; Height = $s.Height } } }
        $fmt = if ($h -and $h.Format) { $h.Format } else { $ext }
        $rows.Add([pscustomobject]@{
                Source = $src; Destination = $dst; Bytes = ([IO.FileInfo]::new($src)).Length; Sha256 = $sha; Category = $p.Category
                Format = $fmt; Width = $(if ($h) { $h.Width } else { '' }); Height = $(if ($h) { $h.Height } else { '' })
            })
    }

    # Post-copy re-hash: every source must be unchanged, and every copy must equal its source.
    foreach ($r in $rows) {
        if ((Get-FileHash -LiteralPath $r.Source -Algorithm SHA256).Hash -ne $r.Sha256) { throw "FAILED: source changed while copying: $($r.Source)" }
        if ((Get-FileHash -LiteralPath $r.Destination -Algorithm SHA256).Hash -ne $r.Sha256) { throw "FAILED: copy differs from its source: $($r.Destination)" }
    }

    $csv = [Text.StringBuilder]::new()
    [void]$csv.Append("source,destination,bytes,sha256,category,format,width,height`r`n")
    foreach ($r in $rows) {
        [void]$csv.Append(((@($r.Source, $r.Destination, $r.Bytes, $r.Sha256, $r.Category, $r.Format, $r.Width, $r.Height) | ForEach-Object { Csv $_ }) -join ',') + "`r`n")
    }
    Write-NewText (Join-Path $destFull 'sources.csv') $csv.ToString()

    $md = [Text.StringBuilder]::new()
    [void]$md.Append("# Mixed corpus coverage`r`n`r`nSeed $seed, byte cap $maxGB GB, $($rows.Count) files, {0:N2} GB. Source hashes re-checked after copying: all unchanged.`r`n`r`n" -f ($c.Bytes / 1GB))
    [void]$md.Append("| category | quota | picked files | sets | bytes | candidates | note |`r`n|---|---|---|---|---|---|---|`r`n")
    foreach ($k in $c.Cov.Keys) {
        $v = $c.Cov[$k]
        $unit = if ($k -eq 'pbr-set') { $v.Sets } else { $v.Files }
        $note = if ($unit -lt $v.Quota) { "short by $($v.Quota - $unit)" } else { '' }
        [void]$md.Append("| $k | $($v.Quota) | $($v.Files) | $(if ($k -eq 'pbr-set') { $v.Sets } else { '' }) | $($v.Bytes) | $($v.Candidates) | $note |`r`n")
    }
    [void]$md.Append("`r`nCandidates are the files (or complete groups, for pbr-set and pair-1k-2k) that match the category's name rule, counted before the header size and alpha tests.`r`n")
    if ($c.Capped) { [void]$md.Append("`r`nThe byte cap skipped at least one pick.`r`n") }
    Write-NewText (Join-Path $destFull 'coverage.md') $md.ToString()
    [pscustomobject]@{ Dest = $destFull; Rows = $rows; Capped = $c.Capped; Cov = $c.Cov }
}

# ---------------------------------------------------------------- self-test

function Add-Bytes([byte[]]$a, [int]$off, [uint32]$v) { [BitConverter]::GetBytes($v).CopyTo($a, $off) }

# kind: DXT1, DXT1A, DXT5, BC7, BC5, BC1O (DX10 BC1, opaque), BC1S (DX10 BC1, straight alpha)
function New-Dds([string]$path, [int]$w, [int]$h, [string]$kind) {
    $dx10 = $kind -in 'BC7', 'BC5', 'BC1O', 'BC1S'
    $b = New-Object byte[] ($(if ($dx10) { 148 } else { 128 }) + 16)
    Add-Bytes $b 0 0x20534444; Add-Bytes $b 4 124; Add-Bytes $b 8 0x1007; Add-Bytes $b 12 $h; Add-Bytes $b 16 $w
    Add-Bytes $b 76 32
    $pf = 4
    if ($kind -eq 'DXT1A') { $pf = 5 }
    Add-Bytes $b 80 $pf
    if ($kind -eq 'D3D113') { Add-Bytes $b 84 113 } else {    # numeric D3DFMT code in the FourCC field
        $fcc = switch ($kind) { 'DXT1' { 'DXT1' } 'DXT1A' { 'DXT1' } 'DXT5' { 'DXT5' } default { 'DX10' } }
        [Text.Encoding]::ASCII.GetBytes($fcc).CopyTo($b, 84)
    }
    if ($dx10) {
        $dxgi = switch ($kind) { 'BC7' { 98 } 'BC5' { 83 } default { 71 } }
        Add-Bytes $b 128 $dxgi; Add-Bytes $b 132 3; Add-Bytes $b 140 1
        Add-Bytes $b 144 $(switch ($kind) { 'BC1O' { 3 } 'BC1S' { 1 } default { 0 } })
    }
    [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($path))
    Write-NewBytes $path $b
}

function New-Png([string]$path, [int]$w, [int]$h) {
    $b = New-Object byte[] 40
    ([byte[]](0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A)).CopyTo($b, 0)
    $b[11] = 13
    [Text.Encoding]::ASCII.GetBytes('IHDR').CopyTo($b, 12)
    foreach ($pair in @(@(16, $w), @(20, $h))) {
        $o = $pair[0]; $v = $pair[1]
        $b[$o] = ($v -shr 24) -band 255; $b[$o + 1] = ($v -shr 16) -band 255; $b[$o + 2] = ($v -shr 8) -band 255; $b[$o + 3] = $v -band 255
    }
    [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($path))
    Write-NewBytes $path $b
}

function Invoke-SelfTest {
    $base = if ($SelfTestRoot) { $SelfTestRoot } else { Join-Path ([IO.Path]::GetTempPath()) 'sdc-profile-corpus-selftest' }
    $baseWhy = Get-RunRootProblem ([IO.Path]::GetFullPath($base)) @(Get-ProtectedList)
    if ($baseWhy) { throw "refused: the self-test folder $base $baseWhy" }
    $t = Join-Path ([IO.Path]::GetFullPath($base)) ('selftest-' + (Get-Date).ToString('yyyyMMdd-HHmmss'))
    New-Item -ItemType Directory -Path $t | Out-Null
    $mods = Join-Path $t 'mods'
    $tex = { param($m, $rest) Join-Path $mods "$m\textures\$rest" }

    # Stand-in protected tree with a sentinel, and a junction into it.
    $prot = Join-Path $t 'prot'
    New-Item -ItemType Directory -Path $prot | Out-Null
    Write-NewText (Join-Path $prot 'keep.txt') 'sentinel'
    New-Item -ItemType Junction -Path (Join-Path $t 'link') -Target $prot | Out-Null
    $sentinel = (Get-FileHash -LiteralPath (Join-Path $prot 'keep.txt') -Algorithm SHA256).Hash

    # Fake mods.
    for ($i = 1; $i -le 45; $i++) { New-Dds (& $tex 'ModN' "arch\nrm$i`_n.dds") 4096 4096 'BC7' }
    for ($i = 1; $i -le 3; $i++) { New-Dds (& $tex 'ModN' "arch\small$i`_n.dds") 512 512 'BC7' }
    foreach ($b in 'a', 'b') {
        New-Dds (& $tex 'ModP' "pbr\$b.dds") 2048 2048 'BC7'
        New-Dds (& $tex 'ModP' "pbr\$b`_n.dds") 2048 2048 'BC5'
        New-Dds (& $tex 'ModP' "pbr\$b`_rmaos.dds") 2048 2048 'BC7'
    }
    New-Dds (& $tex 'ModP' 'decoy\z.dds') 512 512 'BC7'
    New-Dds (& $tex 'ModP' 'decoy\x_n.dds') 512 512 'BC5'
    New-Dds (& $tex 'ModP' 'decoy\y_rmaos.dds') 512 512 'BC7'
    for ($i = 1; $i -le 120; $i++) { New-Png (& $tex 'ModS' ('src\img{0:D3}.png' -f $i)) 64 64 }
    New-Png (& $tex 'ModS' 'src\rock_roughness.png') 64 64
    Write-NewBytes (& $tex 'ModS' 'src\rock_metallic.tga') ([byte[]](1, 2, 3))
    Write-NewBytes (& $tex 'ModS' 'src\rock_ao.tif') ([byte[]](1, 2, 3))
    for ($i = 1; $i -le 65; $i++) { New-Dds (& $tex 'ModI' ("interface\icons\ic$i.dds")) 128 128 'DXT1' }
    foreach ($i in 1, 2) { New-Dds (& $tex 'ModI' "interface\icons\big$i.dds") 512 512 'DXT1' }
    New-Dds (& $tex 'ModF' 'landscape\fbc7.dds') 512 512 'BC7'
    New-Dds (& $tex 'ModF' 'landscape\fbc3.dds') 512 512 'DXT5'
    New-Dds (& $tex 'ModF' 'landscape\fbc1a.dds') 512 512 'DXT1A'
    New-Dds (& $tex 'ModF' 'landscape\fbc1.dds') 512 512 'DXT1'
    New-Dds (& $tex 'ModF' 'plants\fdx.dds') 512 512 'BC1S'
    New-Dds (& $tex 'ModF' 'plants\fdxo.dds') 512 512 'BC1O'
    foreach ($i in 1, 2, 3) {
        New-Dds (& $tex 'ModD' "ground\g$i.dds") 2048 2048 'BC7'
        New-Dds (& $tex 'ModD' "ground\g$i`_n.dds") 2048 2048 'BC5'
    }
    New-Dds (& $tex 'ModD' 'ground\lone.dds') 1024 1024 'BC7'
    New-Dds (& $tex 'ModD' 'ground\big.dds') 4096 4096 'BC7'
    New-Dds (& $tex 'ModD' 'ground\bigx_n.dds') 4096 4096 'BC5'
    # Matched pairs outside 1K-2K that normal-4k cannot take first: both 512, and a 4096 colour with a 2048 normal.
    New-Dds (& $tex 'ModD' 'ground\s.dds') 512 512 'BC7'
    New-Dds (& $tex 'ModD' 'ground\s_n.dds') 512 512 'BC5'
    New-Dds (& $tex 'ModD' 'ground\t.dds') 4096 4096 'BC7'
    New-Dds (& $tex 'ModD' 'ground\t_n.dds') 2048 2048 'BC5'
    New-Dds (& $tex 'ModD' 'ground\odd.dds') 512 512 'D3D113'

    $before = @{}
    foreach ($f in Get-ChildItem -LiteralPath $mods -Recurse -File) { $before[$f.FullName] = (Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash }
    $standIn = @($prot) + @(Get-ProtectedList)

    $results = New-Object System.Collections.Generic.List[object]
    function Case([string]$name, [scriptblock]$test) {
        try { $ok = [bool](& $test); $err = '' } catch { $ok = $false; $err = $_.Exception.Message }
        $results.Add([pscustomobject]@{ Name = $name; Ok = $ok; Err = $err })
        '{0} {1}{2}' -f ($(if ($ok) { 'PASS' } else { 'FAIL' })), $name, ($(if ($err) { "  ($err)" } else { '' }))
    }
    function Try-Build([string]$dest, [int]$seed = 1) {
        try { Invoke-Build $mods $dest $seed 15 $standIn } catch { $_.Exception.Message }
    }
    function Rel([string]$p) { $p.Substring($mods.Length + 1) }

    $out = Join-Path $t 'out'
    $r1 = Try-Build (Join-Path $out 'c1') 1
    $built = $r1 -isnot [string]
    Case 'a build into a new folder outside every protected path succeeds' { $built }
    if (-not $built) { Write-Host "  build said: $r1" }
    $by = @{}
    if ($built) { foreach ($row in $r1.Rows) { if (-not $by.ContainsKey($row.Category)) { $by[$row.Category] = @() }; $by[$row.Category] += (Rel $row.Source) } }
    function Names([string]$cat) {
        $a = [string[]]@($by[$cat] | ForEach-Object { [IO.Path]::GetFileName($_) })
        [Array]::Sort($a, [StringComparer]::Ordinal)
        $a -join ','
    }

    Case 'PBR sets: exactly the two matched sets (6 files), per base' { $by['pbr-set'].Count -eq 6 -and (Names 'pbr-set') -eq 'a.dds,a_n.dds,a_rmaos.dds,b.dds,b_n.dds,b_rmaos.dds' }
    Case 'the decoy folder (x_n, y_rmaos, z of different bases) is not a PBR set' { -not (@($by.Values | ForEach-Object { $_ }) -match 'decoy') }
    Case '4K normals: 40 of the 46 at 4096 (small ones and set members never)' {
        $by['normal-4k'].Count -eq 40 -and -not (@($by['normal-4k']) -match 'small') -and -not (@($by['normal-4k']) -match 'pbr')
    }
    Case 'matched pairs outside 1K-2K (s 512, t 4096 + 2048) are candidates but neither member is picked as a pair' {
        $r1.Cov['pair-1k-2k'].Candidates -eq 5 -and -not (@($by['pair-1k-2k']) -match '\\(s|t|big)(_n)?\.dds$')
    }
    Case 'a numeric D3DFMT code in the FourCC field reads as D3DFMT113, not control bytes' { (Read-DdsHeader (& $tex 'ModD' 'ground\odd.dds')).Format -eq 'D3DFMT113' }
    Case 'pairs: the three matched 2K pairs (6 files), not lone.dds' { $by['pair-1k-2k'].Count -eq 6 -and (Names 'pair-1k-2k') -eq 'g1.dds,g1_n.dds,g2.dds,g2_n.dds,g3.dds,g3_n.dds' }
    Case 'foliage: BC7, BC3, BC1 with alpha bits and DX10 BC1 with straight alpha; not DXT1 without alpha, not DX10 opaque' {
        (Names 'foliage') -eq 'fbc1a.dds,fbc3.dds,fbc7.dds,fdx.dds'
    }
    Case 'icons: 60 of the 65 at 128 px, none of the 512 px ones' { $by['icon'].Count -eq 60 -and -not (@($by['icon']) -match 'big') }
    Case 'raw packing maps: the roughness, metallic and AO sources' { (Names 'raw-map') -eq 'rock_ao.tif,rock_metallic.tga,rock_roughness.png' }
    Case 'source images: 100 of the 120, no raw map among them or among their candidates (120 vs 3 raw)' {
        $by['source-image'].Count -eq 100 -and -not (@($by['source-image']) -match 'rock_') -and $r1.Cov['source-image'].Candidates -eq 120 -and $r1.Cov['raw-map'].Candidates -eq 3
    }
    Case 'BC7 and BC3 count as alpha formats, DXT1 without alpha bits does not, DXT1 with them does' {
        (Read-DdsHeader (& $tex 'ModF' 'landscape\fbc7.dds')).Alpha -and (Read-DdsHeader (& $tex 'ModF' 'landscape\fbc3.dds')).Alpha -and
        -not (Read-DdsHeader (& $tex 'ModF' 'landscape\fbc1.dds')).Alpha -and (Read-DdsHeader (& $tex 'ModF' 'landscape\fbc1a.dds')).Alpha
    }
    Case 'the format and size come from the header bytes (DX10 BC5 at 2048)' {
        $h = Read-DdsHeader (& $tex 'ModD' 'ground\g1_n.dds'); $h.Format -eq 'BC5' -and $h.Width -eq 2048 -and $h.Height -eq 2048
    }
    Case 'sources.csv and coverage.md exist and list every copied file' {
        $csv = @(Import-Csv -LiteralPath (Join-Path $r1.Dest 'sources.csv'))
        $csv.Count -eq $r1.Rows.Count -and (Test-Path -LiteralPath (Join-Path $r1.Dest 'coverage.md')) -and
        -not @($csv | Where-Object { -not (Test-Path -LiteralPath $_.destination) })
    }
    Case 'each copy keeps its path below its mod folder' { Test-Path -LiteralPath (Join-Path $r1.Dest 'ModP\textures\pbr\a_n.dds') }

    $r2 = Try-Build (Join-Path $out 'c2') 1
    $r3 = Try-Build (Join-Path $out 'c3') 2
    Case 'the same seed gives the same picks' { ($r1.Rows | ForEach-Object { Rel $_.Source } | Sort-Object) -join '|' -eq (($r2.Rows | ForEach-Object { Rel $_.Source } | Sort-Object) -join '|') }
    Case 'another seed gives other picks (45 normals, 40 picked)' { ($r1.Rows | ForEach-Object { Rel $_.Source } | Sort-Object) -join '|' -ne (($r3.Rows | ForEach-Object { Rel $_.Source } | Sort-Object) -join '|') }

    $csvBefore = (Get-FileHash -LiteralPath (Join-Path $r1.Dest 'sources.csv') -Algorithm SHA256).Hash
    $ex = Try-Build (Join-Path $out 'c1') 1
    Case 'an existing -Dest is refused and its contents are untouched' {
        $ex -is [string] -and $ex -like '*already exists*' -and (Get-FileHash -LiteralPath (Join-Path $r1.Dest 'sources.csv') -Algorithm SHA256).Hash -eq $csvBefore
    }
    $inProt = Try-Build (Join-Path $prot 'corpus') 1
    Case 'a -Dest inside the stand-in protected tree is refused and nothing is created there' {
        $inProt -is [string] -and $inProt -like '*protected*' -and -not (Test-Path -LiteralPath (Join-Path $prot 'corpus'))
    }
    $inMods = Try-Build (Join-Path $mods 'corpus') 1
    Case 'a -Dest inside -Mods is refused and nothing is created there' {
        $inMods -is [string] -and $inMods -like '*-Mods*' -and -not (Test-Path -LiteralPath (Join-Path $mods 'corpus'))
    }
    $sib = Try-Build (Join-Path $t 'mods2') 1
    Case 'a -Dest that only shares a prefix with -Mods (mods2 beside mods) is allowed' { $sib -isnot [string] -and (Test-Path -LiteralPath (Join-Path $t 'mods2\sources.csv')) }
    $viaLink = Try-Build (Join-Path $t 'link\corpus') 1
    Case 'a -Dest reached through a junction into the protected tree is refused and nothing is created' {
        $viaLink -is [string] -and $viaLink -like '*protected*' -and -not (Test-Path -LiteralPath (Join-Path $prot 'corpus'))
    }
    Case 'the sentinel in the stand-in protected tree is unchanged and holds nothing else' {
        (Get-FileHash -LiteralPath (Join-Path $prot 'keep.txt') -Algorithm SHA256).Hash -eq $sentinel -and @(Get-ChildItem -LiteralPath $prot -Force).Count -eq 1
    }
    Case 'every source file is unchanged (SHA-256) after all builds' {
        $same = $true
        foreach ($k in $before.Keys) { if ((Get-FileHash -LiteralPath $k -Algorithm SHA256).Hash -ne $before[$k]) { $same = $false } }
        $same -and @(Get-ChildItem -LiteralPath $mods -Recurse -File).Count -eq $before.Count
    }
    Case 'the mixed-corpus exemption (stand-in): a new destination there is allowed, inside an existing one it is refused, without the exemption it is refused' {
        $mx = Join-Path $t 'mixedstand'
        $list = @($mx) + @(Get-ProtectedList)
        $new = -not (Get-DestProblem $mx $mods $list $mx)
        $no = [bool](Get-DestProblem $mx $mods $list '')
        New-Item -ItemType Directory -Path $mx | Out-Null
        $inside = (Get-DestProblem (Join-Path $mx 'sub') $mods $list $mx) -like '*existing mixed corpus*'
        $new -and $no -and $inside -and (Get-ChildItem -LiteralPath $mx -Force).Count -eq 0
    }
    Case 'the real mixed-corpus entry comes from the shared protected list' { $MixedCorpus -and (Get-ProtectedList) -contains $MixedCorpus -and $MixedCorpus -like '*mixed-corpus' }
    Case 'the real protected list refuses its own folders as -Dest (string check, nothing created)' {
        (Get-DestProblem 'G:\Acherus\corpus-never' $mods @(Get-ProtectedList)) -and (Get-DestProblem 'B:\testdata\x-never' $mods @(Get-ProtectedList))
    }
    $bad = @($results | Where-Object { -not $_.Ok }).Count
    "self-test folder (left in place): $t"
    "$($results.Count - $bad) passed, $bad failed"
    if ($bad) { exit 1 }
}

# ---------------------------------------------------------------- main

if ($SelfTest) { Invoke-SelfTest; exit 0 }
if (-not $Mods -or -not $Dest) { throw 'usage: profile-corpus.ps1 -Mods <folder> -Dest <folder> [-Seed <int>] [-MaxGB <n>] | -SelfTest' }
$res = Invoke-Build $Mods $Dest $Seed $MaxGB @(Get-ProtectedList)
"built $($res.Rows.Count) files into $($res.Dest); see sources.csv and coverage.md there"

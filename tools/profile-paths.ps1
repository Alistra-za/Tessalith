# Shared protected-path helpers (spec 7.8), dot-sourced by profile-run.ps1 and profile-purge.ps1.
# Not run on its own. Nothing here writes, deletes or moves anything.

function Split-Components([string]$p) {
    $full = [IO.Path]::GetFullPath($p)
    , @($full.Split([char[]]@('\', '/'), [StringSplitOptions]::RemoveEmptyEntries))
}

# Resolves junctions and symlinks component by component; the part below the deepest existing component is kept.
# shortcut: mapped and subst drives are not resolved, upgrade when a protected path is reached through one.
function Resolve-Real([string]$p, [int]$depth = 0) {
    $parts = Split-Components $p
    $cur = $parts[0] + '\'
    for ($i = 1; $i -lt $parts.Count; $i++) {
        $next = Join-Path $cur $parts[$i]
        $item = Get-Item -LiteralPath $next -Force -ErrorAction SilentlyContinue
        if ($item -and $item.LinkType -and $depth -lt 8) {
            $target = $item.ResolveLinkTarget($true)
            if ($target) { $next = Resolve-Real $target.FullName ($depth + 1) }
        }
        $cur = $next
    }
    $cur
}

# Whole-component containment of the resolved paths: is $child equal to or inside $parent?
function Test-Contained([string]$child, [string]$parent) {
    $c = Split-Components (Resolve-Real $child)
    $p = Split-Components (Resolve-Real $parent)
    if ($p.Count -gt $c.Count) { return $false }
    for ($i = 0; $i -lt $p.Count; $i++) {
        if ($c[$i] -ine $p[$i]) { return $false }
    }
    $true
}

function Test-Overlap([string]$a, [string]$b) {
    (Test-Contained $a $b) -or (Test-Contained $b $a)
}

function Get-ProtectedList {
    @(
        'B:\testdata', 'G:\', 'C:\_mount',
        'F:\ai\dds\converter',
        'F:\ai\dds\nifparseref',
        'F:\ai\dds\PBR landscape modders resource',
        'F:\ai\dds\complexgrass',
        'F:\ai\dds\bench\profile\mixed-corpus',
        (Join-Path $env:APPDATA 'sdc'),
        (Join-Path $env:LOCALAPPDATA 'sdc')
    )
}

# Returns the reason a RunRoot is refused, or $null.
function Get-RunRootProblem([string]$root, [string[]]$protected) {
    if ($root.StartsWith('\\') -or $root.StartsWith('//')) { return 'is a UNC path (refused)' }
    foreach ($p in $protected) {
        if (Test-Overlap $root $p) { return "overlaps the protected path $p" }
    }
    $comp = Split-Components (Resolve-Real $root)
    if ($comp.Count -ge 4 -and $comp[0] -ieq 'F:' -and $comp[1] -ieq 'ai' -and $comp[2] -ieq 'dds' -and $comp[3] -like 'converter-nif-*') {
        return 'inside a converter-nif-* checkout'
    }
    for ($i = $comp.Count; $i -ge 1; $i--) {
        $dir = ($comp[0..($i - 1)] -join '\') + '\'
        foreach ($name in 'sdc.toml', 'sdc-project.toml') {
            if (Test-Path -LiteralPath (Join-Path $dir $name)) { return "$dir holds $name (an owner project)" }
        }
    }
    $null
}

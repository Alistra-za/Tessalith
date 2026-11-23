<#
Writes the body of RELEASE-NOTES.md for one release: every line after its "## <version>" heading, trimmed.
The release workflow posts it to Nexus Mods and to the draft GitHub release (docs/DOCUMENTATION.md section 6).

  pwsh tools/release-notes.ps1 -Version 3.1.0 -Out notes.md [-Path RELEASE-NOTES.md]

Fails when the first line is not "## <Version>": the file still holds the Unreleased draft or another version's notes.
#>
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Out,
    [string]$Path = 'RELEASE-NOTES.md'
)
$ErrorActionPreference = 'Stop'
$lines = @(Get-Content -LiteralPath $Path -Encoding utf8)
if ($lines.Count -lt 2 -or $lines[0] -ne "## $Version") {
    throw "$Path starts with '$($lines[0])', not '## $Version': the release commit replaces the file with this version's notes (docs/DOCUMENTATION.md section 6)"
}
$body = ($lines[1..($lines.Count - 1)] -join "`n").Trim()
if (-not $body) { throw "$Path has a heading for $Version and no notes under it" }
$target = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Out)
[IO.File]::WriteAllText($target, "$body`n", [Text.UTF8Encoding]::new($false))

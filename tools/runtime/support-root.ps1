# Shared location for preserved sources and dependencies outside the repository.
param()
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..')).TrimEnd('\', '/')
$root = if ($env:MIXJAM_TECHNO_SUPPORT_ROOT) {
    if (-not [IO.Path]::IsPathFullyQualified($env:MIXJAM_TECHNO_SUPPORT_ROOT)) {
        throw 'MIXJAM_TECHNO_SUPPORT_ROOT must be an absolute path outside the repository.'
    }
    [IO.Path]::GetFullPath($env:MIXJAM_TECHNO_SUPPORT_ROOT)
} else {
    Join-Path (Split-Path $workspace -Parent) ((Split-Path $workspace -Leaf) + '-support')
}
if ($root -eq $workspace -or $root.StartsWith($workspace + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The support folder must be outside the repository.'
}
$root

# Required private files belong to this project's local-data directory.
param()
$ErrorActionPreference = 'Stop'
[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\local-data'))

# The private files that this project needs are in its local-data directory.
param()
$ErrorActionPreference = 'Stop'
[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\local-data'))

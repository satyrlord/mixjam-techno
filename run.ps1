param(
    [string]$App = '',
    [string]$Mix = '',
    # Up to 9 library folders.
    [string[]]$Samples = @(),
    [double]$SourceBpm = 140,
    [switch]$Build,
    # Let a local agent operate the window. See "Agent control" in README.md.
    [switch]$Agent,
    [int]$AgentPort = 0
)
$ErrorActionPreference = 'Stop'
$executable = Join-Path $PSScriptRoot 'target\release\mixjam-techno.exe'
Push-Location $PSScriptRoot
try {
    if ($Build -or -not (Test-Path -LiteralPath $executable)) {
        $localDataRoot = & (Join-Path $PSScriptRoot 'tools\runtime\local-data-root.ps1')
        if (Test-Path -LiteralPath (Join-Path $localDataRoot 'toolchain\cargo\bin\cargo.exe')) {
            & (Join-Path $PSScriptRoot 'tools\runtime\cargo.ps1') -CargoArguments @('build','--release','--locked')
        } else {
            & cargo build --release --locked
        }
        if ($LASTEXITCODE -ne 0) { throw 'The Rust build failed.' }
    }
    $arguments = @('gui')
    if ($App) { $arguments += @('--app',$App) }
    foreach ($folder in $Samples) { $arguments += @('--samples',$folder) }
    if ($Samples) { $arguments += @('--source-bpm',$SourceBpm.ToString([Globalization.CultureInfo]::InvariantCulture)) }
    if ($Mix) { $arguments += @('--mix',$Mix) }
    if ($AgentPort) { $arguments += @('--agent-port',$AgentPort) } elseif ($Agent) { $arguments += '--agent' }
    & $executable @arguments
    if ($LASTEXITCODE -ne 0) { throw 'The application reported an error.' }
} finally { Pop-Location }

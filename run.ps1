param(
    [string]$App = '',
    [string]$Mix = '',
    [string]$Samples = '',
    [double]$SourceBpm = 140,
    [switch]$Build
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
    if ($Samples) { $arguments += @('--samples',$Samples,'--source-bpm',$SourceBpm.ToString([Globalization.CultureInfo]::InvariantCulture)) }
    if ($Mix) { $arguments += @('--mix',$Mix) }
    & $executable @arguments
    if ($LASTEXITCODE -ne 0) { throw 'The application reported an error.' }
} finally { Pop-Location }

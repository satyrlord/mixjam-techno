param([Parameter(ValueFromRemainingArguments=$true)][string[]]$CargoArguments)

$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$savedCargo = $env:CARGO_HOME
$savedRustup = $env:RUSTUP_HOME
$savedPath = $env:PATH
try {
    $env:CARGO_HOME = Join-Path $workspace '.toolchain\cargo'
    $env:RUSTUP_HOME = Join-Path $workspace '.toolchain\rustup'
    $env:PATH = (Join-Path $env:CARGO_HOME 'bin') + [IO.Path]::PathSeparator + $env:PATH
    $cargo = Join-Path $env:CARGO_HOME 'bin\cargo.exe'
    if (-not (Test-Path -LiteralPath $cargo)) { throw 'Run tools\runtime\install-rust.ps1 to install the local Rust toolchain.' }
    & $cargo @CargoArguments
    $result = $LASTEXITCODE
} finally {
    $env:CARGO_HOME = $savedCargo
    $env:RUSTUP_HOME = $savedRustup
    $env:PATH = $savedPath
}
exit $result

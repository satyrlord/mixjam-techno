#requires -Version 7.0
# Installs the measurement tools of the ub-mode skill into the local toolchain:
# line coverage and function complexity. Run
# install-rust.ps1 first. The duplication tool, jscpd, is an npm development
# dependency: `npm ci` installs it.
$ErrorActionPreference = 'Stop'
$toolchainRoot = Join-Path (& (Join-Path $PSScriptRoot 'local-data-root.ps1')) 'toolchain'
$rustup = Join-Path $toolchainRoot 'cargo\bin\rustup.exe'
if (-not (Test-Path -LiteralPath $rustup)) { throw 'Run tools\runtime\install-rust.ps1 to install the local Rust toolchain.' }

$savedCargo = $env:CARGO_HOME
$savedRustup = $env:RUSTUP_HOME
try {
    $env:CARGO_HOME = Join-Path $toolchainRoot 'cargo'
    $env:RUSTUP_HOME = Join-Path $toolchainRoot 'rustup'
    # cargo-llvm-cov needs the LLVM tools of the active toolchain.
    & $rustup component add llvm-tools-preview
    if ($LASTEXITCODE -ne 0) { throw 'The LLVM tools component did not install.' }
} finally {
    $env:CARGO_HOME = $savedCargo
    $env:RUSTUP_HOME = $savedRustup
}

$tools = [ordered]@{
    'cargo-llvm-cov' = '0.9.2'
    'rust-code-analysis-cli' = '0.0.25'
}
foreach ($tool in $tools.GetEnumerator()) {
    & (Join-Path $PSScriptRoot 'cargo.ps1') install --locked $tool.Key --version $tool.Value
    if ($LASTEXITCODE -ne 0) { throw "$($tool.Key) $($tool.Value) did not install." }
}

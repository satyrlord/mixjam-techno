#requires -Version 7.0
param([string]$Toolchain = 'stable')

$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$localRoot = Join-Path (& (Join-Path $PSScriptRoot 'support-root.ps1')) 'toolchain'
$evidence = Join-Path $workspace 'evidence\runtime\toolchain'
$null = New-Item -ItemType Directory -Path $localRoot,$evidence -Force
$installer = Join-Path $localRoot 'rustup-init.exe'
$source = 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe'
Invoke-WebRequest -Uri $source -OutFile $installer
$checksumResponse = (Invoke-WebRequest -Uri ($source + '.sha256')).Content
$checksumText = if ($checksumResponse -is [byte[]]) { [Text.Encoding]::ASCII.GetString($checksumResponse) } else { [string]$checksumResponse }
$expected = ($checksumText -split '\s+')[0].ToUpperInvariant()
$actual = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($expected -ne $actual) { throw 'The Rust installer checksum does not match the official checksum.' }
[ordered]@{ source=$source; sha256=$actual; downloaded_utc=[DateTime]::UtcNow.ToString('o'); requested_toolchain=$Toolchain } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'installer.json') -Encoding utf8

$savedCargo = $env:CARGO_HOME
$savedRustup = $env:RUSTUP_HOME
try {
    $env:CARGO_HOME = Join-Path $localRoot 'cargo'
    $env:RUSTUP_HOME = Join-Path $localRoot 'rustup'
    & $installer -y --no-modify-path --profile minimal --component rustfmt --component clippy --default-host x86_64-pc-windows-msvc --default-toolchain $Toolchain 2>&1 | Tee-Object -FilePath (Join-Path $evidence 'install.log')
    if ($LASTEXITCODE -ne 0) { throw 'The local Rust installation failed.' }
    & (Join-Path $env:CARGO_HOME 'bin\rustc.exe') --version --verbose | Tee-Object -FilePath (Join-Path $evidence 'rustc-version.txt')
    if ($LASTEXITCODE -ne 0) { throw 'The installed Rust compiler does not run.' }
    & (Join-Path $env:CARGO_HOME 'bin\cargo.exe') --version --verbose | Tee-Object -FilePath (Join-Path $evidence 'cargo-version.txt')
    if ($LASTEXITCODE -ne 0) { throw 'The installed Cargo does not run.' }
} finally {
    $env:CARGO_HOME = $savedCargo
    $env:RUSTUP_HOME = $savedRustup
}

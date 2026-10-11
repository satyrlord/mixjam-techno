# PowerShell removes a bare `--` from positional arguments. To pass one to Cargo,
# use the named form: cargo.ps1 -CargoArguments @('test','--','--ignored')
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$CargoArguments)

$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$toolchainRoot = Join-Path (& (Join-Path $PSScriptRoot 'local-data-root.ps1')) 'toolchain'

# Cargo prints the package version, which must be semantic. Keep it equal to the
# application version of this checkout: `1.005` is package version `1.5.0`.
# The major comes from Cargo.toml; the minor is the Git commit count, as in build.rs.
$count = & git -C $workspace rev-list --count HEAD 2>$null
if ($LASTEXITCODE -eq 0) {
    $encoding = [Text.UTF8Encoding]::new($false)
    $manifest = Join-Path $workspace 'Cargo.toml'
    $text = [IO.File]::ReadAllText($manifest)
    $current = [regex]::Match($text, '(?m)^version = "(\d+)\.\d+\.\d+"')
    if (-not $current.Success) { throw 'Cargo.toml needs a package version of the form major.minor.patch.' }
    $version = '{0}.{1}.0' -f $current.Groups[1].Value, [int]$count
    $line = 'version = "' + $version + '"'
    if ($current.Value -ne $line) {
        [IO.File]::WriteAllText($manifest, $text.Remove($current.Index, $current.Length).Insert($current.Index, $line), $encoding)
        $lock = Join-Path $workspace 'Cargo.lock'
        $text = [IO.File]::ReadAllText($lock)
        $text = [regex]::Replace($text, '(?<=name = "mixjam-techno"\r?\nversion = ")[^"]+', $version)
        [IO.File]::WriteAllText($lock, $text, $encoding)
    }
}

$savedCargo = $env:CARGO_HOME
$savedRustup = $env:RUSTUP_HOME
$savedPath = $env:PATH
try {
    $env:CARGO_HOME = Join-Path $toolchainRoot 'cargo'
    $env:RUSTUP_HOME = Join-Path $toolchainRoot 'rustup'
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

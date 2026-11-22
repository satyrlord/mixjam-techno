# PowerShell removes a bare `--` from positional arguments. To pass one to Cargo,
# use the named form: cargo.ps1 -CargoArguments @('test','--','--ignored')
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$CargoArguments)

$ErrorActionPreference = 'Stop'

$subcommand = $null
for ($index = 0; $index -lt $CargoArguments.Length; $index++) {
    $arg = $CargoArguments[$index]
    if ($arg -eq '--') { break }
    if ($arg -cin '--config', '--color', '--explain', '-C', '-Z') {
        $index++
        continue
    }
    if ($arg -and $arg -notlike '-*' -and $arg -notlike '+*') {
        $subcommand = $arg
        break
    }
}
if ($subcommand -eq 'llvm-cov' -and $env:MIXJAM_ALLOW_LLVM_COV -ne '1') {
    [Console]::Error.WriteLine(
        "Blocked 'cargo llvm-cov': coverage runs are disabled by default due to runtime cost. Set MIXJAM_ALLOW_LLVM_COV=1 only when the owner explicitly invokes run-quality-gate and asks for a full gate or coverage run."
    )
    exit 1
}

$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$toolchainRoot = Join-Path (& (Join-Path $PSScriptRoot 'local-data-root.ps1')) 'toolchain'

# The package version must be semantic, and Cargo prints it. Keep it equal to the
# application version of this checkout: `1.005` is package version `1.5.0`.
# The major comes from Cargo.toml. The minor is the Git commit count, as in build.rs.
$count = & git -C $workspace rev-list --count HEAD 2>$null
if ($LASTEXITCODE -eq 0) {
    $encoding = [Text.UTF8Encoding]::new($false)
    # Replace each file whole. A concurrent Cargo call then never reads a half-written manifest.
    function Write-Whole([string]$path, [string]$content) {
        $staged = '{0}.{1}.tmp' -f $path, $PID
        try {
            [IO.File]::WriteAllText($staged, $content, $encoding)
            [IO.File]::Replace($staged, $path, [NullString]::Value)
        } finally {
            if (Test-Path -LiteralPath $staged) {
                Remove-Item -LiteralPath $staged -Force -ErrorAction SilentlyContinue
            }
        }
    }
    $manifest = Join-Path $workspace 'Cargo.toml'
    $text = [IO.File]::ReadAllText($manifest)
    $current = [regex]::Match($text, '(?m)^version = "(\d+)\.\d+\.\d+"')
    if (-not $current.Success) { throw 'Cargo.toml needs a package version of the form major.minor.patch.' }
    $version = '{0}.{1}.0' -f $current.Groups[1].Value, [int]$count
    $line = 'version = "' + $version + '"'
    if ($current.Value -ne $line) {
        Write-Whole $manifest ($text.Remove($current.Index, $current.Length).Insert($current.Index, $line))
    }
    # Check the lockfile even if a previous run already replaced the manifest.
    $lock = Join-Path $workspace 'Cargo.lock'
    if ([IO.File]::Exists($lock)) {
        $text = [IO.File]::ReadAllText($lock)
        $updated = [regex]::Replace($text, '(?<=name = "mixjam-techno"\r?\nversion = ")[^"]+', $version)
        if ($updated -cne $text) {
            Write-Whole $lock $updated
        }
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

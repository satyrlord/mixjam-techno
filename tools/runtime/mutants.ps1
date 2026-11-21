#requires -Version 7.0
# Runs cargo-mutants with the local toolchain. cargo-mutants builds each mutant
# in a copy of the tree without .git and the gitignored files, so GIT_DIR
# points build.rs at this checkout for the app version. A persistent target folder keeps the built
# dependencies between runs. The results are in tmp/measure/mutants.out.
# Arguments go to cargo-mutants. PowerShell removes a bare `--`, so pass test
# arguments with the named form:
#   mutants.ps1 -MutantsArguments @('--in-diff','tmp/measure/change.diff','--','--lib')
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$MutantsArguments)

$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$gitDir = & git -C $workspace rev-parse --absolute-git-dir
if ($LASTEXITCODE -ne 0) { throw 'cargo-mutants needs a Git checkout for the app version.' }

$savedGit = $env:GIT_DIR
$savedTarget = $env:CARGO_TARGET_DIR
try {
    $env:GIT_DIR = $gitDir
    $env:CARGO_TARGET_DIR = Join-Path $workspace 'target\mutants'
    # Without --gitignore, the copy takes local-data and tmp too: many gigabytes.
    $arguments = @('mutants', '--gitignore', 'true', '--output', (Join-Path $workspace 'tmp\measure')) + $MutantsArguments
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments $arguments
    $result = $LASTEXITCODE
} finally {
    $env:GIT_DIR = $savedGit
    $env:CARGO_TARGET_DIR = $savedTarget
}
exit $result

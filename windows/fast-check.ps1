[CmdletBinding()]
param(
    [ValidateSet('x86_64-pc-windows-msvc')]
    [string]$Target = 'x86_64-pc-windows-msvc',
    [switch]$Release,
    [switch]$SkipClippy
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$programFilesX86 = ${env:ProgramFiles(x86)}
$cargoBin = if ($env:USERPROFILE) { Join-Path $env:USERPROFILE '.cargo\bin' } else { $null }
$nodeBin = if ($env:ProgramFiles) { Join-Path $env:ProgramFiles 'nodejs' } else { $null }

function Resolve-ToolPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [string]$FallbackPath
    )

    $command = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $command) {
        return $command.Source
    }
    if ($FallbackPath -and (Test-Path -LiteralPath $FallbackPath -PathType Leaf)) {
        return (Resolve-Path -LiteralPath $FallbackPath).Path
    }
    return $null
}

function Import-VcVarsEnvironment {
    $candidates = @(
        (Join-Path $programFilesX86 'Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat'),
        (Join-Path $programFilesX86 'Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat'),
        (Join-Path $programFilesX86 'Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat'),
        (Join-Path $programFilesX86 'Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat')
    )
    $vcVars = $candidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1

    if ($null -eq $vcVars) {
        throw 'vcvars64.bat was not found. Install the Visual C++ Build Tools workload.'
    }

    $environmentOutput = & $env:ComSpec /d /c "call `"$vcVars`" && set"
    if ($LASTEXITCODE -ne 0) {
        throw "Could not import the MSVC environment from '$vcVars'."
    }

    foreach ($line in $environmentOutput) {
        $separator = $line.IndexOf('=')
        if ($separator -gt 0) {
            [Environment]::SetEnvironmentVariable(
                $line.Substring(0, $separator),
                $line.Substring($separator + 1),
                'Process'
            )
        }
    }
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Description,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Action
    )

    Write-Host "==> $Description"
    & $Action
    if ($LASTEXITCODE -ne 0) {
        throw "Check failed: $Description"
    }
}

$cargoFallbackPath = if ($cargoBin) { Join-Path $cargoBin 'cargo.exe' } else { $null }
$npmFallbackPath = if ($nodeBin) { Join-Path $nodeBin 'npm.cmd' } else { $null }
$cargoPath = Resolve-ToolPath -Name 'cargo.exe' -FallbackPath $cargoFallbackPath
$npmPath = Resolve-ToolPath -Name 'npm.cmd' -FallbackPath $npmFallbackPath
if ($null -eq $cargoPath) {
    throw 'cargo was not found. Install Rustup and the stable MSVC toolchain.'
}
if ($null -eq $npmPath) {
    throw 'npm was not found. Install Node.js LTS.'
}

if ($cargoBin) {
    $env:Path = "$cargoBin;$env:Path"
}
Import-VcVarsEnvironment
if ($nodeBin) {
    $env:Path = "$nodeBin;$env:Path"
}
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-msvc'
$env:CARGO_NET_RETRY = '3'
$env:CARGO_HTTP_TIMEOUT = '120'
$env:CARGO_HTTP_MULTIPLEXING = 'false'

Push-Location $repoRoot
try {
    Invoke-Checked -Description 'Rust formatting' -Action {
        & $cargoPath fmt --check
    }
    Invoke-Checked -Description "Rust check ($Target)" -Action {
        & $cargoPath check --locked --target $Target
    }
    Invoke-Checked -Description "Rust tests ($Target)" -Action {
        & $cargoPath test --locked --target $Target
    }
    if (-not $SkipClippy) {
        Invoke-Checked -Description "Rust Clippy ($Target)" -Action {
            & $cargoPath clippy --locked --target $Target --all-targets -- -D warnings
        }
    }
    Invoke-Checked -Description 'Windows-compatible web checks' -Action {
        & $npmPath run check:windows
    }
    if ($Release) {
        Invoke-Checked -Description "Rust release build ($Target)" -Action {
            & $cargoPath build --locked --release --target $Target
        }
    }
} finally {
    Pop-Location
}

Write-Host 'Windows fast checks passed.'

[CmdletBinding()]
param(
    [string]$Version,
    [ValidateSet('Release')]
    [string]$Configuration = 'Release',
    [ValidateSet('x86_64-pc-windows-msvc')]
    [string]$Target = 'x86_64-pc-windows-msvc',
    [switch]$SkipBuild,
    [string]$OutputDirectory,
    [string]$SignCertificatePath,
    [string]$SignCertificatePassword,
    [string]$TimestampUrl = 'http://timestamp.digicert.com'
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$installerScript = Join-Path $PSScriptRoot 'installer.iss'
$binaryName = 'downshift.exe'

function Resolve-ExistingPath {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [AllowEmptyString()]
        [string[]]$Candidates
    )

    foreach ($candidate in $Candidates) {
        if (-not [string]::IsNullOrWhiteSpace($candidate) -and (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    return $null
}

function Resolve-CommandPath {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Names
    )

    foreach ($name in $Names) {
        $command = Get-Command $name -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($null -ne $command) {
            return $command.Source
        }
    }

    return $null
}

function Get-CargoVersion {
    $versionLine = Get-Content -LiteralPath (Join-Path $repoRoot 'Cargo.toml') |
        Where-Object { $_ -match '^\s*version\s*=\s*"([^"]+)"' } |
        Select-Object -First 1

    if ($versionLine -notmatch '^\s*version\s*=\s*"([^"]+)"') {
        throw 'Could not read the package version from Cargo.toml.'
    }

    return $Matches[1]
}

function Get-WindowsProductVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$PackageVersion
    )

    $match = [regex]::Match(
        $PackageVersion,
        '^(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)(?:-(?<label>[0-9A-Za-z-]+)\.(?<preNumber>\d+))?$'
    )
    if (-not $match.Success) {
        throw "Package version '$PackageVersion' cannot be represented as a Windows product version."
    }

    $components = @(
        [int]$match.Groups['major'].Value,
        [int]$match.Groups['minor'].Value,
        [int]$match.Groups['patch'].Value
    )
    if ($match.Groups['preNumber'].Success) {
        $components += [int]$match.Groups['preNumber'].Value
    } else {
        $components += 0
    }

    if ($components | Where-Object { $_ -gt 65535 }) {
        throw "Package version '$PackageVersion' contains a component larger than the Windows version limit."
    }

    return ($components -join '.')
}

function Import-VcVarsEnvironment {
    $programFilesX86 = ${env:ProgramFiles(x86)}
    $candidates = @(
        (Join-Path $programFilesX86 'Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat'),
        (Join-Path $programFilesX86 'Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat'),
        (Join-Path $programFilesX86 'Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat'),
        (Join-Path $programFilesX86 'Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat')
    )
    $vcVars = Resolve-ExistingPath -Candidates $candidates

    if ($null -eq $vcVars) {
        return
    }

    $environmentOutput = & $env:ComSpec /d /c "call `"$vcVars`" && set"
    if ($LASTEXITCODE -ne 0) {
        throw "Could not import the MSVC environment from '$vcVars'."
    }

    foreach ($line in $environmentOutput) {
        $separator = $line.IndexOf('=')
        if ($separator -gt 0) {
            $name = $line.Substring(0, $separator)
            $value = $line.Substring($separator + 1)
            [Environment]::SetEnvironmentVariable($name, $value, 'Process')
        }
    }
}

function Resolve-InnoCompiler {
    $programFiles = $env:ProgramFiles
    $programFilesX86 = ${env:ProgramFiles(x86)}
    $localAppData = $env:LOCALAPPDATA
    $candidates = @()
    $candidates += Resolve-CommandPath -Names @('ISCC.exe', 'iscc')
    if ($env:INNO_SETUP_HOME) {
        $candidates += Join-Path $env:INNO_SETUP_HOME 'ISCC.exe'
    }
    if ($localAppData) {
        $candidates += Join-Path $localAppData 'Programs\Inno Setup 6\ISCC.exe'
    }
    if ($programFiles) {
        $candidates += Join-Path $programFiles 'Inno Setup 6\ISCC.exe'
    }
    if ($programFilesX86) {
        $candidates += Join-Path $programFilesX86 'Inno Setup 6\ISCC.exe'
    }
    $compiler = Resolve-ExistingPath -Candidates $candidates

    if ($null -eq $compiler) {
        throw 'ISCC.exe was not found. Install Inno Setup or set INNO_SETUP_HOME.'
    }

    return $compiler
}

function Resolve-SignTool {
    $commandPath = Resolve-CommandPath -Names @('signtool.exe', 'signtool')
    if ($null -ne $commandPath) {
        return $commandPath
    }

    $programFilesX86 = ${env:ProgramFiles(x86)}
    if ([string]::IsNullOrWhiteSpace($programFilesX86)) {
        return $null
    }

    $kitsRoot = Join-Path $programFilesX86 'Windows Kits\10\bin'
    if (-not (Test-Path -LiteralPath $kitsRoot -PathType Container)) {
        return $null
    }

    $kitCandidates = Get-ChildItem -LiteralPath $kitsRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        ForEach-Object { Join-Path $_.FullName 'x64\signtool.exe' }

    return Resolve-ExistingPath -Candidates $kitCandidates
}

function Sign-AuthenticodeFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$FilePath,
        [Parameter(Mandatory = $true)]
        [string]$SignToolPath,
        [Parameter(Mandatory = $true)]
        [string]$CertificatePath,
        [Parameter(Mandatory = $true)]
        [string]$CertificatePassword,
        [Parameter(Mandatory = $true)]
        [string]$Timestamp
    )

    $signToolArguments = @(
        'sign',
        '/fd', 'SHA256',
        '/f', $CertificatePath,
        '/p', $CertificatePassword,
        '/tr', $Timestamp,
        '/td', 'SHA256',
        $FilePath
    )
    & $SignToolPath @signToolArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Authenticode signing failed for '$FilePath'."
    }
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = Get-CargoVersion
}
$windowsProductVersion = Get-WindowsProductVersion -PackageVersion $Version

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'dist\windows'
} elseif (-not [IO.Path]::IsPathRooted($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot $OutputDirectory
}

$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$stageDirectory = Join-Path $OutputDirectory 'stage'
$binaryPath = Join-Path $repoRoot "target\$Target\$($Configuration.ToLowerInvariant())\$binaryName"
$stagedBinaryPath = Join-Path $stageDirectory $binaryName
$installerPath = Join-Path $OutputDirectory "Downshift-Setup-$Version.exe"
$checksumsPath = Join-Path $OutputDirectory 'SHA256SUMS.txt'

if (-not $SkipBuild) {
    $cargoPath = Resolve-CommandPath -Names @('cargo.exe', 'cargo')
    if ($null -eq $cargoPath -and $env:USERPROFILE) {
        $cargoPath = Resolve-ExistingPath -Candidates @(
            (Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe')
        )
    }
    if ($null -eq $cargoPath) {
        throw 'cargo was not found on PATH.'
    }

    Import-VcVarsEnvironment

    Write-Host "Building $Target ($Configuration)..."
    & $cargoPath build --locked --release --target $Target
    if ($LASTEXITCODE -ne 0) {
        throw 'Cargo release build failed.'
    }
}

if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw "Windows release binary not found at '$binaryPath'. Build it first or omit -SkipBuild."
}

New-Item -ItemType Directory -Force -Path $stageDirectory, $OutputDirectory | Out-Null
Copy-Item -LiteralPath $binaryPath -Destination $stagedBinaryPath -Force

$hasCertificatePath = -not [string]::IsNullOrWhiteSpace($SignCertificatePath)
$hasCertificatePassword = -not [string]::IsNullOrWhiteSpace($SignCertificatePassword)
$signingRequested = $hasCertificatePath -or $hasCertificatePassword
$signToolPath = $null
$resolvedCertificatePath = $null

if ($signingRequested) {
    if (-not $hasCertificatePath -or -not $hasCertificatePassword -or [string]::IsNullOrWhiteSpace($TimestampUrl)) {
        throw 'Signing requires SignCertificatePath, SignCertificatePassword, and TimestampUrl.'
    }
    if (-not (Test-Path -LiteralPath $SignCertificatePath -PathType Leaf)) {
        throw "Signing certificate was not found at '$SignCertificatePath'."
    }

    $resolvedCertificatePath = (Resolve-Path -LiteralPath $SignCertificatePath).Path
    $signToolPath = Resolve-SignTool
    if ($null -eq $signToolPath) {
        throw 'signtool.exe was not found. Install the Windows SDK or add signtool.exe to PATH.'
    }

    Write-Host "Signing staged application with $signToolPath..."
    Sign-AuthenticodeFile -FilePath $stagedBinaryPath -SignToolPath $signToolPath -CertificatePath $resolvedCertificatePath -CertificatePassword $SignCertificatePassword -Timestamp $TimestampUrl
} else {
    Write-Host 'No Windows signing certificate configured; producing an unsigned installer.'
}

$isccPath = Resolve-InnoCompiler
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

Write-Host "Compiling Inno installer for Downshift $Version..."
& $isccPath "/DAppVersion=$Version" "/DAppProductVersion=$windowsProductVersion" "/O$OutputDirectory" $installerScript
if ($LASTEXITCODE -ne 0) {
    throw 'Inno Setup compilation failed.'
}

if (-not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
    throw "Inno Setup did not create the expected installer at '$installerPath'."
}

if ($signingRequested) {
    Write-Host "Signing installer with $signToolPath..."
    Sign-AuthenticodeFile -FilePath $installerPath -SignToolPath $signToolPath -CertificatePath $resolvedCertificatePath -CertificatePassword $SignCertificatePassword -Timestamp $TimestampUrl
}

$hash = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath $checksumsPath -Value "$hash  $([IO.Path]::GetFileName($installerPath))" -Encoding ASCII

$signature = Get-AuthenticodeSignature -LiteralPath $installerPath
Write-Host "Installer signature status: $($signature.Status)"
Write-Host "Installer: $installerPath"
Write-Host "Checksums: $checksumsPath"

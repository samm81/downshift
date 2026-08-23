[CmdletBinding()]
param(
    [string]$VmRoot = '',
    [switch]$SkipBuild,
    [switch]$ProbeRuntime
)

$ErrorActionPreference = 'Stop'

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ([string]::IsNullOrWhiteSpace($VmRoot)) {
    $VmRoot = Join-Path (Split-Path -Parent $repoRoot) 'downshift-vm'
}
$VmRoot = [IO.Path]::GetFullPath($VmRoot)

$stageRoot = Join-Path $VmRoot 'sandbox'
$installerDirectory = Join-Path $repoRoot 'dist\windows'
$binaryPath = Join-Path $repoRoot 'target\x86_64-pc-windows-msvc\release\downshift.exe'
$guestScriptPath = Join-Path $repoRoot 'windows\smoke-vm-guest.ps1'
$wsbPath = Join-Path $VmRoot 'Downshift-WindowsSandbox.generated.wsb'
$webViewRoot = 'C:\Program Files (x86)\Microsoft\EdgeWebView\Application'

if (-not $SkipBuild) {
    Write-Host 'Building the Windows release installer...'
    & powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repoRoot 'windows\build-installer.ps1')
    if ($LASTEXITCODE -ne 0) {
        throw "Windows installer build failed with exit code $LASTEXITCODE."
    }
}

$installer = Get-ChildItem -LiteralPath $installerDirectory -Filter 'Downshift-Setup-*.exe' -File |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if ($null -eq $installer) {
    throw "No Windows installer found under '$installerDirectory'."
}
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw "Windows release binary not found at '$binaryPath'."
}
if (-not (Test-Path -LiteralPath $guestScriptPath -PathType Leaf)) {
    throw "VM guest script not found at '$guestScriptPath'."
}

$webViewRuntime = Get-ChildItem -LiteralPath $webViewRoot -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -match '^\d+\.' } |
    Sort-Object Name -Descending |
    Select-Object -First 1
if ($null -eq $webViewRuntime) {
    throw "No installed WebView2 runtime was found under '$webViewRoot'."
}

New-Item -ItemType Directory -Force -Path (Join-Path $stageRoot 'windows'), (Join-Path $stageRoot 'dist\windows'), (Join-Path $stageRoot 'dist\stage'), (Join-Path $stageRoot 'logs') | Out-Null

Copy-Item -LiteralPath (Join-Path $repoRoot 'windows\smoke-installer.ps1') -Destination (Join-Path $stageRoot 'windows\smoke-installer.ps1') -Force
Copy-Item -LiteralPath (Join-Path $repoRoot 'windows\smoke-ui.ps1') -Destination (Join-Path $stageRoot 'windows\smoke-ui.ps1') -Force
Copy-Item -LiteralPath $guestScriptPath -Destination (Join-Path $stageRoot 'run-sandbox-smoke.ps1') -Force
Copy-Item -LiteralPath $installer.FullName -Destination (Join-Path $stageRoot "dist\windows\$($installer.Name)") -Force
Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $stageRoot 'dist\stage\downshift.exe') -Force

$runName = 'vm-installer-smoke-' + (Get-Date -Format 'yyyyMMdd-HHmmss')
$hostOutputDirectory = Join-Path $stageRoot "logs\$runName"
$guestOutputDirectory = "logs\$runName"
$probeArgument = if ($ProbeRuntime) { ' -ProbeRuntime' } else { '' }
$guestCommand = "powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File C:\downshift-sandbox\run-sandbox-smoke.ps1 -InstallerName $($installer.Name) -OutputDirectory $guestOutputDirectory$probeArgument"

function ConvertTo-XmlText {
    param([Parameter(Mandatory = $true)][string]$Value)
    return [System.Security.SecurityElement]::Escape($Value)
}

$wsb = @"
<Configuration>
  <VGpu>Enable</VGpu>
  <Networking>Enable</Networking>
  <ClipboardRedirection>Enable</ClipboardRedirection>
  <MemoryInMB>4096</MemoryInMB>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>$(ConvertTo-XmlText $stageRoot)</HostFolder>
      <SandboxFolder>C:\downshift-sandbox</SandboxFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>
    <MappedFolder>
      <HostFolder>$(ConvertTo-XmlText $webViewRuntime.FullName)</HostFolder>
      <SandboxFolder>C:\host-webview\$($webViewRuntime.Name)</SandboxFolder>
      <ReadOnly>true</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>$(ConvertTo-XmlText $guestCommand)</Command>
  </LogonCommand>
</Configuration>
"@
New-Item -ItemType Directory -Force -Path $VmRoot | Out-Null
Set-Content -LiteralPath $wsbPath -Value $wsb -Encoding UTF8

Write-Host 'Launching Windows Sandbox...'
Write-Host "  installer: $($installer.FullName)"
Write-Host "  runtime:   $($webViewRuntime.FullName)"
Write-Host "  results:   $hostOutputDirectory"
$existingSandboxProcessIds = @(Get-Process -Name WindowsSandbox,WindowsSandboxClient -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
$sandboxProcess = Start-Process -FilePath "$env:WINDIR\System32\WindowsSandbox.exe" -ArgumentList ('"' + $wsbPath + '"') -PassThru | Select-Object -First 1

$resultPath = Join-Path $hostOutputDirectory 'vm-exit.txt'
try {
    $deadline = [DateTime]::UtcNow.AddMinutes(15)
    while (-not (Test-Path -LiteralPath $resultPath -PathType Leaf) -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Seconds 2
    }

    if (-not (Test-Path -LiteralPath $resultPath -PathType Leaf)) {
        throw "Windows Sandbox did not produce a result within 15 minutes. Inspect '$hostOutputDirectory' and the Sandbox window."
    }

    $status = (Get-Content -LiteralPath $resultPath -Raw).Trim()
    Write-Host "Windows Sandbox result: $status"
    Write-Host "Screenshots and logs: $hostOutputDirectory"
    if ($status -ne 'passed') {
        throw "Windows Sandbox smoke failed. Inspect '$hostOutputDirectory'."
    }
} finally {
    $newSandboxProcesses = Get-Process -Name WindowsSandbox,WindowsSandboxClient -ErrorAction SilentlyContinue |
        Where-Object { $existingSandboxProcessIds -notcontains $_.Id }
    foreach ($process in $newSandboxProcesses) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
}

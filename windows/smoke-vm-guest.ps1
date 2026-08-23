[CmdletBinding()]
param(
    [string]$InstallerName = '',
    [string]$OutputDirectory = 'logs\vm-installer-smoke',
    [switch]$ProbeRuntime
)

$ErrorActionPreference = 'Stop'

$root = 'C:\downshift-sandbox'
if ([IO.Path]::IsPathRooted($OutputDirectory)) {
    $output = $OutputDirectory
} else {
    $output = Join-Path $root $OutputDirectory
}
$resultPath = Join-Path $output 'vm-exit.txt'
$registryLogPath = Join-Path $output 'webview2-registry.log'
$webView2ClientId = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'

New-Item -ItemType Directory -Force -Path $output | Out-Null

try {
    if ([string]::IsNullOrWhiteSpace($InstallerName)) {
        $installer = Get-ChildItem -LiteralPath (Join-Path $root 'dist\windows') -Filter 'Downshift-Setup-*.exe' -File |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($null -eq $installer) {
            throw 'No staged Windows installer was found.'
        }
        $InstallerName = $installer.Name
    }
    $installerPath = Join-Path $root "dist\windows\$InstallerName"
    if (-not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
        throw "Staged installer not found at '$installerPath'."
    }

    $runtimeHostRoot = 'C:\host-webview'
    $runtimeSource = Get-ChildItem -LiteralPath $runtimeHostRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        Select-Object -First 1
    if ($null -eq $runtimeSource) {
        throw "No mapped WebView2 runtime was found under '$runtimeHostRoot'."
    }
    $webView2Version = $runtimeSource.Name
    $runtimeRoot = "C:\Program Files (x86)\Microsoft\EdgeWebView\Application\$webView2Version"

    & reg.exe add "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$webView2ClientId" /v pv /t REG_SZ /d $webView2Version /f | Out-File -LiteralPath $registryLogPath -Encoding utf8
    & reg.exe add "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$webView2ClientId" /v name /t REG_SZ /d 'Microsoft Edge WebView2 Runtime' /f | Out-File -LiteralPath $registryLogPath -Append -Encoding utf8
    & reg.exe add "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$webView2ClientId" /v location /t REG_SZ /d 'C:\Program Files (x86)\Microsoft\EdgeWebView\Application' /f | Out-File -LiteralPath $registryLogPath -Append -Encoding utf8
    & reg.exe add "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\ClientState\$webView2ClientId" /v pv /t REG_SZ /d $webView2Version /f | Out-File -LiteralPath $registryLogPath -Append -Encoding utf8
    & reg.exe add "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\ClientState\$webView2ClientId" /v EBWebView /t REG_SZ /d $runtimeRoot /f | Out-File -LiteralPath $registryLogPath -Append -Encoding utf8

    if (-not (Test-Path -LiteralPath (Join-Path $runtimeRoot 'msedgewebview2.exe') -PathType Leaf)) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $runtimeRoot) | Out-Null
        Copy-Item -LiteralPath $runtimeSource.FullName -Destination $runtimeRoot -Recurse -Force
    }
    @(
        "runtime_root_exists=$([bool](Test-Path -LiteralPath $runtimeRoot -PathType Container))",
        "runtime_exe_exists=$([bool](Test-Path -LiteralPath (Join-Path $runtimeRoot 'msedgewebview2.exe') -PathType Leaf))",
        "runtime_x64_dll_exists=$([bool](Test-Path -LiteralPath (Join-Path $runtimeRoot 'EBWebView\x64\EmbeddedBrowserWebView.dll') -PathType Leaf))",
        "registry_query=" + ((& reg.exe query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$webView2ClientId" /v pv 2>&1) -join ' ')
    ) | Set-Content -LiteralPath (Join-Path $output 'webview2-runtime.log') -Encoding utf8

    if ($ProbeRuntime) {
        $probeBinary = Join-Path $root 'dist\stage\downshift.exe'
        $probeLogs = Join-Path $output 'probe-logs'
        New-Item -ItemType Directory -Force -Path $probeLogs | Out-Null
    }
    if ($ProbeRuntime -and (Test-Path -LiteralPath $probeBinary -PathType Leaf)) {
        $probePsi = New-Object System.Diagnostics.ProcessStartInfo
        $probePsi.FileName = $probeBinary
        $probePsi.WorkingDirectory = Split-Path -Parent $probeBinary
        $probePsi.UseShellExecute = $false
        $probePsi.CreateNoWindow = $false
        $probePsi.EnvironmentVariables['APPDATA'] = $probeLogs
        $probePsi.EnvironmentVariables['LOCALAPPDATA'] = $probeLogs
        $probePsi.EnvironmentVariables['DOWNSHIFT_LOG_DIR'] = (Join-Path $probeLogs 'app-logs')
        $probePsi.EnvironmentVariables['DOWNSHIFT_TELEMETRY_DIR'] = (Join-Path $probeLogs 'telemetry')
        $probePsi.EnvironmentVariables['DOWNSHIFT_TELEMETRY_ENABLED'] = 'false'
        $probePsi.EnvironmentVariables['DOWNSHIFT_ENV'] = 'smoke'
        $probe = [System.Diagnostics.Process]::Start($probePsi)
        Start-Sleep -Seconds 5
        $probe.Refresh()
        if ($probe.HasExited) {
            Add-Content -LiteralPath (Join-Path $output 'webview2-runtime.log') -Value "probe_exit_code=$($probe.ExitCode)"
        } else {
            Add-Content -LiteralPath (Join-Path $output 'webview2-runtime.log') -Value "probe_running_pid=$($probe.Id)"
            $children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $($probe.Id)" -ErrorAction SilentlyContinue
            $children | ForEach-Object {
                Add-Content -LiteralPath (Join-Path $output 'webview2-runtime.log') -Value "probe_child=$($_.Name):$($_.ProcessId)"
            }
            $probe.Kill()
            $probe.WaitForExit(5000)
        }
    }

    & (Join-Path $root 'windows\smoke-installer.ps1') -InstallerPath $installerPath -OutputDirectory $output
    if ($LASTEXITCODE -ne 0) {
        throw "Installer smoke exited with code $LASTEXITCODE."
    }
    Set-Content -LiteralPath $resultPath -Value 'passed' -Encoding ASCII
} catch {
    $_ | Out-File -LiteralPath (Join-Path $output 'vm-error.txt') -Encoding utf8
    Set-Content -LiteralPath $resultPath -Value 'failed' -Encoding ASCII
}

Start-Sleep -Seconds 30

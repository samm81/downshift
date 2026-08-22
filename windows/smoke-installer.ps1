[CmdletBinding()]
param(
    [string]$InstallerPath,
    [string]$OutputDirectory,
    [switch]$SkipInteractive
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$defaultInstallerPath = Join-Path $repoRoot 'dist\windows\Downshift-Setup-0.1.28.exe'
if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
    $InstallerPath = $defaultInstallerPath
} elseif (-not [IO.Path]::IsPathRooted($InstallerPath)) {
    $InstallerPath = Join-Path $repoRoot $InstallerPath
}
$InstallerPath = (Resolve-Path -LiteralPath $InstallerPath).Path

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $stamp = [DateTime]::Now.ToString('yyyyMMdd-HHmmss')
    $OutputDirectory = Join-Path $repoRoot "logs\installer-smoke-windows-$stamp"
} elseif (-not [IO.Path]::IsPathRooted($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot $OutputDirectory
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

$runLogPath = Join-Path $OutputDirectory 'run.log'
$installRoot = Join-Path ([IO.Path]::GetTempPath()) ('downshift-installer-smoke-' + [Guid]::NewGuid().ToString('N'))
$installDirectory = Join-Path $installRoot 'Downshift'
$silentInstallDirectory = Join-Path $installRoot 'Silent-Downshift'
$installLogPath = Join-Path $OutputDirectory 'installer.log'
$silentInstallLogPath = Join-Path $OutputDirectory 'installer-silent.log'
$guiSmokeScript = Join-Path $PSScriptRoot 'smoke-ui.ps1'
New-Item -ItemType Directory -Force -Path $installRoot | Out-Null

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class DownshiftInstallerSmokeNative
{
    private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("user32.dll")]
    private static extern bool EnumChildWindows(IntPtr parent, EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetClassName(IntPtr hWnd, StringBuilder className, int maxCount);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int maxCount);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool IsWindowEnabled(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll")]
    private static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extraInfo);

    [DllImport("user32.dll")]
    private static extern bool PostMessage(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);

    private const uint MouseLeftDown = 0x0002;
    private const uint MouseLeftUp = 0x0004;
    private const uint WmClose = 0x0010;

    private static string ReadWindowText(IntPtr hWnd)
    {
        var text = new StringBuilder(512);
        GetWindowText(hWnd, text, text.Capacity);
        return text.ToString();
    }

    private static string ReadClassName(IntPtr hWnd)
    {
        var className = new StringBuilder(256);
        GetClassName(hWnd, className, className.Capacity);
        return className.ToString();
    }

    public static IntPtr[] FindVisibleWindowsByClass(string className)
    {
        var windows = new List<IntPtr>();
        EnumWindows((hWnd, _) =>
        {
            if (IsWindowVisible(hWnd) && ReadClassName(hWnd) == className)
            {
                windows.Add(hWnd);
            }
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }

    public static IntPtr FindVisibleChildByText(IntPtr parent, string needle)
    {
        IntPtr found = IntPtr.Zero;
        EnumChildWindows(parent, (hWnd, _) =>
        {
            if (IsWindowVisible(hWnd) && IsWindowEnabled(hWnd) && ReadClassName(hWnd).IndexOf("Button", StringComparison.OrdinalIgnoreCase) >= 0 && ReadWindowText(hWnd).IndexOf(needle, StringComparison.OrdinalIgnoreCase) >= 0)
            {
                found = hWnd;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static int[] GetRectValues(IntPtr hWnd)
    {
        RECT rect;
        if (!GetWindowRect(hWnd, out rect))
        {
            return new int[0];
        }
        return new[] { rect.Left, rect.Top, rect.Right, rect.Bottom };
    }

    public static bool FocusWindow(IntPtr hWnd)
    {
        return SetForegroundWindow(hWnd);
    }

    public static void LeftClick(int x, int y)
    {
        SetCursorPos(x, y);
        mouse_event(MouseLeftDown, 0, 0, 0, UIntPtr.Zero);
        mouse_event(MouseLeftUp, 0, 0, 0, UIntPtr.Zero);
    }

    public static void CloseWindow(IntPtr hWnd)
    {
        PostMessage(hWnd, WmClose, IntPtr.Zero, IntPtr.Zero);
    }
}
'@
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

function Log-Message {
    param([Parameter(Mandatory = $true)][string]$Message)
    $line = "{0} {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff zzz'), $Message
    Add-Content -LiteralPath $runLogPath -Value $line
    Write-Host $line
}

function Get-WindowRectObject {
    param([Parameter(Mandatory = $true)][IntPtr]$Handle)
    $values = [DownshiftInstallerSmokeNative]::GetRectValues($Handle)
    if ($values.Count -ne 4) {
        return $null
    }
    return [PSCustomObject]@{
        Left = $values[0]
        Top = $values[1]
        Right = $values[2]
        Bottom = $values[3]
        Width = $values[2] - $values[0]
        Height = $values[3] - $values[1]
    }
}

function Save-Screen {
    param([Parameter(Mandatory = $true)][string]$Path)
    $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen(
            [System.Drawing.Point]::new($bounds.Left, $bounds.Top),
            [System.Drawing.Point]::Empty,
            $bounds.Size
        )
        $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

function Wait-WizardWindow {
    param([int]$TimeoutSeconds = 20)
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $handles = @([DownshiftInstallerSmokeNative]::FindVisibleWindowsByClass('TWizardForm'))
        if ($handles.Count -gt 0) {
            return $handles[0]
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for the Inno Setup wizard window.'
}

function Click-WizardButton {
    param(
        [Parameter(Mandatory = $true)][IntPtr]$WizardHandle,
        [Parameter(Mandatory = $true)][string]$ButtonText
    )
    $button = [DownshiftInstallerSmokeNative]::FindVisibleChildByText($WizardHandle, $ButtonText)
    if ($button -eq [IntPtr]::Zero) {
        throw "Could not find the Inno Setup button containing '$ButtonText'."
    }
    $rect = Get-WindowRectObject $button
    [DownshiftInstallerSmokeNative]::FocusWindow($WizardHandle) | Out-Null
    [DownshiftInstallerSmokeNative]::LeftClick(
        [int][Math]::Round(($rect.Left + $rect.Right) / 2),
        [int][Math]::Round(($rect.Top + $rect.Bottom) / 2)
    )
    Start-Sleep -Milliseconds 600
    Log-Message "clicked installer button '$ButtonText'"
}

function Invoke-InteractiveInstall {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $InstallerPath
    $psi.Arguments = "/DIR=`"$installDirectory`" /LOG=`"$installLogPath`""
    $psi.UseShellExecute = $true
    $wizardProcess = [System.Diagnostics.Process]::Start($psi)
    try {
        $wizardHandle = Wait-WizardWindow
        Save-Screen (Join-Path $OutputDirectory 'wizard-01-welcome.png')
        Log-Message 'captured wizard welcome page'

        $installedBinary = Join-Path $installDirectory 'downshift.exe'
        $installClicked = $false
        for ($step = 2; $step -le 40; $step++) {
            if (-not $installClicked) {
                $installButton = [DownshiftInstallerSmokeNative]::FindVisibleChildByText($wizardHandle, 'Install')
                if ($installButton -ne [IntPtr]::Zero) {
                    Save-Screen (Join-Path $OutputDirectory "wizard-$('{0:D2}' -f $step)-ready.png")
                    Click-WizardButton -WizardHandle $wizardHandle -ButtonText 'Install'
                    $installClicked = $true
                    continue
                }

                $nextButton = [DownshiftInstallerSmokeNative]::FindVisibleChildByText($wizardHandle, 'Next')
                if ($nextButton -eq [IntPtr]::Zero) {
                    throw 'The Inno Setup wizard exposed neither Next nor Install.'
                }
                Save-Screen (Join-Path $OutputDirectory "wizard-$('{0:D2}' -f $step)-next.png")
                Click-WizardButton -WizardHandle $wizardHandle -ButtonText 'Next'
                continue
            }

            if (-not (Test-Path -LiteralPath $installedBinary -PathType Leaf)) {
                Save-Screen (Join-Path $OutputDirectory "wizard-$('{0:D2}' -f $step)-installing.png")
                Start-Sleep -Milliseconds 500
                continue
            }

            $finishButton = [DownshiftInstallerSmokeNative]::FindVisibleChildByText($wizardHandle, 'Finish')
            if ($finishButton -ne [IntPtr]::Zero) {
                Save-Screen (Join-Path $OutputDirectory "wizard-$('{0:D2}' -f $step)-finished.png")
                Click-WizardButton -WizardHandle $wizardHandle -ButtonText 'Finish'
                break
            }
            Start-Sleep -Milliseconds 500
        }

        $installDeadline = [DateTime]::UtcNow.AddSeconds(10)
        while (-not (Test-Path -LiteralPath $installedBinary -PathType Leaf) -and [DateTime]::UtcNow -lt $installDeadline) {
            Start-Sleep -Milliseconds 200
        }
        if (-not (Test-Path -LiteralPath $installedBinary -PathType Leaf)) {
            throw 'Interactive Inno installation did not create downshift.exe.'
        }
        Log-Message 'interactive Inno installation passed'
    } finally {
        if ($null -ne $wizardProcess) {
            $wizardProcess.Refresh()
            if (-not $wizardProcess.HasExited) {
                $wizardProcess.CloseMainWindow()
                if (-not $wizardProcess.WaitForExit(3000)) {
                    $wizardProcess.Kill()
                }
            }
        }
    }
}

function Invoke-SilentInstall {
    param(
        [Parameter(Mandatory = $true)][string]$TargetDirectory,
        [Parameter(Mandatory = $true)][string]$LogPath
    )
    $arguments = @(
        '/VERYSILENT',
        '/SUPPRESSMSGBOXES',
        '/NORESTART',
        '/LANG=english',
        "/DIR=$TargetDirectory",
        "/LOG=$LogPath"
    )
    $process = Start-Process -FilePath $InstallerPath -ArgumentList $arguments -PassThru -Wait
    if ($process.ExitCode -ne 0) {
        throw "Silent Inno installation exited with code $($process.ExitCode)."
    }
    if (-not (Test-Path -LiteralPath (Join-Path $TargetDirectory 'downshift.exe') -PathType Leaf)) {
        throw "Silent Inno installation did not create '$TargetDirectory\downshift.exe'."
    }
    Log-Message "silent Inno installation passed at $TargetDirectory"
}

function Get-UninstallEntry {
    param([Parameter(Mandatory = $true)][string]$TargetDirectory)
    $roots = @(
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
        'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
        'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall'
    )
    foreach ($root in $roots) {
        if (-not (Test-Path -LiteralPath $root)) {
            continue
        }
        foreach ($key in Get-ChildItem -LiteralPath $root -ErrorAction SilentlyContinue) {
            $properties = Get-ItemProperty -LiteralPath $key.PSPath -ErrorAction SilentlyContinue
            if ($properties.DisplayName -eq 'Downshift' -and $properties.InstallLocation -eq "$TargetDirectory\") {
                return $properties
            }
        }
    }
    return $null
}

function Find-StartMenuShortcut {
    $programsDirectory = [Environment]::GetFolderPath('Programs')
    if (-not (Test-Path -LiteralPath $programsDirectory -PathType Container)) {
        return $null
    }
    return Get-ChildItem -LiteralPath $programsDirectory -Filter 'Downshift.lnk' -File -Recurse -ErrorAction SilentlyContinue |
        Select-Object -First 1
}

function Invoke-SilentUninstall {
    param([Parameter(Mandatory = $true)][string]$TargetDirectory)
    $uninstaller = Join-Path $TargetDirectory 'unins000.exe'
    if (-not (Test-Path -LiteralPath $uninstaller -PathType Leaf)) {
        throw "Uninstaller not found at '$uninstaller'."
    }
    $process = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -PassThru -Wait
    if ($process.ExitCode -ne 0) {
        throw "Silent Inno uninstall exited with code $($process.ExitCode)."
    }

    # Inno can finish the uninstaller process before its final directory
    # cleanup has become visible to the filesystem. Give that cleanup a
    # bounded grace period, while stopping only a Downshift process installed
    # in this test directory if one was left behind by the GUI smoke.
    $cleanupDeadline = [DateTime]::UtcNow.AddSeconds(15)
    while ((Test-Path -LiteralPath $TargetDirectory) -and [DateTime]::UtcNow -lt $cleanupDeadline) {
        Stop-ProcessAtPath -Path (Join-Path $TargetDirectory 'downshift.exe')
        Start-Sleep -Milliseconds 250
    }
    if (Test-Path -LiteralPath $TargetDirectory) {
        throw "Silent Inno uninstall left files at '$TargetDirectory'."
    }
    if (Get-UninstallEntry -TargetDirectory $TargetDirectory) {
        throw 'Silent Inno uninstall left an Add/Remove Programs entry.'
    }
    Log-Message "silent Inno uninstall passed for $TargetDirectory"
}

function Stop-ProcessAtPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    $processes = Get-Process -Name downshift -ErrorAction SilentlyContinue |
        Where-Object {
            try { $_.Path -eq $Path } catch { $false }
        }
    foreach ($process in $processes) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
}

function Stop-InstallerProcessesForDirectory {
    param([Parameter(Mandatory = $true)][string]$Directory)
    foreach ($process in Get-CimInstance Win32_Process -ErrorAction SilentlyContinue) {
        if ($process.CommandLine -and $process.CommandLine.Contains($Directory)) {
            Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue
        }
    }
}

if (-not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
    throw "Installer not found at '$InstallerPath'."
}

$originalRunValue = $null
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
try {
    try {
        $originalRunValue = (Get-ItemProperty -LiteralPath $runKey -Name Downshift -ErrorAction Stop).Downshift
    } catch {
        $originalRunValue = $null
    }

    if (-not $SkipInteractive) {
        Invoke-InteractiveInstall
        $installedBinary = Join-Path $installDirectory 'downshift.exe'
        Stop-ProcessAtPath -Path $installedBinary
        $guiOutput = Join-Path $OutputDirectory 'installed-gui-smoke'
        $guiProcess = Start-Process -FilePath 'powershell.exe' -ArgumentList @(
            '-NoProfile',
            '-ExecutionPolicy', 'Bypass',
            '-File', $guiSmokeScript,
            '-BinaryPath', $installedBinary,
            '-OutputDirectory', $guiOutput
        ) -PassThru -Wait
        if ($guiProcess.ExitCode -ne 0) {
            throw "Installed-binary GUI smoke failed with code $($guiProcess.ExitCode)."
        }
        Stop-ProcessAtPath -Path $installedBinary
        Invoke-SilentUninstall -TargetDirectory $installDirectory
    }

    Invoke-SilentInstall -TargetDirectory $silentInstallDirectory -LogPath $silentInstallLogPath
    $shortcut = Find-StartMenuShortcut
    if ($null -eq $shortcut) {
        throw "Start Menu shortcut was not found under '$([Environment]::GetFolderPath('Programs'))'."
    }
    $shortcutPath = $shortcut.FullName
    if ($null -eq (Get-UninstallEntry -TargetDirectory $silentInstallDirectory)) {
        throw 'Add/Remove Programs entry was not created for the silent install.'
    }
    Invoke-SilentUninstall -TargetDirectory $silentInstallDirectory
    if (Test-Path -LiteralPath $shortcutPath -PathType Leaf) {
        throw "Start Menu shortcut was not removed at '$shortcutPath'."
    }

    Set-Content -LiteralPath (Join-Path $OutputDirectory 'result.txt') -Value @(
        'installer=' + $InstallerPath,
        'interactive_install=' + $(if ($SkipInteractive) { 'skipped' } else { 'passed' }),
        'installed_gui_smoke=' + $(if ($SkipInteractive) { 'skipped' } else { 'passed' }),
        'silent_install=passed',
        'silent_uninstall=passed',
        'start_menu_shortcut=passed',
        'uninstall_registry=passed',
        'run_log=' + $runLogPath
    ) -Encoding UTF8
    Log-Message "installer smoke passed; result written to $(Join-Path $OutputDirectory 'result.txt')"
} finally {
    Stop-InstallerProcessesForDirectory -Directory $installRoot
    foreach ($directory in @($installDirectory, $silentInstallDirectory)) {
        if (Test-Path -LiteralPath $directory) {
            Remove-Item -LiteralPath $directory -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    if ($null -ne $originalRunValue) {
        Set-ItemProperty -LiteralPath $runKey -Name Downshift -Value $originalRunValue
    } else {
        Remove-ItemProperty -LiteralPath $runKey -Name Downshift -ErrorAction SilentlyContinue
    }
}

[CmdletBinding()]
param(
    [string]$BinaryPath,
    [string]$OutputDirectory,
    [int]$StartupTimeoutSeconds = 30,
    [switch]$HideAutomationConsole
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$defaultBinaryPath = Join-Path $repoRoot 'target\x86_64-pc-windows-msvc\release\downshift.exe'
if ([string]::IsNullOrWhiteSpace($BinaryPath)) {
    $BinaryPath = $defaultBinaryPath
} elseif (-not [IO.Path]::IsPathRooted($BinaryPath)) {
    $BinaryPath = Join-Path $repoRoot $BinaryPath
}
$BinaryPath = (Resolve-Path -LiteralPath $BinaryPath).Path

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $stamp = [DateTime]::Now.ToString('yyyyMMdd-HHmmss')
    $OutputDirectory = Join-Path $repoRoot "logs\gui-smoke-windows-$stamp"
} elseif (-not [IO.Path]::IsPathRooted($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot $OutputDirectory
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

$runLogPath = Join-Path $OutputDirectory 'run.log'
$appDataPath = Join-Path $OutputDirectory 'appdata'
$telemetryPath = Join-Path $OutputDirectory 'telemetry'
$userStateBackupPath = Join-Path $OutputDirectory 'user-state-backup'
New-Item -ItemType Directory -Force -Path $appDataPath, $telemetryPath, $userStateBackupPath | Out-Null

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class DownshiftSmokeNative
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
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetClassName(IntPtr hWnd, StringBuilder className, int maxCount);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int maxCount);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("kernel32.dll")]
    private static extern IntPtr GetConsoleWindow();

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int command);

    [DllImport("user32.dll")]
    private static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll")]
    private static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extraInfo);

    [DllImport("user32.dll")]
    private static extern void keybd_event(byte virtualKey, byte scanCode, uint flags, UIntPtr extraInfo);

    [DllImport("user32.dll")]
    private static extern bool PostMessage(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);

    private const uint MouseLeftDown = 0x0002;
    private const uint MouseLeftUp = 0x0004;
    private const uint MouseRightDown = 0x0008;
    private const uint MouseRightUp = 0x0010;
    private const uint KeyUp = 0x0002;
    private const uint WmClose = 0x0010;
    private const byte Escape = 0x1B;
    private const int SwHide = 0;

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

    private static IntPtr[] FindWindows(Func<IntPtr, bool> predicate)
    {
        var windows = new List<IntPtr>();
        EnumWindows((hWnd, _) =>
        {
            if (IsWindowVisible(hWnd) && predicate(hWnd))
            {
                windows.Add(hWnd);
            }
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }

    public static IntPtr[] FindVisibleWindowsByClass(string className)
    {
        return FindWindows(hWnd => ReadClassName(hWnd) == className);
    }

    public static IntPtr[] FindVisibleWindowsByTitle(string title)
    {
        return FindWindows(hWnd => ReadWindowText(hWnd) == title);
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

    public static void HideAutomationConsole()
    {
        var console = GetConsoleWindow();
        if (console != IntPtr.Zero)
        {
            ShowWindow(console, SwHide);
        }
    }

    public static void LeftClick(int x, int y)
    {
        SetCursorPos(x, y);
        mouse_event(MouseLeftDown, 0, 0, 0, UIntPtr.Zero);
        mouse_event(MouseLeftUp, 0, 0, 0, UIntPtr.Zero);
    }

    public static void RightClick(int x, int y)
    {
        SetCursorPos(x, y);
        mouse_event(MouseRightDown, 0, 0, 0, UIntPtr.Zero);
        mouse_event(MouseRightUp, 0, 0, 0, UIntPtr.Zero);
    }

    public static void MovePointer(int x, int y)
    {
        SetCursorPos(x, y);
    }

    public static void PressEscape()
    {
        keybd_event(Escape, 0, 0, UIntPtr.Zero);
        keybd_event(Escape, 0, KeyUp, UIntPtr.Zero);
    }

    public static void CloseWindow(IntPtr hWnd)
    {
        PostMessage(hWnd, WmClose, IntPtr.Zero, IntPtr.Zero);
    }
}
'@
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

if ($HideAutomationConsole) {
    [DownshiftSmokeNative]::HideAutomationConsole()
}

function Log-Message {
    param([Parameter(Mandatory = $true)][string]$Message)
    $line = "{0} {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff zzz'), $Message
    Add-Content -LiteralPath $runLogPath -Value $line
    Write-Host $line
}

function Get-WindowRectObject {
    param([Parameter(Mandatory = $true)][IntPtr]$Handle)
    $values = [DownshiftSmokeNative]::GetRectValues($Handle)
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

function Get-PopupWindows {
    return @([DownshiftSmokeNative]::FindVisibleWindowsByClass('#32768'))
}

function Get-MainPopup {
    $popups = Get-PopupWindows
    if ($popups.Count -eq 0) {
        return $null
    }
    $popups |
        ForEach-Object { [PSCustomObject]@{ Handle = $_; Rect = Get-WindowRectObject $_ } } |
        Where-Object { $null -ne $_.Rect } |
        Sort-Object { $_.Rect.Right } -Descending |
        Select-Object -First 1
}

function Wait-MainPopup {
    param([int]$TimeoutMilliseconds = 3000)
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    do {
        $popup = Get-MainPopup
        if ($null -ne $popup) {
            return $popup
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for the native context menu.'
}

function Wait-WindowTitle {
    param(
        [Parameter(Mandatory = $true)][string]$Title,
        [int]$TimeoutMilliseconds = 5000
    )
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    do {
        $handles = @([DownshiftSmokeNative]::FindVisibleWindowsByTitle($Title))
        if ($handles.Count -gt 0) {
            return $handles[0]
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for a '$Title' window."
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

function Save-Window {
    param(
        [Parameter(Mandatory = $true)][IntPtr]$Handle,
        [Parameter(Mandatory = $true)][string]$Path
    )
    $rect = Get-WindowRectObject $Handle
    if ($null -eq $rect -or $rect.Width -le 0 -or $rect.Height -le 0) {
        throw "Could not capture window 0x$($Handle.ToString('X'))."
    }
    $bitmap = New-Object System.Drawing.Bitmap $rect.Width, $rect.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen(
            [System.Drawing.Point]::new($rect.Left, $rect.Top),
            [System.Drawing.Point]::Empty,
            $bitmap.Size
        )
        $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

function Capture-Checkpoint {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [IntPtr]$WindowHandle = [IntPtr]::Zero
    )
    $screenPath = Join-Path $OutputDirectory "$Name-screen.png"
    Save-Screen $screenPath
    if ($WindowHandle -ne [IntPtr]::Zero) {
        $windowPath = Join-Path $OutputDirectory "$Name-window.png"
        Save-Window -Handle $WindowHandle -Path $windowPath
    }
    Log-Message "captured $Name"
}

function Get-MenuPoint {
    param(
        [Parameter(Mandatory = $true)]$Popup,
        [Parameter(Mandatory = $true)][int]$TextIndex
    )
    $rect = $Popup.Rect
    # The current native menu has nine 29px rows, a separator, and a shorter help row at 100% DPI.
    # Scale the measured layout from the popup height so the same script works at common VM DPIs.
    $normalRow = ($rect.Height - 1) / 9.7
    if ($TextIndex -lt 9) {
        $y = $rect.Top + ($normalRow * ($TextIndex + 0.5))
        if ($TextIndex -ge 6) {
            $y += 1
        }
    } else {
        $y = $rect.Top + ($normalRow * 9) + 1 + (($rect.Height - ($normalRow * 9) - 1) / 2)
    }
    return [PSCustomObject]@{
        X = [int][Math]::Round($rect.Left + [Math]::Min(80, $rect.Width / 2))
        Y = [int][Math]::Round($y)
    }
}

function Open-MainMenu {
    param([Parameter(Mandatory = $true)][IntPtr]$WindowHandle)
    $rect = Get-WindowRectObject $WindowHandle
    Log-Message "opening native menu for window 0x$($WindowHandle.ToString('X')) at rect $($rect.Left),$($rect.Top),$($rect.Right),$($rect.Bottom)"
    [DownshiftSmokeNative]::FocusWindow($WindowHandle) | Out-Null
    $x = [int][Math]::Round(($rect.Left + $rect.Right) / 2)
    $y = [int][Math]::Round(($rect.Top + $rect.Bottom) / 2)
    Log-Message "right-clicking at $x,$y"
    [DownshiftSmokeNative]::RightClick($x, $y)
    return Wait-MainPopup
}

function Move-ToMainMenuItem {
    param(
        [Parameter(Mandatory = $true)]$Popup,
        [Parameter(Mandatory = $true)][int]$TextIndex
    )
    $point = Get-MenuPoint -Popup $Popup -TextIndex $TextIndex
    [DownshiftSmokeNative]::MovePointer($point.X, $point.Y)
    Start-Sleep -Milliseconds 350
}

function Click-MainMenuItem {
    param(
        [Parameter(Mandatory = $true)]$Popup,
        [Parameter(Mandatory = $true)][int]$TextIndex
    )
    $point = Get-MenuPoint -Popup $Popup -TextIndex $TextIndex
    [DownshiftSmokeNative]::LeftClick($point.X, $point.Y)
    Start-Sleep -Milliseconds 450
}

function Get-SubmenuPopup {
    param([Parameter(Mandatory = $true)]$MainPopup)
    $popups = Get-PopupWindows |
        ForEach-Object { [PSCustomObject]@{ Handle = $_; Rect = Get-WindowRectObject $_ } } |
        Where-Object {
            $null -ne $_.Rect -and $_.Handle -ne $MainPopup.Handle -and $_.Rect.Left -lt $MainPopup.Rect.Left
        } |
        Sort-Object { $_.Rect.Left }
    return $popups | Select-Object -First 1
}

function Wait-SubmenuPopup {
    param(
        [Parameter(Mandatory = $true)]$MainPopup,
        [int]$TimeoutMilliseconds = 3000
    )
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    do {
        $submenu = Get-SubmenuPopup -MainPopup $MainPopup
        if ($null -ne $submenu) {
            return $submenu
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for the native submenu.'
}

function Click-SubmenuItem {
    param(
        [Parameter(Mandatory = $true)]$Submenu,
        [Parameter(Mandatory = $true)][int]$TextIndex,
        [int]$TextItemCount = 3,
        [int]$SeparatorBeforeIndex = -1,
        [switch]$HasSeparator
    )
    $rowHeight = if ($HasSeparator) {
        $Submenu.Rect.Height / ($TextItemCount + 0.5)
    } else {
        $Submenu.Rect.Height / $TextItemCount
    }
    $y = $Submenu.Rect.Top + ($rowHeight * ($TextIndex + 0.5))
    if ($SeparatorBeforeIndex -ge 0 -and $TextIndex -gt $SeparatorBeforeIndex) {
        $y += $rowHeight * 0.5
    }
    $x = $Submenu.Rect.Left + [Math]::Min(110, $Submenu.Rect.Width / 2)
    [DownshiftSmokeNative]::LeftClick([int][Math]::Round($x), [int][Math]::Round($y))
    Start-Sleep -Milliseconds 500
}

function New-IsolatedProcess {
    param([Parameter(Mandatory = $true)][string]$Path)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Path
    $psi.WorkingDirectory = Split-Path -Parent $Path
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $false
    $psi.EnvironmentVariables['APPDATA'] = $appDataPath
    $psi.EnvironmentVariables['LOCALAPPDATA'] = $appDataPath
    $psi.EnvironmentVariables['DOWNSHIFT_LOG_DIR'] = (Join-Path $OutputDirectory 'app-logs')
    $psi.EnvironmentVariables['DOWNSHIFT_TELEMETRY_DIR'] = $telemetryPath
    $psi.EnvironmentVariables['DOWNSHIFT_TELEMETRY_ENABLED'] = 'false'
    $psi.EnvironmentVariables['DOWNSHIFT_ENV'] = 'smoke'
    New-Item -ItemType Directory -Force -Path $psi.EnvironmentVariables['DOWNSHIFT_LOG_DIR'] | Out-Null
    return [System.Diagnostics.Process]::Start($psi)
}

function Wait-ProcessWindow {
    param(
        [Parameter(Mandatory = $true)][System.Diagnostics.Process]$Process,
        [int]$TimeoutSeconds = 30
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $Process.Refresh()
        if ($Process.HasExited) {
            throw "Downshift exited during startup with code $($Process.ExitCode)."
        }
        $handles = @([DownshiftSmokeNative]::FindVisibleWindowsByTitle('downshift'))
        foreach ($handle in $handles) {
            $rect = Get-WindowRectObject $handle
            if ($null -ne $rect -and $rect.Width -ge 50 -and $rect.Height -ge 50) {
                return $handle
            }
        }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for the Downshift window.'
}

function Get-SettingsPath {
    $configDir = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'downshift'
    New-Item -ItemType Directory -Force -Path $configDir | Out-Null
    return Join-Path $configDir 'settings.toml'
}

function Get-RunValue {
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    try {
        return (Get-ItemProperty -LiteralPath $key -Name Downshift -ErrorAction Stop).Downshift
    } catch {
        return $null
    }
}

function Is-WebView2Installed {
    $clientId = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $subkey = "Software\Microsoft\EdgeUpdate\Clients\$clientId"
    $keys = @(
        "Registry::HKEY_CURRENT_USER\$subkey",
        "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$clientId",
        "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\EdgeUpdate\Clients\$clientId"
    )
    foreach ($key in $keys) {
        if (Test-Path -LiteralPath $key) {
            $version = (Get-ItemProperty -LiteralPath $key).pv
            if ($version -and $version -ne '0.0.0.0') {
                return $version
            }
        }
    }
    return $null
}

$appProcess = $null
$windowHandle = [IntPtr]::Zero
$settingsPath = Get-SettingsPath
$settingsExisted = Test-Path -LiteralPath $settingsPath -PathType Leaf
$runValue = Get-RunValue
$runValueExisted = $null -ne $runValue
$settingsBackup = Join-Path $userStateBackupPath 'settings.toml'

try {
    if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
        throw "Binary not found at '$BinaryPath'."
    }

    $runningInstances = Get-Process -Name downshift -ErrorAction SilentlyContinue |
        Where-Object {
            try { $_.Path -eq $BinaryPath } catch { $false }
        }
    if ($runningInstances) {
        throw "Downshift is already running from '$BinaryPath'; close it before running the smoke test."
    }

    if ($settingsExisted) {
        Copy-Item -LiteralPath $settingsPath -Destination $settingsBackup -Force
    }
    Remove-Item -LiteralPath $settingsPath -Force -ErrorAction SilentlyContinue
    Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name Downshift -ErrorAction SilentlyContinue

    $webView2Version = Is-WebView2Installed
    if ($webView2Version) {
        Log-Message "WebView2 detected: $webView2Version"
    } else {
        Log-Message 'WebView2 was not detected before launch.'
    }
    Log-Message "launching $BinaryPath"
    $appProcess = New-IsolatedProcess -Path $BinaryPath
    $windowHandle = Wait-ProcessWindow -Process $appProcess -TimeoutSeconds $StartupTimeoutSeconds
    Log-Message "main window detected: 0x$($windowHandle.ToString('X'))"
    Capture-Checkpoint -Name '01-widget' -WindowHandle $windowHandle

    $secondProcess = New-IsolatedProcess -Path $BinaryPath
    if (-not $secondProcess.WaitForExit(10000)) {
        $secondProcess.Kill()
        throw 'The second Downshift launch did not exit after forwarding activation.'
    }
    if ($secondProcess.ExitCode -ne 0) {
        throw "The second Downshift launch exited with code $($secondProcess.ExitCode)."
    }
    Log-Message 'second-instance activation passed'

    $mainPopup = Open-MainMenu -WindowHandle $windowHandle
    Capture-Checkpoint -Name '02-context-menu'

    Click-MainMenuItem -Popup $mainPopup -TextIndex 0
    $settingsText = Get-Content -LiteralPath $settingsPath -Raw
    if ($settingsText -notmatch '(?m)^paused = true$') {
        throw 'Pause menu action did not persist paused = true.'
    }
    Capture-Checkpoint -Name '03-paused' -WindowHandle $windowHandle

    $mainPopup = Open-MainMenu -WindowHandle $windowHandle
    Click-MainMenuItem -Popup $mainPopup -TextIndex 0
    $settingsText = Get-Content -LiteralPath $settingsPath -Raw
    if ($settingsText -notmatch '(?m)^paused = false$') {
        throw 'Resume menu action did not persist paused = false.'
    }
    Log-Message 'pause/resume menu action passed'

    $mainPopup = Open-MainMenu -WindowHandle $windowHandle
    Move-ToMainMenuItem -Popup $mainPopup -TextIndex 8
    $bugsSubmenu = Wait-SubmenuPopup -MainPopup $mainPopup
    Capture-Checkpoint -Name '04-bugs-submenu'
    Click-SubmenuItem -Submenu $bugsSubmenu -TextIndex 0 -TextItemCount 3
    $clipboard = Get-Clipboard -Raw -ErrorAction SilentlyContinue
    if ($null -eq $clipboard -or $clipboard -notmatch 'downshift diagnostics') {
        throw 'Copy diagnostics did not place the expected summary on the Windows clipboard.'
    }
    Log-Message "clipboard diagnostics passed ($($clipboard.Length) chars)"

    $mainPopup = Open-MainMenu -WindowHandle $windowHandle
    Move-ToMainMenuItem -Popup $mainPopup -TextIndex 3
    $breathingSubmenu = Wait-SubmenuPopup -MainPopup $mainPopup
    Capture-Checkpoint -Name '05-breathing-submenu'
    [DownshiftSmokeNative]::PressEscape()

    $mainPopup = Open-MainMenu -WindowHandle $windowHandle
    Move-ToMainMenuItem -Popup $mainPopup -TextIndex 7
    $updatesSubmenu = Wait-SubmenuPopup -MainPopup $mainPopup
    Capture-Checkpoint -Name '06-updates-submenu'
    Click-SubmenuItem -Submenu $updatesSubmenu -TextIndex 0 -TextItemCount 2
    $updatesHandle = Wait-WindowTitle -Title 'updates'
    Capture-Checkpoint -Name '07-updates-dialog' -WindowHandle $updatesHandle
    [DownshiftSmokeNative]::CloseWindow($updatesHandle)
    Log-Message 'updates dialog opened and closed'

    $currentRunValue = Get-RunValue
    if ($null -eq $currentRunValue -or $currentRunValue.Trim('"') -ne $BinaryPath) {
        throw "Launch-at-login registry value did not point to the test binary: '$currentRunValue'."
    }
    Log-Message 'launch-at-login registry value passed'

    $resultLines = @(
        'binary=' + $BinaryPath,
        'main_window_handle=0x' + $windowHandle.ToString('X'),
        'webview2_version=' + $(if ($webView2Version) { $webView2Version } else { 'missing' }),
        'second_instance=passed',
        'pause_resume=passed',
        'clipboard_copy=passed',
        'breathing_submenu_screenshot=passed',
        'updates_dialog=passed',
        'launch_at_login=passed',
        'run_log=' + $runLogPath
    )
    Set-Content -LiteralPath (Join-Path $OutputDirectory 'result.txt') -Value $resultLines -Encoding UTF8
    Log-Message "GUI smoke passed; result written to $(Join-Path $OutputDirectory 'result.txt')"
} finally {
    if ($null -ne $appProcess) {
        try {
            $appProcess.Refresh()
            if (-not $appProcess.HasExited) {
                [DownshiftSmokeNative]::CloseWindow($windowHandle)
                if (-not $appProcess.WaitForExit(5000)) {
                    Stop-Process -Id $appProcess.Id -Force
                }
            }
        } catch {
            Log-Message "warning: failed to stop smoke process cleanly: $($_.Exception.Message)"
        }
    }

    if ($settingsExisted) {
        Copy-Item -LiteralPath $settingsBackup -Destination $settingsPath -Force
    } else {
        Remove-Item -LiteralPath $settingsPath -Force -ErrorAction SilentlyContinue
    }

    $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    if ($runValueExisted) {
        Set-ItemProperty -LiteralPath $runKey -Name Downshift -Value $runValue
    } else {
        Remove-ItemProperty -LiteralPath $runKey -Name Downshift -ErrorAction SilentlyContinue
    }
}

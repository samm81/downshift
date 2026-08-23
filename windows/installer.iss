#define AppName "Downshift"
#define AppExeName "downshift.exe"
#ifndef AppVersion
#define AppVersion "0.2.0-rc.1"
#endif
#ifndef AppProductVersion
#define AppProductVersion "0.2.0.1"
#endif

[Setup]
AppId={{7A8BC2F5-8E5F-4E7D-9F7C-7DA32A5121E7}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Downshift
AppPublisherURL=https://github.com/samm81/downshift
AppSupportURL=https://github.com/samm81/downshift/issues
AppUpdatesURL=https://github.com/samm81/downshift/releases/latest
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
OutputBaseFilename=Downshift-Setup-{#AppVersion}
OutputDir=..\dist\windows
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
Uninstallable=yes
UninstallDisplayName={#AppName}
CloseApplications=yes
RestartIfNeededByRun=no
VersionInfoCompany=Downshift
VersionInfoDescription=Downshift breathing companion installer
VersionInfoProductName={#AppName}
VersionInfoProductVersion={#AppProductVersion}
VersionInfoCopyright=Copyright (c) Downshift contributors

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
Source: "..\dist\windows\stage\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; WebView2 creates this per-install cache beside the executable. It is
; application runtime data, not user settings, so remove it on uninstall.
Type: filesandordirs; Name: "{app}\{#AppExeName}.WebView2"

[Code]
const
  WebView2ClientId = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';
  WebView2BootstrapperUrl = 'https://go.microsoft.com/fwlink/?linkid=2124703';
  WebView2BootstrapperName = 'MicrosoftEdgeWebView2Setup.exe';

function WebView2VersionAt(RootKey: Integer; const Subkey: String): String;
begin
  if not RegQueryStringValue(RootKey, Subkey, 'pv', Result) then
    Result := '';
end;

function IsUsableWebView2Version(const Version: String): Boolean;
begin
  Result := (Version <> '') and (Version <> '0.0.0.0');
end;

function IsWebView2RuntimeInstalled(): Boolean;
var
  Subkey: String;
begin
  Subkey := 'Software\Microsoft\EdgeUpdate\Clients\' + WebView2ClientId;
  Result := IsUsableWebView2Version(WebView2VersionAt(HKEY_CURRENT_USER, Subkey));

  if not Result then
    Result := IsUsableWebView2Version(WebView2VersionAt(HKEY_LOCAL_MACHINE_64, Subkey));
  if not Result then
    Result := IsUsableWebView2Version(WebView2VersionAt(HKEY_LOCAL_MACHINE_32, Subkey));
end;

function InstallWebView2Runtime(): String;
var
  BootstrapperPath: String;
  ResultCode: Integer;
begin
  Result := '';
  BootstrapperPath := ExpandConstant('{tmp}\') + WebView2BootstrapperName;

  WizardForm.StatusLabel.Caption := 'Downloading Microsoft Edge WebView2 Runtime...';
  try
    DownloadTemporaryFile(WebView2BootstrapperUrl, WebView2BootstrapperName, '', nil);
  except
    Result := 'Downshift needs the Microsoft Edge WebView2 Runtime, but its installer could not be downloaded.'#13#13 + GetExceptionMessage;
    exit;
  end;

  if not FileExists(BootstrapperPath) then
  begin
    Result := 'Downshift needs the Microsoft Edge WebView2 Runtime, but its installer could not be downloaded.';
    exit;
  end;

  WizardForm.StatusLabel.Caption := 'Installing Microsoft Edge WebView2 Runtime...';
  if not Exec(BootstrapperPath, '/silent /install', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
  begin
    Result := 'Downshift needs the Microsoft Edge WebView2 Runtime, but its installer could not be started.';
    exit;
  end;

  if (ResultCode <> 0) and (ResultCode <> 3010) then
  begin
    Result := Format('The Microsoft Edge WebView2 Runtime installer exited with code %d.', [ResultCode]);
    exit;
  end;

  if not IsWebView2RuntimeInstalled() then
    Result := 'The Microsoft Edge WebView2 Runtime was not detected after installation.';
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if not IsWebView2RuntimeInstalled() then
    Result := InstallWebView2Runtime();
end;

procedure CurUninstallStepChanged(Change: TUninstallStep);
begin
  if Change = usUninstall then
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', '{#AppName}');
end;

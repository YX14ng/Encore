; The Windows installer, built with Inno Setup by .github/workflows/release.yml:
;   ISCC /DAppVersion=0.2.0 packaging\windows\encore.iss
; It installs for the current user only (no administrator rights), under
; %LOCALAPPDATA%\Programs\Encore, so the yt-dlp shipped with Encore can update itself there.

#ifndef AppVersion
  #error Pass the version: ISCC /DAppVersion=x.y.z encore.iss
#endif

[Setup]
AppId={{539E9C15-E10E-44F9-A873-39919C1F5843}
AppName=Encore
AppVersion={#AppVersion}
AppPublisher=YX14ng
AppPublisherURL=https://github.com/YX14ng/Encore
AppSupportURL=https://github.com/YX14ng/Encore/issues
AppUpdatesURL=https://github.com/YX14ng/Encore/releases
DefaultDirName={autopf}\Encore
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\..\LICENSE
SetupIconFile=encore.ico
UninstallDisplayIcon={app}\encore.exe
OutputDir=..\..\dist
OutputBaseFilename=encore-{#AppVersion}-x86_64-windows-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\dist\Encore\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; The AppUserModelID is the name Encore's notifications go out under.
Name: "{autoprograms}\Encore"; Filename: "{app}\encore.exe"; AppUserModelID: "io.github.yx14ng.Encore"
Name: "{autodesktop}\Encore"; Filename: "{app}\encore.exe"; Tasks: desktopicon; AppUserModelID: "io.github.yx14ng.Encore"

[Run]
; Also after a silent install: that is how Encore's own updater runs the installer, closing
; Encore first, so it has to start again.
Filename: "{app}\encore.exe"; Description: "{cm:LaunchProgram,Encore}"; Flags: nowait postinstall

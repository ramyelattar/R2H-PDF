#define MyAppName "R2H-PDF"
#define MyAppVersion "2.1.0"
#define MyAppPublisher "R2H"
#define MyAppExeName "r2h-pdf.exe"

[Setup]
AppId={{D53F8A0D-4704-4D5D-BA4C-75FCDCB792A1}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\R2H-PDF
DefaultGroupName=R2H-PDF
OutputDir=release\inno
OutputBaseFilename=R2H-PDF-2.1.0-x64-Full
Compression=lzma2/max
SolidCompression=yes
DiskSpanning=yes
DiskSliceSize=max
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExeName}
CloseApplications=yes
RestartApplications=no

[Files]
Source: "release\inno-stage\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\R2H-PDF"; Filename: "{app}\r2h-pdf.exe"
Name: "{autodesktop}\R2H-PDF"; Filename: "{app}\r2h-pdf.exe"

[Run]
Filename: "{app}\r2h-pdf.exe"; Description: "Launch R2H-PDF"; Flags: nowait postinstall skipifsilent

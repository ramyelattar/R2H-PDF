; R2H PDF AI Workstation - Full Offline Bootstrapper (Inno Setup 6)
; Purpose:
;   A disk-spanned offline setup that extracts the bundled payload to TEMP,
;   runs the real application installer, copies local-ai assets, configures
;   offline runtime paths, validates the local AI bundle, and exits.
;
; Important:
;   This Inno wrapper must NOT register itself as a second installed app.
;   Windows "Installed apps" must show only the real R2H PDF application
;   installed by the bundled NSIS/MSI payload.

#define MyAppName "R2H PDF AI Workstation"
#define MyAppVersion "2.1.0-beta"
#define MyAppVerName "R2H PDF AI Workstation v2.1.0-beta Full Offline"
#define MyAppPublisher "R2H"
#define MyAppURL "https://github.com/r2h-pdf"
#define PayloadRoot "{tmp}\R2H-PDF-FullOffline"

[Setup]
; Stable wrapper AppId. This is intentionally not used to create an uninstall entry
; because this setup is a bootstrapper only.
AppId={{A7E3F2B1-9C4D-4E5F-8A6B-1D2E3F4A5B6C}

AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppVerName}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}

; Bootstrapper mode:
; Do not create a wrapper application directory.
; Do not create a wrapper uninstall record.
; Do not create a wrapper Start Menu group.
; The bundled real app installer is the only component allowed to register
; the installed application in Windows Installed Apps.
CreateAppDir=no
Uninstallable=no
CreateUninstallRegKey=no
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=no
DisableFinishedPage=no

; Keep a log for diagnostics, but do not register the wrapper as installed software.
SetupLogging=yes

OutputDir=..\release\v2.1.0-beta\full-offline-installer
OutputBaseFilename=R2H-PDF-v2.1.0-beta-Full-Offline-Setup

; Large offline payload. DiskSpanning keeps the bootstrap EXE small while the
; bundled data is emitted beside it as numbered .bin slices.
Compression=lzma2/fast
SolidCompression=yes
LZMAUseSeparateProcess=yes
LZMANumBlockThreads=4
DiskSpanning=yes
DiskSliceSize=2000000000
SlicesPerDisk=1

ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
WizardStyle=modern
ShowLanguageDialog=no

; The wrapper must not create its own shortcuts.
; Do not add an [Icons] section for this file.

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
WelcomeLabel2=This will install {#MyAppVerName} on your computer.%n%nThis is a FULL OFFLINE installer that includes the application, local AI models, runtimes, workers, validation scripts, and documentation. No internet connection is required.%n%nThe installer will:%n- Install the R2H PDF application%n- Copy local AI assets to the local application data folder%n- Configure the environment for offline AI operation%n- Validate the local AI bundle%n%nOnly the main R2H PDF application will appear in Windows Installed Apps. This full-offline wrapper will not create a second installed-app entry.%n%nIt is recommended that you close all other applications before continuing.

[Files]
; Bundled real app installer payload.
; This folder must contain the freshly built NSIS/MSI app installers copied from:
;   src-tauri\target\release\bundle\...
Source: "..\release\v2.1.0-beta\full-offline-inno-staging\install-app\*"; \
    DestDir: "{#PayloadRoot}\install-app"; \
    Flags: ignoreversion recursesubdirs createallsubdirs

; Local AI assets: models, runtimes, workers, config.
Source: "..\release\v2.1.0-beta\full-offline-inno-staging\local-ai\*"; \
    DestDir: "{#PayloadRoot}\local-ai"; \
    Flags: ignoreversion recursesubdirs createallsubdirs

; Offline installation and validation scripts.
Source: "..\release\v2.1.0-beta\full-offline-inno-staging\scripts\*"; \
    DestDir: "{#PayloadRoot}\scripts"; \
    Flags: ignoreversion recursesubdirs createallsubdirs

; Release documentation.
Source: "..\release\v2.1.0-beta\full-offline-inno-staging\docs\*"; \
    DestDir: "{#PayloadRoot}\docs"; \
    Flags: ignoreversion recursesubdirs createallsubdirs

[Run]
; Bootstrap action:
; - installs the real R2H PDF application from the bundled app installer
; - copies local-ai to the configured local location
; - sets/validates R2H_LOCAL_AI_ROOT
; - runs local AI validation
;
; The PowerShell script must be responsible for invoking the real app installer.
; This wrapper does not install itself as an application.
Filename: "powershell.exe"; \
    Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{#PayloadRoot}\scripts\install-full-offline-inno.ps1"" -SourceRoot ""{#PayloadRoot}"""; \
    StatusMsg: "Installing R2H PDF and configuring offline AI assets..."; \
    Flags: waituntilterminated runhidden

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;

; Inno Setup script for the Cadmium installer.
; iscc /DVersion=0.1.0 /DArch=x64|arm64 /DSource=<portable folder> /DOutputDir=<dir> cadmium.iss

#if Arch == "arm64"
  #define Architectures "arm64"
#else
  #define Architectures "x64compatible"
#endif

[Setup]
AppId={{6F0B3C51-9A3E-4E57-9D5C-CD48A1B0C0DE}
AppName=Cadmium
AppVersion={#Version}
AppPublisher=Marcelo Martins
AppPublisherURL=https://github.com/marcelogm/cadmium
AppSupportURL=https://github.com/marcelogm/cadmium/issues
DefaultDirName={autopf}\Cadmium
DefaultGroupName=Cadmium
DisableProgramGroupPage=yes
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed={#Architectures}
ArchitecturesInstallIn64BitMode={#Architectures}
OutputDir={#OutputDir}
OutputBaseFilename=Cadmium-windows-{#Arch}-setup
SetupIconFile=..\..\crates\cadmium-app\data\app-icons\cadmium.ico
UninstallDisplayIcon={app}\cadmium.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Source}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Cadmium"; Filename: "{app}\cadmium.exe"
Name: "{autodesktop}\Cadmium"; Filename: "{app}\cadmium.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\cadmium.exe"; Description: "{cm:LaunchProgram,Cadmium}"; Flags: nowait postinstall skipifsilent

[Setup]
AppName=BnD-Ware (Rust Engine)
AppVersion=1.0
DefaultDirName={autopf}\BnDWare
DefaultGroupName=BnD-Ware
OutputDir=Output
OutputBaseFilename=BnDWare_Installer
Compression=lzma2
SolidCompression=yes
SetupIconFile=assets\game_icon.ico
UninstallDisplayIcon={app}\game_icon.ico
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes

[Files]
; Main Executables
Source: "target\release\bnd_game.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\bnd_mod_manager.exe"; DestDir: "{app}"; Flags: ignoreversion

; Core Game Data
; NOTE: the engine reads its assets straight from a "core" folder on disk
; (see resolve_path()/vfs_root in src/main.rs) - it never unzips core.bnd at
; runtime. So we install the *extracted* core\ folder produced by fetch.bat,
; not the zipped core.bnd archive (that archive is only an intermediate
; build artifact, kept around in case it's needed for distribution elsewhere).
Source: "core\*"; DestDir: "{app}\core"; Flags: ignoreversion recursesubdirs createallsubdirs

; Predefined Icons
Source: "assets\game_icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\mod_icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Shortcuts with predefined icons
Name: "{group}\BnD-Ware"; Filename: "{app}\bnd_game.exe"; IconFilename: "{app}\game_icon.ico"
Name: "{commondesktop}\BnD-Ware"; Filename: "{app}\bnd_game.exe"; IconFilename: "{app}\game_icon.ico"

Name: "{group}\BnD Mod Manager"; Filename: "{app}\bnd_mod_manager.exe"; IconFilename: "{app}\mod_icon.ico"
Name: "{commondesktop}\BnD Mod Manager"; Filename: "{app}\bnd_mod_manager.exe"; IconFilename: "{app}\mod_icon.ico"

Name: "{group}\Uninstall BnD-Ware"; Filename: "{uninstallexe}"

[Dirs]
; Ensure the HOME\BnD\Mods directory structure is mapped via Rust at runtime,
; but we can initialize the core folders here.
Name: "{app}\core"

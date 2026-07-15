[Setup]
AppName=BnD-Ware (Rust Engine)
AppVersion=1.0
DefaultDirName={autopf}\BnDWare
DefaultGroupName=BnD-Ware
OutputDir=Output
OutputBaseFilename=BnDWare_Installer

[Files]
; Main Executables
Source: "target\release\bnd_game.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\bnd_mod_manager.exe"; DestDir: "{app}"; Flags: ignoreversion

; Core Game Data Package
Source: "core.bnd"; DestDir: "{app}\core"; Flags: ignoreversion

; Predefined Icons
Source: "assets\game_icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\mod_icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Shortcuts with predefined icons
Name: "{group}\BnD-Ware"; Filename: "{app}\bnd_game.exe"; IconFilename: "{app}\game_icon.ico"
Name: "{commondesktop}\BnD-Ware"; Filename: "{app}\bnd_game.exe"; IconFilename: "{app}\game_icon.ico"

Name: "{group}\BnD Mod Manager"; Filename: "{app}\bnd_mod_manager.exe"; IconFilename: "{app}\mod_icon.ico"
Name: "{commondesktop}\BnD Mod Manager"; Filename: "{app}\bnd_mod_manager.exe"; IconFilename: "{app}\mod_icon.ico"

[Dirs]
; Ensure the HOME\BnD\Mods directory structure is mapped via Rust at runtime, 
; but we can initialize the core folders here.
Name: "{app}\core"
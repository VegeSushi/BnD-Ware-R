@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"

echo ===============================================
echo  BnD-Ware Full Build
echo ===============================================

REM ---------------------------------------------------------
REM 0. Sanity check tools
REM ---------------------------------------------------------
where cargo >nul 2>nul
if errorlevel 1 (
    echo ERROR: cargo not found on PATH. Install Rust from https://rustup.rs
    exit /b 1
)

REM ---------------------------------------------------------
REM 1. Fetch/refresh game assets and minigames into .\core
REM    (skipped if core\ already exists and /skipfetch was passed)
REM ---------------------------------------------------------
if "%~1"=="/skipfetch" (
    echo Skipping fetch step, using existing .\core
) else (
    if not exist core (
        echo Running fetch.bat to download assets/minigames...
        call fetch.bat
        if errorlevel 1 (
            echo ERROR: fetch.bat failed.
            exit /b 1
        )
    ) else (
        echo core\ already exists, skipping fetch. Delete it or pass no args to force re-fetch.
    )
)

REM ---------------------------------------------------------
REM 2. Build game + mod manager (release)
REM ---------------------------------------------------------
echo.
echo Building bnd_game and bnd_mod_manager (release)...
cargo build --release --bin bnd_game --bin bnd_mod_manager
if errorlevel 1 (
    echo ERROR: cargo build failed.
    exit /b 1
)

if not exist target\release\bnd_game.exe (
    echo ERROR: target\release\bnd_game.exe was not produced.
    exit /b 1
)
if not exist target\release\bnd_mod_manager.exe (
    echo ERROR: target\release\bnd_mod_manager.exe was not produced.
    exit /b 1
)

REM ---------------------------------------------------------
REM 3. Build the installer with Inno Setup (ISCC.exe)
REM ---------------------------------------------------------
echo.
echo Looking for Inno Setup compiler (ISCC.exe)...

set "ISCC="
where ISCC >nul 2>nul
if not errorlevel 1 (
    set "ISCC=ISCC"
) else (
    if exist "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe" set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
    if exist "%ProgramFiles%\Inno Setup 6\ISCC.exe" set "ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe"
)

if "%ISCC%"=="" (
    echo ERROR: ISCC.exe not found. Install Inno Setup 6 from https://jrsoftware.org/isinfo.php
    echo        or add its folder to PATH, then re-run this script.
    exit /b 1
)

echo Using: %ISCC%
if not exist Output mkdir Output

"%ISCC%" setup.iss
if errorlevel 1 (
    echo ERROR: Inno Setup compilation failed.
    exit /b 1
)

echo.
echo ===============================================
echo  Build complete!
echo  Game:        target\release\bnd_game.exe
echo  Mod Manager: target\release\bnd_mod_manager.exe
echo  Installer:   Output\BnDWare_Installer.exe
echo ===============================================
exit /b 0
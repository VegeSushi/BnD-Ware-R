@echo off
setlocal enabledelayedexpansion

echo Fetching BnD-Ware Repository...
if exist temp_bnd rmdir /S /Q temp_bnd
git clone https://github.com/VegeSushi/BnD-Ware temp_bnd
if errorlevel 1 (
    echo ERROR: git clone failed. Is git installed and is the repo reachable?
    exit /b 1
)

echo Creating core directories...
if exist core rmdir /S /Q core
mkdir core\minigames
mkdir core\assets

echo Copying files...
xcopy /E /I /Y temp_bnd\filesystem\minigames core\minigames
if errorlevel 1 (
    echo ERROR: failed to copy minigames.
    goto :cleanup_fail
)
xcopy /E /I /Y temp_bnd\assets core\assets
if errorlevel 1 (
    echo ERROR: failed to copy assets.
    goto :cleanup_fail
)

echo Cleaning up...
rmdir /S /Q temp_bnd

echo Done!
exit /b 0

:cleanup_fail
rmdir /S /Q temp_bnd
exit /b 1

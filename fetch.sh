#!/usr/bin/env bash
# Linux equivalent of fetch.bat: pulls game assets/minigames from the
# BnD-Ware repo into ./core so the engine has something to load.
set -euo pipefail
cd "$(dirname "$0")"

echo "Fetching BnD-Ware Repository..."
rm -rf temp_bnd
git clone https://github.com/VegeSushi/BnD-Ware temp_bnd

echo "Creating core directories..."
rm -rf core
mkdir -p core/minigames core/assets

echo "Copying files..."
cp -a temp_bnd/filesystem/minigames/. core/minigames/
cp -a temp_bnd/assets/. core/assets/

echo "Cleaning up..."
rm -rf temp_bnd

echo "Done!"

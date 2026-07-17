#!/usr/bin/env bash
# Linux equivalent of build.bat:
#   1. fetch/refresh core\ (assets + minigames), unless --skipfetch
#   2. cargo build --release (bnd_game + bnd_mod_manager)
#   3. package everything into an AppImage (Output/BnDWare-x86_64.AppImage)
set -euo pipefail
cd "$(dirname "$0")"

echo "==============================================="
echo " BnD-Ware Full Build (Linux)"
echo "==============================================="

# ---------------------------------------------------------
# 0. Sanity check tools
# ---------------------------------------------------------
if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: cargo not found on PATH. Install Rust from https://rustup.rs"
    exit 1
fi

# ---------------------------------------------------------
# 1. Fetch/refresh game assets and minigames into ./core
#    (skipped if core/ already exists and --skipfetch was passed)
# ---------------------------------------------------------
if [ "${1:-}" = "--skipfetch" ]; then
    echo "Skipping fetch step, using existing ./core"
else
    if [ ! -d core ]; then
        echo "Running fetch.sh to download assets/minigames..."
        ./fetch.sh
    else
        echo "core/ already exists, skipping fetch. Delete it or pass no args to force re-fetch."
    fi
fi

# ---------------------------------------------------------
# 2. Build game + mod manager (release)
# ---------------------------------------------------------
echo
echo "Building bnd_game and bnd_mod_manager (release)..."
cargo build --release --bin bnd_game --bin bnd_mod_manager

if [ ! -f target/release/bnd_game ]; then
    echo "ERROR: target/release/bnd_game was not produced."
    exit 1
fi
if [ ! -f target/release/bnd_mod_manager ]; then
    echo "ERROR: target/release/bnd_mod_manager was not produced."
    exit 1
fi

# ---------------------------------------------------------
# 3. Package into an AppImage
# ---------------------------------------------------------
echo
echo "Assembling AppDir..."

APPDIR="Output/BnDWare.AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"

cp target/release/bnd_game "$APPDIR/usr/bin/"
cp target/release/bnd_mod_manager "$APPDIR/usr/bin/"

# Engine expects a "core" folder sitting next to the executable at runtime
# (see resolve_path()/core_root in src/main.rs), so it travels inside the AppDir.
if [ -d core ]; then
    cp -a core "$APPDIR/usr/bin/core"
else
    echo "WARNING: ./core not found; AppImage will be missing game assets."
fi

# Icon: reuse the Windows .ico source asset if present, otherwise skip.
if [ -f assets/game_icon.ico ]; then
    if command -v convert >/dev/null 2>&1; then
        convert assets/game_icon.ico -resize 256x256 "$APPDIR/bndware.png" || true
    fi
fi
if [ ! -f "$APPDIR/bndware.png" ]; then
    # Fallback: create a tiny placeholder so appimagetool has an icon to embed.
    printf 'placeholder' > "$APPDIR/bndware.png.missing"
fi

cat > "$APPDIR/bndware.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=BnD-Ware
Comment=BnD-Ware (Rust Engine)
Exec=bnd_game
Icon=bndware
Categories=Game;
Terminal=false
EOF

cat > "$APPDIR/AppRun" <<'EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "${0}")")"
cd "${HERE}/usr/bin"
exec ./bnd_game "$@"
EOF
chmod +x "$APPDIR/AppRun"

echo "Fetching appimagetool..."
APPIMAGETOOL="Output/appimagetool.AppImage"
if [ ! -f "$APPIMAGETOOL" ]; then
    curl -L -o "$APPIMAGETOOL" \
        https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
    chmod +x "$APPIMAGETOOL"
fi

echo "Building AppImage..."
ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" \
    "Output/BnDWare-x86_64.AppImage"

echo
echo "==============================================="
echo " Build complete!"
echo " Game:        target/release/bnd_game"
echo " Mod Manager: target/release/bnd_mod_manager"
echo " AppImage:    Output/BnDWare-x86_64.AppImage"
echo "==============================================="
